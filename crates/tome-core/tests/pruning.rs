//! Pruning pages the site no longer has (S4-1; policy agreed 2026-07-29).
//!
//! **The guard is the entire feature, and it is the half that fails
//! silently.** Deleting pages a clean crawl did not see is easy and correct.
//! Deleting pages an *interrupted* crawl did not see would delete a user's
//! library a few hundred pages at a time — and it would look exactly like
//! working software, because the pages that survive are the ones the crawl
//! reached. So the tests here run in pairs: one that prunes, and one that must
//! not.
//!
//! Against the real fixture server through the real `pipeline::pull`, because
//! the condition is a property of a *crawl outcome*, and a unit test of the
//! filter would assert the easy half of the logic and none of the wiring.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::Path;

use tome_core::config::SourceConfig;
use tome_core::db::Database;
use tome_core::model::SourceId;
use tome_core::pipeline::{self, IngestReport};
use tome_core::Paths;
use tome_testkit::{FixtureServer, Scripted};

struct Library {
    _dir: tempfile::TempDir,
    paths: Paths,
    source: SourceId,
}

fn library() -> Library {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = Paths::under_root(dir.path().to_path_buf());
    paths.ensure_created().expect("roots");
    Library {
        _dir: dir,
        paths,
        source: SourceId::new("fixture").expect("source id"),
    }
}

fn config(server: &FixtureServer, max_pages: Option<u32>) -> SourceConfig {
    let yaml = format!(
        "schema_version: 1\n\
         name: Fixture\n\
         source:\n  type: generic\n  url: {}\n  generic:\n    entry_points: [\"/\"]\n\
         fetch:\n  allow_insecure: true\n  rate_limit_rps: 4\n",
        server.url(),
    );
    let mut config = SourceConfig::parse_str(
        SourceId::new("fixture").unwrap(),
        &yaml,
        Path::new("fixture.yaml"),
    )
    .expect("fixture config parses");
    if let Some(cap) = max_pages {
        config.cap_pages(cap);
    }
    config
}

fn pull(paths: &Paths, config: &SourceConfig) -> IngestReport {
    pipeline::pull(paths, config, &mut |_| {}).expect("pull")
}

/// Every page path the database currently holds.
fn stored_paths(paths: &Paths, source: &SourceId) -> BTreeSet<String> {
    Database::open(paths)
        .expect("db")
        .list_pages(source)
        .expect("list")
        .into_iter()
        .map(|page| page.path.to_string())
        .collect()
}

/// Whether a page's stored content is still on disk.
fn stored_file_exists(paths: &Paths, source: &SourceId, path: &str) -> bool {
    tome_core::store::PageStore::new(paths, source)
        .read(&tome_core::model::PagePath::new(path).expect("path"))
        .expect("read")
        .is_some()
}

#[test]
fn a_clean_crawl_removes_pages_the_site_no_longer_has() {
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let library = library();
    let config = config(&server, None);

    let first = pull(&library.paths, &config);
    assert_eq!(first.pages_pruned, 0, "nothing to prune on a first pull");
    let before = stored_paths(&library.paths, &library.source);
    assert!(
        before.len() >= 2,
        "the fixture should produce several pages: {before:?}"
    );

    // The site drops a page. 404 rather than an error status: a page that is
    // *gone* is the case pruning exists for, and 404 is not a crawl error.
    let vanished = before
        .iter()
        .find(|path| path.contains("reference"))
        .cloned()
        .expect("the fixture has an api/reference page");
    server.script("/api/reference.html", Scripted::new(404));

    let second = pull(&library.paths, &config);
    // The 404 IS reported — a link pointing at a page that is gone is worth
    // knowing about — but it does not block pruning. It is the evidence.
    assert_eq!(second.page_errors.len(), 1, "{:?}", second.page_errors);
    assert!(second.page_errors[0].contains("404"));
    assert!(!second.hit_page_cap);
    assert_eq!(second.pages_pruned, 1, "the vanished page should have gone");

    let after = stored_paths(&library.paths, &library.source);
    assert!(
        !after.contains(&vanished),
        "{vanished} is still in the database"
    );
    assert!(
        !stored_file_exists(&library.paths, &library.source, &vanished),
        "the stored file outlived its row"
    );
    // And nothing else went with it.
    assert_eq!(
        after.len(),
        before.len() - 1,
        "before {before:?} after {after:?}"
    );
}

#[test]
fn a_capped_crawl_deletes_nothing() {
    // This is the failure that would look like working software: the crawl
    // stopped at `max_pages`, so most of the site was never *looked at*, and
    // "not seen" is not evidence of "deleted".
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let library = library();

    let full = pull(&library.paths, &config(&server, None));
    let before = stored_paths(&library.paths, &library.source);
    assert!(before.len() > 1, "need more than one page to cap below");
    assert!(!full.hit_page_cap);

    let capped = pull(&library.paths, &config(&server, Some(1)));
    assert!(capped.hit_page_cap, "the cap should have been hit");
    assert_eq!(
        capped.pages_pruned, 0,
        "a capped crawl must delete nothing at all"
    );
    assert_eq!(
        stored_paths(&library.paths, &library.source),
        before,
        "the library changed under a capped crawl"
    );
}

#[test]
fn an_errored_crawl_deletes_nothing() {
    // The other half of the guard. A crawl that saw a subset with holes in it
    // is not evidence about the pages in the holes.
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let library = library();
    let config = config(&server, None);

    pull(&library.paths, &config);
    let before = stored_paths(&library.paths, &library.source);

    // 500, not 404: a server error is a page the crawl could not read, which
    // is exactly the ambiguity the guard exists for.
    server.script("/api/reference.html", Scripted::new(500));

    let errored = pull(&library.paths, &config);
    assert!(
        !errored.page_errors.is_empty(),
        "the crawl should have reported an error"
    );
    assert_eq!(
        errored.pages_pruned, 0,
        "an errored crawl must delete nothing at all"
    );
    assert_eq!(
        stored_paths(&library.paths, &library.source),
        before,
        "the library changed under an errored crawl"
    );
}

#[test]
fn a_pruned_page_leaves_the_search_index_too() {
    // The index is reconciled from the database after pruning, so a removed
    // page must stop being findable. Without this, search returns a hit that
    // `read_page` then cannot open.
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let library = library();
    let config = config(&server, None);

    pull(&library.paths, &config);
    let engine = tome_core::search::SearchEngine::open(&library.paths).expect("index");
    let indexed_before = engine
        .indexed_pages(&library.source)
        .expect("indexed pages")
        .len();
    drop(engine);

    server.script("/api/reference.html", Scripted::new(404));
    let second = pull(&library.paths, &config);
    assert_eq!(second.pages_pruned, 1);

    let engine = tome_core::search::SearchEngine::open(&library.paths).expect("index");
    assert_eq!(
        engine
            .indexed_pages(&library.source)
            .expect("indexed pages")
            .len(),
        indexed_before - 1,
        "the pruned page is still in the index"
    );
}

#[test]
fn a_crawl_that_found_nothing_deletes_nothing() {
    // The disaster case, and the one the other two guards do not catch. A
    // site that has moved, an entry point that 404s, a captive portal: each
    // is a crawl that "completed cleanly" with no ambiguous errors and found
    // zero pages. Without the produced-something guard, that empties the
    // library — every page is "not seen this run", and a 404 on the entry
    // point is not an ambiguous error.
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let library = library();
    let config = config(&server, None);

    pull(&library.paths, &config);
    let before = stored_paths(&library.paths, &library.source);
    assert!(!before.is_empty());

    // The whole site is gone.
    server.script("/", Scripted::new(404));
    server.script("/index.html", Scripted::new(404));

    let empty = pull(&library.paths, &config);
    assert_eq!(empty.pages_stored, 0, "the crawl should have found nothing");
    assert_eq!(
        empty.pages_pruned, 0,
        "a crawl that found nothing must delete nothing"
    );
    assert_eq!(
        stored_paths(&library.paths, &library.source),
        before,
        "the library was emptied by a site that went away"
    );
}
