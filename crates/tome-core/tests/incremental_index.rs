//! Incremental indexing (S2-3, spec P2-003).
//!
//! These drive `pipeline::index_source` against a real library on disk rather
//! than the search engine directly, because every defect this ticket can have
//! lives in the *reconciliation* between three things that can disagree — the
//! database, the page store, and the index — not in Tantivy.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use tome_core::db::Database;
use tome_core::model::{ContentHash, Node, Page, PagePath, SourceId};
use tome_core::pipeline::{index_source, IndexReport};
use tome_core::search::SearchEngine;
use tome_core::store::{PageStore, StoredPage};
use tome_core::Paths;

/// A library rooted at a temporary directory.
struct Library {
    _dir: tempfile::TempDir,
    paths: Paths,
    source: SourceId,
}

impl Library {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = Paths::under_root(dir.path().to_path_buf());
        let source = SourceId::new("python").expect("source id");
        paths.ensure_created().expect("roots");
        paths.ensure_source_dirs(&source).expect("dirs");

        let database = Database::open(&paths).expect("open db");
        let mut config_source = tome_core::model::Source::new(
            source.clone(),
            "Python",
            tome_core::model::SourceType::Generic,
        );
        config_source.page_count = 0;
        database.upsert_source(&config_source).expect("upsert");

        Self {
            _dir: dir,
            paths,
            source,
        }
    }

    /// Write a page to both the store and the database, as a pull would.
    fn put(&self, path: &str, title: &str, body_text: &str, ordinal: u32) {
        let page_path = PagePath::new(path).expect("page path");
        let hash = ContentHash::from_digest(tome_core::hash::sha256(body_text.as_bytes()));

        let store = PageStore::new(&self.paths, &self.source);
        store
            .write(&StoredPage {
                path: page_path.clone(),
                title: title.to_owned(),
                description: None,
                body: Node::Document {
                    children: vec![Node::Paragraph {
                        children: vec![Node::Text {
                            value: body_text.to_owned(),
                        }],
                    }],
                },
            })
            .expect("write page");

        let database = Database::open(&self.paths).expect("open db");
        database
            .upsert_page(
                &Page::new(self.source.clone(), page_path, title, hash),
                ordinal,
            )
            .expect("upsert page");
    }

    fn remove(&self, path: &str) {
        let page_path = PagePath::new(path).expect("page path");
        let database = Database::open(&self.paths).expect("open db");
        database
            .delete_page(&self.source, &page_path)
            .expect("delete page");
        PageStore::new(&self.paths, &self.source)
            .remove(&page_path)
            .expect("remove stored page");
    }

    fn index(&self) -> IndexReport {
        index_source(&self.paths, &self.source, "Python", &mut |_| {}).expect("index")
    }

    fn engine(&self) -> SearchEngine {
        SearchEngine::open(&self.paths).expect("open engine")
    }

    fn search(&self, query: &str) -> Vec<String> {
        self.engine()
            .search(query, 20)
            .expect("search")
            .into_iter()
            .map(|hit| hit.path)
            .collect()
    }
}

#[test]
fn first_index_adds_everything() {
    let library = Library::new();
    library.put("a.html", "Alpha", "alpha content", 0);
    library.put("b.html", "Beta", "beta content", 1);

    let report = library.index();
    assert_eq!(report.added, 2);
    assert_eq!(report.updated, 0);
    assert_eq!(report.removed, 0);
    assert_eq!(report.unchanged, 0);
    assert_eq!(library.search("content").len(), 2);
}

#[test]
fn re_indexing_unchanged_pages_writes_nothing() {
    // The whole point of the ticket. Not for indexing speed — SPIKE-003 says
    // that is free — but because a commit creates a segment, and segment count
    // is what degrades search latency.
    let library = Library::new();
    library.put("a.html", "Alpha", "alpha content", 0);
    library.index();

    let before = library.engine().segment_count();
    let report = library.index();

    assert_eq!(report.unchanged, 1);
    assert_eq!(report.added, 0);
    assert!(report.is_noop(), "a no-op sync must report itself as one");
    assert_eq!(
        library.engine().segment_count(),
        before,
        "an unchanged sync must not create a segment"
    );
}

#[test]
fn a_changed_page_replaces_rather_than_duplicating() {
    // P2-003: "no duplicate documents in index". Tantivy has no update, so
    // adding without deleting leaves both documents and the page appears
    // twice in every result list.
    let library = Library::new();
    library.put("a.html", "Alpha", "original wording", 0);
    library.index();

    library.put("a.html", "Alpha", "replacement wording", 0);
    let report = library.index();

    assert_eq!(report.updated, 1);
    assert_eq!(report.added, 0);
    assert_eq!(
        library.engine().len().expect("len"),
        1,
        "the page must exist once, not twice"
    );
    assert_eq!(library.search("replacement"), ["a.html"]);
    assert!(
        library.search("original").is_empty(),
        "the superseded document must be gone"
    );
}

#[test]
fn a_deleted_page_leaves_the_index() {
    let library = Library::new();
    library.put("a.html", "Alpha", "alpha content", 0);
    library.put("b.html", "Beta", "beta content", 1);
    library.index();

    library.remove("b.html");
    let report = library.index();

    assert_eq!(report.removed, 1);
    assert_eq!(report.unchanged, 1);
    assert_eq!(library.search("content"), ["a.html"]);
}

#[test]
fn only_the_changed_page_is_rewritten() {
    let library = Library::new();
    for (i, name) in ["a", "b", "c", "d"].iter().enumerate() {
        library.put(
            &format!("{name}.html"),
            name,
            &format!("{name} content"),
            u32::try_from(i).expect("ordinal"),
        );
    }
    library.index();

    library.put("c.html", "c", "c rewritten", 2);
    let report = library.index();

    assert_eq!(report.updated, 1);
    assert_eq!(report.unchanged, 3);
    assert_eq!(report.added, 0);
    assert_eq!(report.removed, 0);
}

#[test]
fn an_emptied_index_is_rebuilt_from_the_library() {
    // The divergence that makes "ask the database what is indexed" wrong. The
    // index is in the cache and the database in the state root, so clearing
    // one leaves the other intact. If the sync trusted the database it would
    // conclude "all indexed, nothing to do" and leave search permanently
    // empty, with no error anywhere.
    let library = Library::new();
    library.put("a.html", "Alpha", "alpha content", 0);
    library.index();
    assert_eq!(library.search("alpha").len(), 1);

    std::fs::remove_dir_all(library.paths.index_dir()).expect("clear the index cache");

    let report = library.index();
    assert_eq!(report.added, 1, "a cleared index must be repopulated");
    assert_eq!(library.search("alpha").len(), 1);
}

#[test]
fn a_corrupt_index_is_discarded_and_rebuilt() {
    // P2-003: "handle index corruption gracefully". The index is derived and
    // lives in the cache, so throwing it away is the correct response rather
    // than a desperate one.
    let library = Library::new();
    library.put("a.html", "Alpha", "alpha content", 0);
    library.index();

    // Overwrite Tantivy's metadata with something that cannot be parsed.
    let meta = library.paths.index_dir().join("meta.json");
    assert!(meta.exists(), "expected tantivy's meta.json");
    std::fs::write(&meta, b"{ this is not valid index metadata").expect("corrupt the index");

    let report = library.index();
    assert!(report.rebuilt, "a corrupt index should be rebuilt");
    assert_eq!(report.added, 1);
    assert_eq!(library.search("alpha").len(), 1);
}

#[test]
fn a_page_missing_from_the_store_does_not_abort_the_sync() {
    // The index is derived; one unreadable page must not stop the rest of the
    // library from becoming searchable.
    let library = Library::new();
    library.put("a.html", "Alpha", "alpha content", 0);
    library.put("b.html", "Beta", "beta content", 1);

    // Remove the content but leave the database row, which is what an
    // interrupted write or a partially-evicted cache looks like.
    PageStore::new(&library.paths, &library.source)
        .remove(&PagePath::new("a.html").expect("path"))
        .expect("remove stored page");

    let report = library.index();
    assert_eq!(report.added, 2, "both pages are accounted for");
    assert_eq!(
        library.search("content"),
        ["b.html"],
        "only the readable page is searchable"
    );
}

#[test]
fn two_sources_may_hold_the_same_path() {
    // Why deletion keys on (source, path) rather than path alone. `index.html`
    // exists in almost every source; deleting by path would empty the others.
    let library = Library::new();
    let other = SourceId::new("cargo").expect("source id");
    library.paths.ensure_source_dirs(&other).expect("dirs");

    let database = Database::open(&library.paths).expect("db");
    database
        .upsert_source(&tome_core::model::Source::new(
            other.clone(),
            "Cargo",
            tome_core::model::SourceType::Generic,
        ))
        .expect("upsert");

    library.put("index.html", "Python", "python landing page", 0);
    let store = PageStore::new(&library.paths, &other);
    store
        .write(&StoredPage {
            path: PagePath::new("index.html").expect("path"),
            title: "Cargo".to_owned(),
            description: None,
            body: Node::Document {
                children: vec![Node::Paragraph {
                    children: vec![Node::Text {
                        value: "cargo landing page".to_owned(),
                    }],
                }],
            },
        })
        .expect("write");
    database
        .upsert_page(
            &Page::new(
                other.clone(),
                PagePath::new("index.html").expect("path"),
                "Cargo",
                ContentHash::from_digest(tome_core::hash::sha256(b"cargo landing page")),
            ),
            0,
        )
        .expect("upsert page");

    library.index();
    index_source(&library.paths, &other, "Rust", &mut |_| {}).expect("index cargo");
    assert_eq!(library.engine().len().expect("len"), 2);

    // Change only the Python page. The Cargo page shares its path and must
    // survive.
    library.put("index.html", "Python", "python landing page, revised", 0);
    library.index();

    assert_eq!(
        library.engine().len().expect("len"),
        2,
        "updating one source's index.html must not delete the other's"
    );
    // Single terms, not phrases: the query parser ORs, so "cargo landing"
    // would match both pages through "landing" and prove nothing.
    assert_eq!(library.search("cargo").len(), 1);
    assert_eq!(library.search("revised").len(), 1);
}

#[test]
fn progress_is_reported_for_the_pages_actually_written() {
    let library = Library::new();
    library.put("a.html", "Alpha", "alpha", 0);
    library.put("b.html", "Beta", "beta", 1);

    let mut seen = Vec::new();
    index_source(&library.paths, &library.source, "Python", &mut |progress| {
        if let tome_core::pipeline::Progress::Indexing { indexed, total } = progress {
            seen.push((indexed, total));
        }
    })
    .expect("index");
    assert_eq!(seen, [(1, 2), (2, 2)]);

    // A no-op sync reports nothing, because it writes nothing.
    let mut seen = Vec::new();
    index_source(&library.paths, &library.source, "Python", &mut |progress| {
        if let tome_core::pipeline::Progress::Indexing { .. } = progress {
            seen.push(progress);
        }
    })
    .expect("index");
    assert!(seen.is_empty());
}
