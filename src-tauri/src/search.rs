//! The search command (S2-7, specs P2-004/005/008).
//!
//! The one thing worth knowing before changing anything here: **snippets cross
//! the IPC boundary as spans, never as HTML**.
//!
//! A snippet is page content, and page content becomes HTML in exactly one
//! place — [`tome_core::render`], whose output goes into the sandboxed reader
//! frame. A snippet does not: it is drawn in the *app's* DOM, where the app's
//! origin and its IPC layer are reachable, and where an `{@html}` would be the
//! shortest path from a crawled page to executing script with the app's
//! privileges. So [`tome_core::search::Span`] carries text and a boolean, the
//! frontend renders each span's text as a text node, and there is no markup to
//! escape because there is no markup.
//!
//! Scoping (P2-008) filters *after* ranking, so the search over-fetches. That
//! is the honest stopgap the CLI already uses; a scoped query proper needs the
//! `source_id` term folded into the query, which is a ranking change and wants
//! the eval set.

use serde::Serialize;
use tome_core::model::{PagePath, SourceId};
use tome_core::search::{snippet, SearchEngine, Span};
use tome_core::store::PageStore;
use tome_core::Paths;

use crate::reader::ReaderState;

/// One result, as the results list needs it.
#[derive(Serialize)]
pub struct SearchHit {
    source_id: String,
    /// The source's display name, so the list can show "Rust std" rather than
    /// `rust-std`. Resolved here because the frontend would otherwise need a
    /// second round trip per result.
    source_name: String,
    path: String,
    title: String,
    score: f32,
    /// `function`, `type`, `module`… or `null` for a page that is not a
    /// reference page for one symbol (P2-015).
    symbol_kind: Option<&'static str>,
    /// The snippet, as alternating matched/unmatched runs. See the module
    /// docs for why this is not a string of HTML.
    snippet: Vec<Span>,
}

/// A whole search, including what it cost and what it corrected.
#[derive(Serialize)]
pub struct SearchResponse {
    hits: Vec<SearchHit>,
    /// Corrections applied to the query (P2-009), as `{typed, meant}`.
    /// Always present, never null.
    suggestions: Vec<Suggestion>,
    /// How long the search took, for P2-005's "show result count and search
    /// time". Measured around the whole command — index, snippets and all —
    /// because that is what the user waited for.
    elapsed_ms: f64,
    /// Whether the ranked list was truncated by `limit`. P2-005 asks for a
    /// result count; a *total* count needs a second, uncapped collector pass,
    /// so this reports the honest thing instead of an invented number.
    truncated: bool,
}

#[derive(Serialize)]
pub struct Suggestion {
    typed: String,
    meant: String,
}

/// How many hits to over-fetch when scoping, so a scoped search can still
/// fill its limit. Bounded, so a large limit cannot ask for the whole index.
const SCOPE_OVERFETCH: usize = 10;
const MAX_FETCH: usize = 1_000;

/// Search the library.
///
/// `scope` limits results to one source (P2-008). `limit` is the number of
/// results wanted, not the number ranked.
#[tauri::command]
pub fn search(
    state: tauri::State<'_, ReaderState>,
    query: String,
    scope: Option<String>,
    limit: usize,
) -> Result<SearchResponse, String> {
    run_search(&state.paths, &query, scope.as_deref(), limit)
}

/// The command's body, without the Tauri wrapper.
///
/// Split out because a `tauri::State` cannot be constructed in a unit test, so
/// a command that does its work inline is a command that is never tested.
pub fn run_search(
    paths: &Paths,
    query: &str,
    scope: Option<&str>,
    limit: usize,
) -> Result<SearchResponse, String> {
    let started = std::time::Instant::now();

    // An empty query is not an error and not a search: the modal opens empty
    // and calls this on every keystroke, including the one that empties it.
    if query.trim().is_empty() {
        return Ok(SearchResponse {
            hits: Vec::new(),
            suggestions: Vec::new(),
            elapsed_ms: 0.0,
            truncated: false,
        });
    }

    let engine = SearchEngine::open(paths).map_err(to_message)?;
    let fetch = if scope.is_some() {
        limit.saturating_mul(SCOPE_OVERFETCH).min(MAX_FETCH)
    } else {
        limit.min(MAX_FETCH)
    };

    let ranked = engine.search(query, fetch).map_err(to_message)?;
    let truncated = ranked.len() == fetch;
    let terms = engine.highlight_terms(query).map_err(to_message)?;

    // Names are looked up once rather than per hit: ten results from one
    // source would otherwise be ten identical database reads.
    let database = tome_core::db::Database::open(paths).map_err(to_message)?;
    let names: std::collections::BTreeMap<String, String> = database
        .list_sources()
        .map_err(to_message)?
        .into_iter()
        .map(|source| (source.id.as_str().to_owned(), source.name))
        .collect();

    // One store per source, kept across hits: ten results from one source
    // would otherwise rebuild the same path resolution ten times.
    let mut stores: std::collections::BTreeMap<String, PageStore> =
        std::collections::BTreeMap::new();
    let mut hits = Vec::new();
    for hit in ranked {
        if scope.is_some_and(|s| s != hit.source.as_str()) {
            continue;
        }
        if hits.len() >= limit {
            break;
        }

        let store = stores
            .entry(hit.source.as_str().to_owned())
            .or_insert_with(|| PageStore::new(paths, &hit.source));

        // A hit whose stored page has gone is a stale index entry, not a
        // failure: the index lives in the cache and the store in state, and
        // they can legitimately diverge. Showing the result without a snippet
        // beats dropping it, and beats failing the whole search.
        let snippet = PagePath::new(&hit.path)
            .ok()
            .and_then(|path| store.read(&path).ok().flatten())
            .map(|page| snippet::snippet(&page.body, &terms, snippet::default_length()))
            .unwrap_or_default();

        hits.push(SearchHit {
            source_name: names
                .get(hit.source.as_str())
                .cloned()
                .unwrap_or_else(|| hit.source.as_str().to_owned()),
            source_id: hit.source.as_str().to_owned(),
            path: hit.path,
            title: hit.title,
            score: hit.score,
            symbol_kind: hit.symbol_kind.map(|kind| kind.as_str()),
            snippet,
        });
    }

    let suggestions = engine
        .suggest(query)
        .map_err(to_message)?
        .into_iter()
        .map(|suggestion| Suggestion {
            typed: suggestion.typed,
            meant: suggestion.meant,
        })
        .collect();

    Ok(SearchResponse {
        hits,
        suggestions,
        elapsed_ms: started.elapsed().as_secs_f64() * 1_000.0,
        truncated,
    })
}

/// Errors reaching the UI must not carry the query.
///
/// A search query is reading history. `tome_core`'s errors are already written
/// to that rule — [`tome_core::error::Error::Search`] carries tantivy's
/// message and never the query text — and this keeps the boundary honest by
/// not adding any.
fn to_message(error: tome_core::error::Error) -> String {
    error.to_string()
}

/// Whether a source id is one the library knows, for scope validation.
///
/// Exposed so the UI can drop a remembered scope (P2-008 asks it to remember
/// one) that names a source since removed, rather than silently returning no
/// results for ever.
#[tauri::command]
pub fn source_exists(
    state: tauri::State<'_, ReaderState>,
    source_id: String,
) -> Result<bool, String> {
    run_source_exists(&state.paths, &source_id)
}

pub fn run_source_exists(paths: &Paths, source_id: &str) -> Result<bool, String> {
    let Ok(id) = SourceId::new(source_id) else {
        return Ok(false);
    };
    let database = tome_core::db::Database::open(paths).map_err(to_message)?;
    Ok(database
        .list_sources()
        .map_err(to_message)?
        .iter()
        .any(|source| source.id == id))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tome_core::model::{ContentHash, Node, Page, Source, SourceType};
    use tome_core::store::StoredPage;

    fn heading(level: u8, value: &str) -> Node {
        Node::Heading {
            level,
            id: None,
            children: vec![Node::Text {
                value: value.to_owned(),
            }],
        }
    }

    fn para(value: &str) -> Node {
        Node::Paragraph {
            children: vec![Node::Text {
                value: value.to_owned(),
            }],
        }
    }

    /// A two-source library with one page each, indexed and stored.
    fn library() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under_root(dir.path());
        paths.ensure_created().unwrap();

        let database = tome_core::db::Database::open(&paths).unwrap();
        let engine = SearchEngine::open(&paths).unwrap();
        let mut session = engine.session().unwrap();

        for (id, name, path, title, body) in [
            (
                "rust-std",
                "Rust std",
                "std/vec/struct.Vec.html",
                "Struct Vec",
                vec![
                    heading(1, "Struct Vec"),
                    para("A contiguous growable array type, written Vec<T>."),
                ],
            ),
            (
                "cargo",
                "The Cargo Book",
                "reference/environment-variables.html",
                "Environment variables",
                vec![
                    heading(1, "Environment variables"),
                    para("Cargo sets and reads a number of environment variables."),
                ],
            ),
        ] {
            let source = SourceId::new(id).unwrap();
            paths.ensure_source_dirs(&source).unwrap();
            database
                .upsert_source(&Source::new(source.clone(), name, SourceType::Generic))
                .unwrap();

            let page_path = PagePath::new(path).unwrap();
            let document = Node::Document { children: body };
            PageStore::new(&paths, &source)
                .write(&StoredPage {
                    path: page_path.clone(),
                    title: title.to_owned(),
                    description: None,
                    body: document.clone(),
                })
                .unwrap();

            let page = Page::new(
                source.clone(),
                page_path,
                title,
                ContentHash::new("0".repeat(64)).unwrap(),
            );
            session.add_page(&page, "Rust", &document).unwrap();
        }
        session.commit().unwrap();
        drop(session);

        (dir, paths)
    }

    #[test]
    fn a_search_returns_hits_with_snippets_and_kinds() {
        let (_dir, paths) = library();
        let response = run_search(&paths, "growable array", None, 10).unwrap();

        assert_eq!(response.hits.len(), 1);
        let hit = &response.hits[0];
        assert_eq!(hit.path, "std/vec/struct.Vec.html");
        // The display name, not the id: a results list showing `rust-std` has
        // made the user translate.
        assert_eq!(hit.source_name, "Rust std");
        assert_eq!(hit.symbol_kind, Some("type"));

        let text: String = hit.snippet.iter().map(|span| span.text.as_str()).collect();
        assert!(text.contains("contiguous growable array"), "{text:?}");
        let marked: Vec<&str> = hit
            .snippet
            .iter()
            .filter(|span| span.matched)
            .map(|span| span.text.as_str())
            .collect();
        assert!(marked.contains(&"growable"), "{marked:?}");
        assert!(marked.contains(&"array"), "{marked:?}");
    }

    #[test]
    fn a_snippet_is_spans_and_never_markup() {
        // The invariant the whole IPC shape exists for. If this ever becomes a
        // string of HTML, the frontend has to escape it, in the app's own DOM
        // where the IPC bridge is reachable.
        let (_dir, paths) = library();
        let response = run_search(&paths, "growable", None, 10).unwrap();
        let json = serde_json::to_string(&response.hits[0].snippet).unwrap();
        assert!(json.contains("\"matched\""), "{json}");
        assert!(!json.contains("<mark"), "{json}");
    }

    #[test]
    fn scoping_limits_results_to_one_source() {
        let (_dir, paths) = library();
        let scoped = run_search(&paths, "environment", Some("cargo"), 10).unwrap();
        assert_eq!(scoped.hits.len(), 1);
        assert_eq!(scoped.hits[0].source_id, "cargo");

        let elsewhere = run_search(&paths, "environment", Some("rust-std"), 10).unwrap();
        assert!(elsewhere.hits.is_empty(), "{:?}", elsewhere.hits.len());
    }

    #[test]
    fn an_empty_query_is_answered_without_touching_the_index() {
        // The modal calls this on every keystroke, including the one that
        // empties the field.
        let (_dir, paths) = library();
        for query in ["", "   ", "\t\n"] {
            let response = run_search(&paths, query, None, 10).unwrap();
            assert!(response.hits.is_empty());
            assert!(response.suggestions.is_empty());
        }
    }

    #[test]
    fn a_misspelling_is_corrected_and_reported() {
        let (_dir, paths) = library();
        let response = run_search(&paths, "enviroment", None, 10).unwrap();
        assert_eq!(response.suggestions.len(), 1);
        assert_eq!(response.suggestions[0].typed, "enviroment");
        assert_eq!(response.suggestions[0].meant, "environment");
        assert_eq!(response.hits.len(), 1);
        // And the correction is what gets highlighted — marking `enviroment`
        // would mark nothing, and the result would look unrelated.
        let marked: Vec<&str> = response.hits[0]
            .snippet
            .iter()
            .filter(|span| span.matched)
            .map(|span| span.text.as_str())
            .collect();
        // Case-insensitively: the snippet keeps the page's capitalisation,
        // which is the whole point of matching on a lowered copy.
        assert!(
            marked.iter().any(|m| m.eq_ignore_ascii_case("environment")),
            "{marked:?}"
        );
    }

    #[test]
    fn an_at_symbol_query_is_honoured_through_the_command() {
        let (_dir, paths) = library();
        let response = run_search(&paths, "@Vec", None, 10).unwrap();
        assert_eq!(response.hits.len(), 1);
        assert_eq!(response.hits[0].path, "std/vec/struct.Vec.html");
    }

    #[test]
    fn a_page_missing_from_the_store_still_yields_a_result() {
        // The index is in the cache and the store in state; they can diverge.
        // Dropping the hit would make a cleared store look like an empty
        // library, and failing would break the whole search.
        let (_dir, paths) = library();
        let source = SourceId::new("rust-std").unwrap();
        std::fs::remove_dir_all(paths.pages_dir(&source)).unwrap();

        let response = run_search(&paths, "growable", None, 10).unwrap();
        assert_eq!(response.hits.len(), 1);
        assert!(response.hits[0].snippet.is_empty());
    }

    #[test]
    fn a_remembered_scope_naming_a_removed_source_is_rejected() {
        let (_dir, paths) = library();
        assert!(run_source_exists(&paths, "cargo").unwrap());
        assert!(!run_source_exists(&paths, "never-existed").unwrap());
        // Not a valid id at all — a hand-edited preference. `false`, not an
        // error: the UI's response to both is the same.
        assert!(!run_source_exists(&paths, "../etc").unwrap());
        assert!(!run_source_exists(&paths, "").unwrap());
    }
}
