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
pub mod schema;
pub mod tokenizer;

use std::path::Path;

use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::{Facet, Value};
use tantivy::tokenizer::{LowerCaser, TextAnalyzer};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

use crate::error::{Error, Result};
use crate::model::{Node, Page, SourceId};
use crate::Paths;

pub use extract::Extracted;
pub use schema::{boost, Fields, CODE_TOKENIZER};

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
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>> {
        let searcher = self.reader.searcher();

        let mut parser = QueryParser::for_index(
            &self.index,
            vec![
                self.fields.title,
                self.fields.headers,
                self.fields.body,
                self.fields.code,
            ],
        );
        parser.set_field_boost(self.fields.title, boost::TITLE);
        parser.set_field_boost(self.fields.headers, boost::HEADERS);
        parser.set_field_boost(self.fields.body, boost::BODY);
        parser.set_field_boost(self.fields.code, boost::CODE);

        let parsed = parser.parse_query(query).map_err(|source| Error::Search {
            message: source.to_string(),
        })?;

        // `.order_by_score()` is not optional decoration: in tantivy 0.26
        // `TopDocs` is a builder and only the ordered form implements
        // `Collector`. `limit.max(1)` is also load-bearing — `with_limit`
        // panics on 0, and the limit comes from a caller.
        let top = searcher
            .search(&parsed, &TopDocs::with_limit(limit.max(1)).order_by_score())
            .map_err(index_error)?;

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
    fn title_outranks_body() {
        // Not a tuning assertion — the boost values are S2-4's to set. This
        // only pins the direction, so a future change that inverts the field
        // weights fails here rather than silently.
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());

        let mut session = engine.session().expect("session");
        session
            .add_page(
                &page("s", "body-match.html", "Unrelated"),
                "C",
                &doc(vec![para("decorators are mentioned here in the body")]),
            )
            .expect("add");
        session
            .add_page(
                &page("s", "title-match.html", "Decorators"),
                "C",
                &doc(vec![para("unrelated prose")]),
            )
            .expect("add");
        session.commit().expect("commit");

        let hits = engine.search("decorators", 10).expect("search");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, "title-match.html", "title should rank first");
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
    fn a_malformed_query_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let engine = engine(dir.path());
        // Unbalanced quotes and a dangling operator: user input, so it must
        // surface as an error.
        assert!(engine.search("\"unterminated", 10).is_err());
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
