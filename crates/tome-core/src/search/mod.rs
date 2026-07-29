//! Full-text search over the library (S2-2, specs P2-001 and P2-002).
//!
//! Adopted on [SPIKE-003]'s measurements: 439 MB peak indexing 100 000 pages,
//! 18.7 ms worst-case p95, a 224 MB index, 3 MB idle. Every criterion passed
//! with margin, so none of the fallbacks the spike listed — sharding,
//! index-per-source, lazy loading — exist here.
//!
//! [SPIKE-003]: https://github.com/alexnodeland/tome/blob/main/docs/spikes/003-tantivy-scale.md
//!
//! # Shape
//!
//! [`SearchEngine`] is long-lived and read-only: it owns the index and a
//! reader, and answers queries. Writing happens through a short-lived
//! [`IndexSession`] obtained from [`SearchEngine::session`].
//!
//! That split is not ceremony. Tantivy's `IndexWriter` allocates per-thread
//! arenas sized from its memory budget, and SPIKE-003 (finding 3) showed the
//! budget behaves as a **speed** knob rather than a memory one — 512 MB
//! indexed 2.7× faster than 50 MB at no extra peak RSS. Taking that speed
//! means not holding a writer open for the process's lifetime, or the app
//! would carry indexing arenas while merely reading. A session is opened when
//! a crawl has pages to write and dropped when it commits.
//!
//! Tantivy also enforces a single writer per index directory with a lockfile,
//! so two concurrent sessions fail loudly rather than corrupting anything —
//! while searches continue to be served throughout, which is the
//! "concurrent read/write" P2-001 asks for.
//!
//! # Deleting
//!
//! There is no delete-one-page operation, deliberately. Doing it needs a term
//! unique to a page, which means a schema field beyond the seven P2-002
//! specifies, and per-page updates are S2-3's ticket, not this one. The reason
//! adding it later is cheap is SPIKE-003 finding 1: **indexing is three orders
//! of magnitude cheaper than crawling** — 100 000 pages index in 5–21 seconds,
//! against about seven hours to crawl them. The index lives under the cache
//! root precisely because throwing it away and rebuilding costs seconds.

pub mod extract;
pub mod fuzzy;
pub mod ranking;
pub mod schema;
pub mod tokenizer;

use std::collections::BTreeMap;
use std::path::Path;

use tantivy::collector::{DocSetCollector, TopDocs};
use tantivy::directory::MmapDirectory;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::{Facet, Value};
use tantivy::tokenizer::{LowerCaser, TextAnalyzer};
use tantivy::{
    doc, Index, IndexReader, IndexWriter, ReloadPolicy, Score, SegmentReader, TantivyDocument, Term,
};

use crate::error::{Error, Result};
use crate::model::{Node, Page, PagePath, SourceId};
use crate::Paths;

pub use extract::Extracted;
pub use fuzzy::Suggestion;
pub use ranking::{Ranking, StopwordPolicy};
pub use schema::{Fields, CODE_TOKENIZER};

/// Writer memory budget, in bytes.
///
/// 512 MB on SPIKE-003 finding 3: it indexed 2.7× faster than a 50 MB budget
/// (54k vs 19k pages/s) while peak RSS stayed flat, because peak is dominated
/// by per-thread arenas and merge activity rather than the nominal budget. The
/// parameter's name suggests a memory dial; it is not one.
const WRITER_MEMORY_BUDGET: usize = 512 * 1024 * 1024;

/// One search result.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub source: SourceId,
    pub path: String,
    pub title: String,
    pub score: f32,
}

/// The library's search index.
pub struct SearchEngine {
    index: Index,
    fields: Fields,
    reader: IndexReader,
}

impl SearchEngine {
    /// Open the library's index, creating it if absent.
    pub fn open(paths: &Paths) -> Result<Self> {
        Self::open_at(&paths.index_dir())
    }

    /// Open the library's index, rebuilding it from scratch if it will not
    /// open (P2-003: "handle index corruption gracefully").
    ///
    /// Discarding the index is the *correct* response, not a desperate one:
    /// it lives under the cache root precisely because it is derived, and
    /// SPIKE-003 finding 1 measured a rebuild at 5–21 seconds for 100 000
    /// pages against about seven hours to re-crawl them. Nothing
    /// irreplaceable is here.
    ///
    /// Returns whether it had to rebuild, so a caller can reindex rather than
    /// leave the user with a silently empty search.
    pub fn open_or_rebuild(paths: &Paths) -> Result<(Self, bool)> {
        let dir = paths.index_dir();
        match Self::open_at(&dir) {
            Ok(engine) => Ok((engine, false)),
            Err(error) => {
                tracing::warn!(
                    %error,
                    "search index could not be opened; discarding and rebuilding"
                );
                // Remove rather than truncate: a half-written segment file is
                // exactly what would not open a second time either.
                if dir.exists() {
                    std::fs::remove_dir_all(&dir).map_err(|source| Error::CreateDirectory {
                        path: dir.clone(),
                        source,
                    })?;
                }
                Ok((Self::open_at(&dir)?, true))
            }
        }
    }

    /// Open an index in a specific directory.
    ///
    /// Prefer [`open`](Self::open) — `paths` is the single source of truth for
    /// locations. This exists for tests and for the benchmark harness.
    pub fn open_at(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).map_err(|source| Error::CreateDirectory {
            path: dir.to_path_buf(),
            source,
        })?;

        let (index_schema, fields) = schema::build();

        // `open_or_create` rather than `create_in_dir`: the index must survive
        // restarts (P2-001), and an existing directory is the normal case.
        let directory = MmapDirectory::open(dir).map_err(|source| Error::Search {
            message: source.to_string(),
        })?;
        let index = Index::open_or_create(directory, index_schema).map_err(index_error)?;

        register_tokenizers(&index);

        // `OnCommitWithDelay` picks up commits made by *another* process — the
        // CLI indexing while the app is open — without the app polling.
        //
        // It is not sufficient on its own, and the "WithDelay" in the name is
        // the reason: the reload happens on a background thread after a delay,
        // so a search issued immediately after our own commit returns the
        // pre-commit view. Every "indexed page cannot be found" symptom starts
        // here, and it is timing-dependent, so a test can pass while the
        // product is broken. [`IndexSession::commit`] therefore reloads this
        // reader synchronously; the policy covers the cross-process case and
        // the explicit reload covers our own.
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(index_error)?;

        Ok(Self {
            index,
            fields,
            reader,
        })
    }

    /// Begin a writing session. See the module docs for why this is not held
    /// open for the engine's lifetime.
    ///
    /// Fails if another session holds the directory lock.
    pub fn session(&self) -> Result<IndexSession<'_>> {
        let writer: IndexWriter = self
            .index
            .writer(WRITER_MEMORY_BUDGET)
            .map_err(index_error)?;

        // Set explicitly on SPIKE-003 finding 4: search cost is roughly linear
        // in segment count, and a library that syncs incrementally forever
        // drifts toward many small segments. Inheriting the default would make
        // that drift invisible and unversioned; naming the policy means a
        // change to tantivy's default cannot silently change our latency.
        // S2-12's benchmark should watch segment count for the same reason.
        writer.set_merge_policy(Box::new(tantivy::merge_policy::LogMergePolicy::default()));

        Ok(IndexSession {
            writer,
            fields: self.fields,
            reader: &self.reader,
        })
    }

    /// Run a query, returning at most `limit` hits, best first.
    ///
    /// `query` is treated as **text a person typed**, not as query syntax —
    /// see [`plain_text_query`]. Field scoping (P2-016) will get a typed
    /// parameter rather than asking users to type `source_id:python`.
    ///
    /// Ranking uses [`Ranking::TUNED`]. Use [`search_with`](Self::search_with)
    /// to vary it; the relevance sweep is the only caller that should.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>> {
        self.search_with(query, limit, &Ranking::TUNED)
    }

    /// [`search`](Self::search) with explicit ranking parameters.
    ///
    /// This exists so that S2-4's tuning is *measured* rather than asserted:
    /// the sweep in `tests/relevance.rs` builds one index and evaluates the
    /// whole parameter space against it. Nothing in the product should pass
    /// anything but [`Ranking::TUNED`] — if a second configuration is ever
    /// wanted at runtime, it needs its own baseline.
    pub fn search_with(&self, query: &str, limit: usize, ranking: &Ranking) -> Result<Vec<Hit>> {
        let searcher = self.reader.searcher();
        let prepared = ranking.stopwords.apply(&plain_text_query(query));
        let prepared = prepared.as_str();

        let parser = self.parser(ranking);
        let parse = |text: &str| {
            parser.parse_query(text).map_err(|source| Error::Search {
                message: source.to_string(),
            })
        };

        let exact = parse(prepared)?;

        // Typo tolerance (S2-5). Corrections are searched as an *additional*
        // clause rather than in place of what was typed: the misspelling
        // matched nothing by definition, so leaving it in costs nothing and
        // removing it would mean rewriting the query string and getting the
        // substitution boundaries right for no benefit.
        let suggestions = self.corrections_for(&searcher, prepared, ranking)?;
        let parsed: Box<dyn tantivy::query::Query> = if suggestions.is_empty() {
            exact
        } else {
            let corrected: String = suggestions
                .iter()
                .map(|suggestion| suggestion.meant.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            Box::new(BooleanQuery::new(vec![
                (Occur::Should, exact),
                (
                    Occur::Should,
                    Box::new(BoostQuery::new(parse(&corrected)?, ranking.fuzzy)),
                ),
            ]))
        };

        // `limit.max(1)` is load-bearing — `with_limit` panics on 0, and the
        // limit comes from a caller.
        //
        // `tweak_score` replaces the `.order_by_score()` this used to call:
        // both produce a score-ordered collector, but only this one can reach
        // the per-document field length that the length penalty needs. It runs
        // per *matched* document rather than per returned one, so what it does
        // is kept to one branch and one division.
        let body = self.fields.body;
        let (pivot, strength) = (ranking.length_pivot, ranking.length_penalty);
        let collector =
            TopDocs::with_limit(limit.max(1)).tweak_score(move |segment: &SegmentReader| {
                // A field with no fieldnorms is not an error worth failing a
                // search over: it means nothing in this segment has a body, so
                // there is no length to penalise.
                let fieldnorms = segment.get_fieldnorms_reader(body).ok();
                move |doc, original: Score| {
                    let length = fieldnorms
                        .as_ref()
                        .map_or(0, |reader| reader.fieldnorm(doc));
                    ranking::length_scaled(original, length, pivot, strength)
                }
            });
        let top = searcher.search(&parsed, &collector).map_err(index_error)?;

        let mut hits = Vec::with_capacity(top.len());
        for (score, address) in top {
            let document: TantivyDocument = searcher.doc(address).map_err(index_error)?;

            let text = |field| {
                document
                    .get_first(field)
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned()
            };

            // A stored source_id that no longer parses means the index was
            // written by an older build with different validation rules.
            // Skipping the hit degrades one result; propagating would fail the
            // whole search.
            let Ok(source) = SourceId::new(text(self.fields.source_id)) else {
                continue;
            };

            hits.push(Hit {
                source,
                path: text(self.fields.path),
                title: text(self.fields.title),
                score,
            });
        }
        Ok(hits)
    }

    /// The fields a query is matched against, in the order the parser sees
    /// them.
    fn searched_fields(&self) -> [tantivy::schema::Field; 4] {
        [
            self.fields.title,
            self.fields.headers,
            self.fields.body,
            self.fields.code,
        ]
    }

    /// A parser over the searched fields with `ranking`'s boosts applied.
    fn parser(&self, ranking: &Ranking) -> QueryParser {
        let mut parser = QueryParser::for_index(&self.index, self.searched_fields().to_vec());
        parser.set_field_boost(self.fields.title, ranking.title);
        parser.set_field_boost(self.fields.headers, ranking.headers);
        parser.set_field_boost(self.fields.body, ranking.body);
        parser.set_field_boost(self.fields.code, ranking.code);
        parser
    }

    /// "Did you mean?" for a query, as P2-009 asks for (S2-5).
    ///
    /// Returns one [`Suggestion`] per term that appears **nowhere** in the
    /// index and has a near neighbour that does. An empty result means every
    /// term the user typed exists — which is the common case, and is why this
    /// costs nothing on a query that is spelled correctly: it is term
    /// dictionary lookups, and the scan only happens for a term that missed.
    ///
    /// This is the same computation [`search`](Self::search) applies
    /// internally, exposed so a caller can *show* the correction it silently
    /// benefited from. A search that quietly answers a different question than
    /// the one asked is worse than one that says what it did.
    pub fn suggest(&self, query: &str) -> Result<Vec<Suggestion>> {
        let searcher = self.reader.searcher();
        let prepared = Ranking::TUNED.stopwords.apply(&plain_text_query(query));
        self.corrections_for(&searcher, &prepared, &Ranking::TUNED)
    }

    /// Corrections for an already-prepared query string.
    fn corrections_for(
        &self,
        searcher: &tantivy::Searcher,
        prepared: &str,
        ranking: &Ranking,
    ) -> Result<Vec<Suggestion>> {
        if ranking.fuzzy <= 0.0 || ranking.fuzzy_max_distance == 0 {
            return Ok(Vec::new());
        }
        let terms = self.query_terms(prepared)?;
        fuzzy::corrections(
            searcher,
            &terms,
            &self.searched_fields(),
            // Corrections are drawn from prose only. The `code` field's
            // vocabulary is identifiers and their fragments, where near
            // neighbours are rife and meaningful — `read_dir` and `read_din`
            // would be one edit apart if both existed — so a correction
            // towards it is far likelier to be wrong than helpful.
            &[self.fields.title, self.fields.headers, self.fields.body],
            ranking.fuzzy_max_distance,
        )
    }

    /// The terms a query string produces, analysed exactly as the index
    /// analysed the documents.
    ///
    /// Tokenizing by hand here — splitting on whitespace, say — would ask the
    /// dictionary about strings that are not terms. The default analyser
    /// splits on punctuation and lowercases, so `Path.join` is two terms and
    /// `Vec` is `vec`; a hand-rolled split would look up `Path.join`, find
    /// nothing, and confidently offer to correct a word that was spelled
    /// perfectly.
    fn query_terms(&self, prepared: &str) -> Result<Vec<String>> {
        let mut analyzer = self
            .index
            .tokenizer_for_field(self.fields.body)
            .map_err(index_error)?;
        let mut stream = analyzer.token_stream(prepared);
        let mut terms = Vec::new();
        while let Some(token) = stream.next() {
            terms.push(token.text.clone());
        }
        Ok(terms)
    }

    /// Number of indexed documents currently visible to searches.
    pub fn len(&self) -> Result<u64> {
        Ok(self.reader.searcher().num_docs())
    }

    /// Whether the index has no visible documents.
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Segment count — the thing SPIKE-003 finding 4 identified as what
    /// actually degrades search latency. Exposed so S2-12's benchmark can
    /// assert on it rather than infer it.
    pub fn segment_count(&self) -> usize {
        self.reader.searcher().segment_readers().len()
    }

    /// Every page currently indexed for a source, as `path → content_hash`.
    ///
    /// This is the input to incremental indexing (S2-3), and it is read from
    /// **the index itself** rather than from the database on purpose. The
    /// database lives under the state root and the index under the cache
    /// root, so the two can legitimately diverge — a user clearing their cache,
    /// or macOS evicting it under disk pressure, leaves a full database and an
    /// empty index. Asking the database what is indexed would then report
    /// "everything, nothing to do" and leave search permanently empty with no
    /// error anywhere. Asking the index makes the sync self-correcting.
    pub fn indexed_pages(&self, source: &SourceId) -> Result<BTreeMap<String, String>> {
        let searcher = self.reader.searcher();
        let query = TermQuery::new(
            Term::from_field_text(self.fields.source_id, source.as_str()),
            tantivy::schema::IndexRecordOption::Basic,
        );
        let docs = searcher
            .search(&query, &DocSetCollector)
            .map_err(index_error)?;

        let mut out = BTreeMap::new();
        for address in docs {
            let document: TantivyDocument = searcher.doc(address).map_err(index_error)?;
            let text = |field| {
                document
                    .get_first(field)
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned()
            };
            out.insert(text(self.fields.path), text(self.fields.content_hash));
        }
        Ok(out)
    }
}

/// Register the custom tokenizer on the index.
///
/// **On the index, not on a writer.** `QueryParser` resolves tokenizers
/// through the same manager, so registering only where documents are written
/// would analyse queries with the default tokenizer and return nothing for
/// every code search — a failure with no error attached to it.
fn register_tokenizers(index: &Index) {
    let analyzer = TextAnalyzer::builder(tokenizer::CodeTokenizer)
        .filter(LowerCaser)
        .build();
    index.tokenizers().register(CODE_TOKENIZER, analyzer);
}

/// An open writing session. Commit it, or drop it to discard.
pub struct IndexSession<'a> {
    writer: IndexWriter,
    fields: Fields,
    /// Borrowed so [`commit`](Self::commit) can refresh it synchronously. See
    /// the reload-policy comment in [`SearchEngine::open_at`].
    reader: &'a IndexReader,
}

impl IndexSession<'_> {
    /// Add a page to the index.
    ///
    /// `category` is the owning source's category, which becomes the facet
    /// results can be filtered by. `content` is the page's stored AST.
    pub fn add_page(&mut self, page: &Page, category: &str, content: &Node) -> Result<()> {
        let extracted = extract::extract(content);

        let mut document = doc!(
            self.fields.source_id => page.source.as_str(),
            self.fields.path => page.path.as_str(),
            self.fields.title => page.title.as_str(),
            self.fields.body => extracted.body,
            self.fields.code => extracted.code,
            self.fields.category => facet_for(category),
            self.fields.content_hash => page.content_hash.as_str(),
        );

        // Multi-valued: one entry per heading, so a phrase query cannot run
        // from the end of one heading into the start of the next.
        for header in extracted.headers {
            document.add_text(self.fields.headers, header);
        }

        self.writer.add_document(document).map_err(index_error)?;
        Ok(())
    }

    /// Remove every document belonging to a source.
    ///
    /// `source_id` is a `STRING` field, so it is one unanalysed term and this
    /// is a single term delete. Takes effect on [`commit`](Self::commit).
    pub fn delete_source(&mut self, source: &SourceId) -> Result<()> {
        self.writer.delete_term(Term::from_field_text(
            self.fields.source_id,
            source.as_str(),
        ));
        Ok(())
    }

    /// Remove one page.
    ///
    /// A page's identity is the *pair* `(source_id, path)` — two sources may
    /// hold the same path, and routinely do (`index.html`). Tantivy's cheaper
    /// `delete_term` takes a single term, so this uses `delete_query` with a
    /// conjunction rather than adding a synthetic `source_id\0path` field to
    /// the schema: that field would be one more term per document, and
    /// SPIKE-003 finding 2 measured index size as vocabulary-driven. Deletes
    /// happen once per changed page during a sync, so the query costs nothing
    /// worth optimising away.
    pub fn delete_page(&mut self, source: &SourceId, path: &PagePath) -> Result<()> {
        let query = BooleanQuery::new(vec![
            (
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.fields.source_id, source.as_str()),
                    tantivy::schema::IndexRecordOption::Basic,
                )) as Box<dyn tantivy::query::Query>,
            ),
            (
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.fields.path, path.as_str()),
                    tantivy::schema::IndexRecordOption::Basic,
                )),
            ),
        ]);
        self.writer
            .delete_query(Box::new(query))
            .map_err(index_error)?;
        Ok(())
    }

    /// Make every change in this session visible to searches.
    ///
    /// Nothing added or deleted is durable until this returns — and nothing is
    /// *searchable* until the reader is refreshed, which this does
    /// synchronously. Relying on the reader's own `OnCommitWithDelay` policy
    /// instead would make "index a page, then search for it" a race that
    /// usually resolves the right way.
    pub fn commit(&mut self) -> Result<()> {
        self.writer.commit().map_err(index_error)?;
        self.reader.reload().map_err(index_error)?;
        Ok(())
    }
}

/// Build a facet from a category string.
///
/// Facets are hierarchical and `/`-delimited, and `Facet::from` treats the
/// input as an already-formed path. A category containing a slash would
/// therefore silently become two levels; escaping keeps it one. An empty
/// category becomes the root facet rather than an error, because a source with
/// no category is a normal thing to have.
fn facet_for(category: &str) -> Facet {
    let trimmed = category.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Facet::root();
    }
    Facet::from_path(trimmed.split('/').filter(|s| !s.is_empty()))
}

/// Neutralise Tantivy query-parser syntax in text a person typed.
///
/// **Found by S2-1's eval set on its first run**, which is what it is for.
/// Twelve of the corpus's symbol queries returned *nothing at all* —
/// `os.cpus()`, `range()`, `str.format()`, `[features] section` — because
/// `(`, `)`, `[` and `]` are grouping and range syntax to the query parser,
/// not characters. `os.cpus()` parsed as a term followed by an empty group;
/// `[features]` parsed as a malformed range. Typing a function's call syntax
/// is the single most natural way to search API documentation, and every one
/// of those queries silently failed.
///
/// `:` is the same trap and worse: `Vec::new` parses as *field* `Vec`, which
/// does not exist, so the query errors rather than returning nothing.
///
/// The fix is to stop treating user input as a query language. Structural
/// characters become spaces; the tokenizer already splits on them, so
/// `os.cpus()` and `Vec::new` reach the index as the terms they should have
/// been all along.
///
/// **Double quotes survive**, because phrase search is a feature users
/// reasonably expect and type — but only in pairs. An unbalanced quote is a
/// half-typed query, and passing it through turns an incomplete keystroke
/// into a parse error.
fn plain_text_query(raw: &str) -> String {
    // Tantivy's parser also treats `&&` and `||` as operators, but both are
    // stripped below as individual characters, so `a && b` becomes `a  b`.
    const STRUCTURAL: &[char] = &[
        '+', '-', '!', '(', ')', ':', '^', '[', ']', '{', '}', '~', '*', '?', '\\', '/', '&', '|',
        '<', '>', '=',
    ];

    let keep_quotes = raw.matches('"').count() % 2 == 0;
    let stripped: String = raw
        .chars()
        .map(|ch| match ch {
            '"' if keep_quotes => '"',
            '"' => ' ',
            c if STRUCTURAL.contains(&c) => ' ',
            c => c,
        })
        .collect();

    // Punctuation is not the whole story: the parser also has *keyword*
    // operators, so a bare `AND` is a syntax error and `pods AND nodes` is a
    // boolean expression rather than a search for three words. Lowercasing
    // removes the operator meaning and costs nothing for matching, because
    // both the default and code tokenizers lowercase what they index.
    stripped
        .split_inclusive(char::is_whitespace)
        .map(|piece| {
            let word = piece.trim_end();
            if matches!(word, "AND" | "OR" | "NOT" | "IN") {
                piece.to_lowercase()
            } else {
                piece.to_owned()
            }
        })
        .collect()
}

/// Tantivy's errors name index internals, never page content or queries.
fn index_error(source: tantivy::TantivyError) -> Error {
    Error::Search {
        message: source.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{ContentHash, PagePath};

    fn engine(dir: &Path) -> SearchEngine {
        SearchEngine::open_at(dir).expect("open index")
    }

    fn page(source: &str, path: &str, title: &str) -> Page {
        Page::new(
            SourceId::new(source).expect("source id"),
            PagePath::new(path).expect("page path"),
            title,
            ContentHash::new("a".repeat(64)).expect("hash"),
        )
    }

    fn doc(children: Vec<Node>) -> Node {
        Node::Document { children }
    }

    fn para(value: &str) -> Node {
        Node::Paragraph {
            children: vec![Node::Text {
                value: value.to_owned(),
            }],
        }
    }

    fn code(value: &str) -> Node {
        Node::CodeBlock {
            language: Some("rust".to_owned()),
            code: value.to_owned(),
        }
    }

    fn heading(level: u8, value: &str) -> Node {
        Node::Heading {
            level,
            id: None,
            children: vec![Node::Text {
                value: value.to_owned(),
            }],
        }
    }

    fn h1(value: &str) -> Node {
        heading(1, value)
    }

    #[test]
    fn indexes_and_finds_a_page() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("python", "tutorial/io.html", "Reading and Writing Files"),
                "Python",
                &doc(vec![para("Open a file and read its contents.")]),
            )
            .expect("add");
        session.commit().expect("commit");

        let hits = engine.search("reading files", 10).expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "tutorial/io.html");
        assert_eq!(hits[0].title, "Reading and Writing Files");
        assert_eq!(hits[0].source.as_str(), "python");
    }

    #[test]
    fn nothing_is_visible_before_commit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("python", "a.html", "Alpha"),
                "Python",
                &doc(vec![para("uncommitted content")]),
            )
            .expect("add");

        assert!(
            engine.search("uncommitted", 10).expect("search").is_empty(),
            "an uncommitted write must not be searchable"
        );

        session.commit().expect("commit");
        assert_eq!(engine.search("uncommitted", 10).expect("search").len(), 1);
    }

    #[test]
    fn index_survives_reopening() {
        // P2-001: "index persistence across restarts". Dropping and reopening
        // is the closest a test gets to a restart.
        let dir = tempfile::tempdir().expect("tempdir");
        {
            let engine = engine(dir.path());
            let mut session = engine.session().expect("session");
            session
                .add_page(
                    &page("rust", "std/vec.html", "Vec"),
                    "Rust",
                    &doc(vec![para("A contiguous growable array type.")]),
                )
                .expect("add");
            session.commit().expect("commit");
        }

        let reopened = engine(dir.path());
        assert_eq!(reopened.len().expect("len"), 1);
        assert_eq!(reopened.search("growable", 10).expect("search").len(), 1);
    }

    #[test]
    fn code_search_finds_a_snake_case_identifier_by_its_parts() {
        // The reason the code tokenizer exists. The default tokenizer would
        // index `read_to_string` as three terms and lose the whole, or as one
        // and lose the parts.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("rust", "fs.html", "std::fs"),
                "Rust",
                &doc(vec![code("let s = std::fs::read_to_string(path)?;")]),
            )
            .expect("add");
        session.commit().expect("commit");

        for query in ["read_to_string", "read_to_string", "string", "fs"] {
            assert_eq!(
                engine.search(query, 10).expect("search").len(),
                1,
                "query {query:?} should match"
            );
        }
    }

    #[test]
    fn code_search_finds_a_camel_case_identifier_by_its_parts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("node", "os.html", "OS"),
                "Node",
                &doc(vec![code("const n = os.getUserInfo();")]),
            )
            .expect("add");
        session.commit().expect("commit");

        for query in ["getUserInfo", "user", "info"] {
            assert_eq!(
                engine.search(query, 10).expect("search").len(),
                1,
                "query {query:?} should match"
            );
        }
    }

    #[test]
    fn query_matching_is_case_insensitive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("rust", "vec.html", "Vec"),
                "Rust",
                &doc(vec![code("Vec::with_capacity(10)")]),
            )
            .expect("add");
        session.commit().expect("commit");

        // If the LowerCaser were applied on only one of the two paths, one of
        // these would return nothing.
        for query in ["VEC", "vec", "with_capacity", "WITH_CAPACITY"] {
            assert_eq!(
                engine.search(query, 10).expect("search").len(),
                1,
                "query {query:?} should match"
            );
        }
    }

    #[test]
    fn deleting_a_source_leaves_the_others() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("python", "a.html", "Alpha"),
                "Python",
                &doc(vec![para("shared word")]),
            )
            .expect("add");
        session
            .add_page(
                &page("rust", "b.html", "Beta"),
                "Rust",
                &doc(vec![para("shared word")]),
            )
            .expect("add");
        session.commit().expect("commit");
        assert_eq!(engine.search("shared", 10).expect("search").len(), 2);

        // Explicit, not shadowed: rebinding the name would keep the first
        // session alive to the end of the scope and the second would fail to
        // take the directory lock. Callers hit this too — a session must be
        // dropped, not merely gone out of use.
        drop(session);
        let mut session = engine.session().expect("session");
        session
            .delete_source(&SourceId::new("python").expect("id"))
            .expect("delete");
        session.commit().expect("commit");

        let hits = engine.search("shared", 10).expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source.as_str(), "rust");
    }

    #[test]
    fn a_page_about_a_subject_outranks_one_that_merely_mentions_it() {
        // The direction that must never invert, pinned so a field-weight
        // change fails here rather than silently.
        //
        // Both pages carry an h1 matching their title, because that is the
        // only page shape that can be stored: normalization guarantees exactly
        // one h1 per page and that it *is* the title (see `Node::Heading`).
        // The version of this test before S2-4 omitted the headings, which
        // made it a test of the `title` field alone rather than of the
        // subject-versus-mention behaviour it was named for — and it duly
        // failed when tuning moved weight from `title` to `headers`, on a page
        // shape that does not occur.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("s", "body-match.html", "Unrelated"),
                "C",
                &doc(vec![
                    h1("Unrelated"),
                    para("decorators are mentioned here in the body"),
                ]),
            )
            .expect("add");
        session
            .add_page(
                &page("s", "subject.html", "Decorators"),
                "C",
                &doc(vec![h1("Decorators"), para("unrelated prose")]),
            )
            .expect("add");
        session.commit().expect("commit");

        let hits = engine.search("decorators", 10).expect("search");
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].path, "subject.html",
            "the page about it ranks first"
        );
    }

    #[test]
    fn a_whole_book_on_one_page_does_not_bury_the_page_about_the_topic() {
        // The defect S2-4 exists to fix, in miniature. `cargo:cargo/print.html`
        // is the entire Cargo book concatenated onto one page and it outranked
        // every specific page for every Cargo query, because a long document
        // matches everything. BM25's `b` would be the textbook lever and is a
        // private constant in tantivy 0.26, so `Ranking::length_penalty` does
        // the job from outside the scorer.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");

        // A page whose subject is workspaces, mentioning the term a few times.
        session
            .add_page(
                &page("cargo", "workspaces.html", "Workspaces"),
                "Rust",
                &doc(vec![
                    h1("Workspaces"),
                    para("A workspace is a set of packages that share one lock file."),
                ]),
            )
            .expect("add");

        // The whole book on one page. It carries every chapter's *heading* as
        // well as its prose — including a "Workspaces" one — which is what
        // makes a real concatenated book beat the specific page rather than
        // lose to it on the headers boost. A first version of this test
        // omitted the headings and the control assertion below caught it.
        // 2000 sections, not 200: a real single-page book is tens of
        // thousands of words, and a synthetic one only a few thousand words
        // long sits close enough to the length pivot that the test balances on
        // a knife edge and flips when an unrelated boost moves. It should
        // model the thing it is named after.
        let mut book = vec![h1("The Cargo Book"), heading(2, "Workspaces")];
        for section in 0..2_000 {
            book.push(heading(2, &format!("Chapter {section}")));
            book.push(para(&format!(
                "Section {section} covers packages, targets, profiles, features, \
                 registries and workspaces in exhaustive detail with examples."
            )));
        }
        session
            .add_page(
                &page("cargo", "print.html", "The Cargo Book"),
                "Rust",
                &doc(book),
            )
            .expect("add");
        session.commit().expect("commit");

        let hits = engine.search("workspaces", 10).expect("search");
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].path, "workspaces.html",
            "the page about workspaces should beat the book that contains a chapter on them"
        );

        // And the penalty is what does it: turn it off and the book wins,
        // which is both the regression guard and the evidence that this test
        // is testing the thing it claims to.
        let unpenalised = Ranking {
            length_penalty: 0.0,
            ..Ranking::TUNED
        };
        let hits = engine
            .search_with("workspaces", 10, &unpenalised)
            .expect("search");
        assert_eq!(
            hits[0].path, "print.html",
            "without the length penalty the book dump should win — if it does not, \
             this test has stopped exercising the penalty"
        );
    }

    #[test]
    fn searches_are_served_while_a_session_is_open() {
        // P2-001's "concurrent read/write support": a writer holds the
        // directory lock, and reads must not block on it.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("s", "a.html", "Alpha"),
                "C",
                &doc(vec![para("findable")]),
            )
            .expect("add");
        session.commit().expect("commit");

        // The session is deliberately still open and still holding the
        // directory lock. A reader that contended on that lock would deadlock
        // or error here rather than answer.
        assert_eq!(engine.search("findable", 10).expect("search").len(), 1);

        // And it can keep writing afterwards.
        session
            .add_page(
                &page("s", "b.html", "Beta"),
                "C",
                &doc(vec![para("findable too")]),
            )
            .expect("add");
        session.commit().expect("commit");
        assert_eq!(engine.search("findable", 10).expect("search").len(), 2);
    }

    #[test]
    fn a_second_concurrent_session_fails_loudly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());
        let _first = engine.session().expect("first session");
        assert!(
            engine.session().is_err(),
            "tantivy's lockfile should reject a second writer rather than \
             allow two to interleave"
        );
    }

    #[test]
    fn call_syntax_and_paths_are_searchable() {
        // The defect S2-1's eval set found: `(`, `)`, `[`, `]` and `:` are
        // query-parser syntax, so every one of these returned nothing (or
        // errored) before `plain_text_query`. Typing a function's call syntax
        // is the most natural way to search API documentation.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("node", "os.html", "OS"),
                "Node",
                &doc(vec![
                    code("os.cpus(); os.homedir();"),
                    para("returns an array"),
                ]),
            )
            .expect("add");
        session
            .add_page(
                &page("rust", "vec.html", "Vec"),
                "Rust",
                &doc(vec![code("let v = Vec::new();")]),
            )
            .expect("add");
        session.commit().expect("commit");

        for query in ["os.cpus()", "os.homedir()", "cpus()"] {
            let hits = engine.search(query, 10).expect("search");
            assert_eq!(
                hits.first().map(|h| h.path.as_str()),
                Some("os.html"),
                "query {query:?}"
            );
        }
        for query in ["Vec::new", "Vec::new()"] {
            let hits = engine.search(query, 10).expect("search");
            assert_eq!(
                hits.first().map(|h| h.path.as_str()),
                Some("vec.html"),
                "query {query:?}"
            );
        }
    }

    #[test]
    fn user_input_is_never_query_syntax() {
        // Anything a person can type must return results or nothing — never a
        // parse error. A search box that rejects `C++` is broken.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("s", "a.html", "Alpha"),
                "C",
                &doc(vec![para("ordinary prose")]),
            )
            .expect("add");
        session.commit().expect("commit");

        for query in [
            "\"unterminated",
            "C++",
            "a && b",
            "AND",
            "^",
            "*",
            "title:foo",
            "[unclosed",
            "~~~",
            "a || b",
            "!important",
            "-leading-dash",
            "()",
            "",
            "   ",
        ] {
            assert!(
                engine.search(query, 10).is_ok(),
                "query {query:?} must not be an error"
            );
        }
    }

    #[test]
    fn plain_text_query_strips_structure_but_keeps_paired_quotes() {
        assert_eq!(plain_text_query("os.cpus()"), "os.cpus  ");
        assert_eq!(plain_text_query("Vec::new"), "Vec  new");
        assert_eq!(plain_text_query("[features] section"), " features  section");
        // Paired quotes survive so phrase search still works...
        assert_eq!(plain_text_query("\"read to string\""), "\"read to string\"");
        // ...and an unbalanced one does not, because a half-typed query should
        // return results rather than an error.
        assert_eq!(plain_text_query("\"unterminated"), " unterminated");
        // `_` and `.` are not parser syntax and must survive: the code
        // tokenizer relies on seeing the whole identifier.
        assert_eq!(plain_text_query("read_to_string"), "read_to_string");
        // Keyword operators are lowercased, not stripped — `pods AND nodes`
        // should search for three words, not evaluate a boolean expression.
        assert_eq!(plain_text_query("pods AND nodes"), "pods and nodes");
        assert_eq!(plain_text_query("NOT"), "not");
        // A word that merely contains an operator is untouched.
        assert_eq!(plain_text_query("ANDROID"), "ANDROID");
        assert_eq!(plain_text_query("android"), "android");
    }

    /// A small library with enough prose for corrections to have somewhere to
    /// come from.
    fn typo_corpus(dir: &Path) -> SearchEngine {
        let engine = engine(dir);
        let mut session = engine.session().expect("session");
        for (path, title, prose) in [
            (
                "environment-variables.html",
                "Environment variables",
                "Cargo reads a number of environment variables. Each environment \
                 variable is documented here with the environment it applies to.",
            ),
            (
                "manifest.html",
                "The manifest format",
                "The manifest format is described here. Every manifest is a TOML \
                 file, and the manifest sections follow.",
            ),
            (
                "profiles.html",
                "Profiles",
                "Profiles provide a way to alter compiler settings.",
            ),
        ] {
            session
                .add_page(
                    &page("cargo", path, title),
                    "Rust",
                    &doc(vec![h1(title), para(prose)]),
                )
                .expect("add");
        }
        session.commit().expect("commit");
        drop(session);
        engine
    }

    #[test]
    fn a_misspelled_query_finds_the_page_anyway() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = typo_corpus(dir.path());

        // Exactly the shape P2-009's success metric names: a typo that
        // matches nothing at all should still reach the right page.
        for query in ["enviroment variables", "manifset format"] {
            let hits = engine.search(query, 10).expect("search");
            assert!(!hits.is_empty(), "query {query:?} found nothing");
        }
        assert_eq!(
            engine.search("enviroment", 10).expect("search")[0].path,
            "environment-variables.html"
        );
        assert_eq!(
            engine.search("manifset", 10).expect("search")[0].path,
            "manifest.html"
        );
    }

    #[test]
    fn typo_tolerance_is_what_finds_it() {
        // The control. Without this the test above would pass just as happily
        // if `enviroment` were somehow matching on its own, and would stop
        // exercising typo tolerance the moment it broke.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = typo_corpus(dir.path());
        let off = Ranking {
            fuzzy: 0.0,
            ..Ranking::TUNED
        };
        assert!(
            engine
                .search_with("enviroment", 10, &off)
                .expect("search")
                .is_empty(),
            "with tolerance off a misspelling must match nothing"
        );
    }

    #[test]
    fn did_you_mean_names_the_correction() {
        // P2-009 asks for suggestions. A search that quietly answers a
        // different question than the one asked is worse than one that says
        // what it did.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = typo_corpus(dir.path());

        assert_eq!(
            engine.suggest("enviroment variables").expect("suggest"),
            vec![Suggestion {
                typed: "enviroment".to_owned(),
                meant: "environment".to_owned(),
            }]
        );
    }

    #[test]
    fn a_correctly_spelled_query_is_never_corrected() {
        // The common case, and the one that must cost nothing: every term is
        // in the dictionary, so no candidate scan happens at all.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = typo_corpus(dir.path());

        for query in [
            "environment variables",
            "manifest format",
            "profiles",
            "toml",
        ] {
            assert!(
                engine.suggest(query).expect("suggest").is_empty(),
                "query {query:?} was spelled correctly and must not be corrected"
            );
        }
    }

    #[test]
    fn a_word_that_is_simply_absent_is_not_corrected_into_something_else() {
        // The failure mode that makes typo tolerance worse than none: a user
        // searches for a term this library genuinely does not contain, and
        // gets confident results about a different topic. Nothing within edit
        // distance means no suggestion, not the nearest thing available.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = typo_corpus(dir.path());

        assert!(engine.suggest("kubernetes").expect("suggest").is_empty());
        assert!(engine
            .suggest("photosynthesis")
            .expect("suggest")
            .is_empty());
    }

    #[test]
    fn short_terms_are_never_corrected() {
        // P2-009's schedule allows 0 edits below four characters, and the
        // reason is false positives: at distance 1, `Vec` reaches `Vex`,
        // `Vev`, `sec`, `hex`, `ves`. A three-character typo is better left
        // alone than guessed at.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = typo_corpus(dir.path());
        assert!(engine.suggest("tml").expect("suggest").is_empty());
        assert!(engine.suggest("env").expect("suggest").is_empty());
    }

    #[test]
    fn a_typo_inside_the_first_characters_is_not_corrected() {
        // The documented blind spot of prefix-anchored candidate search, kept
        // honest by a test: this is a *limitation*, not a bug, and it should
        // fail here rather than surprise someone in the app. `nvironment` is
        // one deletion from `environment` and shares no three-character
        // prefix with it.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = typo_corpus(dir.path());
        assert!(
            engine.suggest("nvironment").expect("suggest").is_empty(),
            "correcting this would mean a Levenshtein automaton over the whole \
             dictionary — see the `fuzzy` module docs"
        );
    }

    #[test]
    fn corrections_do_not_reach_into_the_code_field() {
        // Identifiers have dense near neighbours and correcting towards them
        // is far likelier to be wrong than helpful. `paresInt` should not
        // become `parseInt` by way of the code field alone.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());
        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("node", "globals.html", "Globals"),
                "Node",
                &doc(vec![h1("Globals"), code("parseInt(value, radix);")]),
            )
            .expect("add");
        session.commit().expect("commit");

        assert!(
            engine.suggest("parseInto").expect("suggest").is_empty(),
            "a correction must not be drawn from the code field's vocabulary"
        );
    }

    #[test]
    fn empty_index_returns_no_hits() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());
        assert!(engine.is_empty().expect("is_empty"));
        assert!(engine.search("anything", 10).expect("search").is_empty());
    }

    #[test]
    fn category_with_a_slash_stays_one_facet_level() {
        assert_eq!(facet_for("Python").to_string(), "/Python");
        assert_eq!(facet_for("").to_string(), "/");
        assert_eq!(facet_for("/").to_string(), "/");
        // Two levels, deliberately — a category is allowed to be hierarchical,
        // and the escaping that matters is that empty segments cannot appear.
        assert_eq!(facet_for("Python//Std").to_string(), "/Python/Std");
    }
}
