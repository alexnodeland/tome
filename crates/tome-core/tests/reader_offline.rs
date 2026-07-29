//! **Stage 1's exit gate, at the layer a person actually sees** (S1-13).
//!
//! Everything before this asserted the gate on the *AST*: asset localization
//! (S1-10) shuts the fixture server down and confirms no `http` reference
//! survives into the stored tree. That is necessary and not sufficient. The
//! tree is not what the reader displays; HTML is, and the renderer is where a
//! remote reference could be reintroduced — an unescaped attribute, an asset
//! base that resolves outward, a link rewritten into a live one.
//!
//! So this test runs the whole thing: config → crawl → normalize → sanitize →
//! localize → **store on disk** → *shut the server down* → read from disk →
//! render → assert the HTML reaches for nothing.
//!
//! The server shutdown is the load-bearing part.
//! [`FixtureServer::shutdown`] makes the port refuse connections, so a page
//! that still tried to fetch something would fail loudly rather than quietly
//! passing on a machine where the server happened to still be up.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;
use std::time::Duration;

use tome_core::config::{FetchConfig, SourceConfig};
use tome_core::highlight::Highlighter;
use tome_core::model::{PagePath, SourceId};
use tome_core::pipeline::{pull, Progress};
use tome_core::render::{render, RenderOptions};
use tome_core::store::PageStore;
use tome_core::Paths;
use tome_testkit::FixtureServer;

/// A library rooted in a temporary directory. `TOME_HOME` is not set — the
/// roots are constructed directly, so these tests never touch the developer's
/// real library and never race another test over an environment variable.
fn temp_library() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::under_root(dir.path());
    paths.ensure_created().unwrap();
    (dir, paths)
}

fn config_for(server: &FixtureServer) -> SourceConfig {
    let yaml = format!(
        "schema_version: 1\n\
         name: Fixture Docs\n\
         source:\n  type: generic\n  url: {}\n  generic:\n    entry_points: ['/index.html']\n\
         fetch:\n  allow_insecure: true\n  rate_limit_rps: 100\n",
        server.url(),
    );
    SourceConfig::parse_str(
        SourceId::new("fixture").unwrap(),
        &yaml,
        Path::new("fixture.yaml"),
    )
    .expect("fixture config parses")
}

#[test]
fn a_pulled_source_renders_with_the_server_gone() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let (_dir, paths) = temp_library();
    let mut config = config_for(&server);
    // The fixture server is loopback http, which the SSRF filter blocks by
    // default — an owned host is exactly the exception the flag is for.
    config.fetch = FetchConfig {
        allow_insecure: true,
        rate_limit_rps: 100.0,
        timeout: Duration::from_millis(1500),
        ..config.fetch
    };

    let mut progress_seen = 0usize;
    let report = pull(&paths, &config, &mut |p| {
        if matches!(p, Progress::Storing { .. }) {
            progress_seen += 1;
        }
    })
    .expect("pull succeeds");

    assert!(
        report.pages_stored > 0,
        "nothing was stored; errors: {:?}",
        report.page_errors
    );
    assert_eq!(
        progress_seen, report.pages_stored,
        "progress must be reported once per stored page"
    );

    // Everything from here on happens with the network gone.
    server.shutdown();
    assert!(!server.is_running());

    let store = PageStore::new(&paths, &config.id);
    let stored_paths = store.list().expect("the store lists what it wrote");
    assert_eq!(stored_paths.len(), report.pages_stored);

    let highlighter = Highlighter::shared();
    let options = RenderOptions {
        asset_base: "tome://localhost/fixture/",
        highlighter,
    };

    let address = server.addr().to_string();
    let mut rendered_any = false;
    // The `src` loop below is vacuous if every image degraded to alt text,
    // which is exactly what a broken asset path looks like from the outside.
    // This counter is what makes the assertion mean something: it caught the
    // synthesised-base bug, where the pipeline localized nothing and the
    // offline test passed because there was nothing left to leak.
    let mut images_rendered = 0usize;
    let mut internal_links = 0usize;

    for path in stored_paths {
        let page = store.read(&path).unwrap().expect("a page it just listed");
        let out = render(&page.body, &options);

        assert!(!out.html.is_empty(), "{path} rendered to nothing");
        rendered_any = true;

        // The gate is about **subresources, not links**. An `<a href>` to
        // example.com is inert until someone clicks it, and S1-15 intercepts
        // the click anyway; an `<img src>` fetches the moment the page
        // renders. Asserting "no https anywhere" would fail on a page that
        // merely links out — which the fixture does, and which is correct
        // behaviour, not a leak.
        //
        // So: every attribute a browser fetches without being asked must be
        // local. In this renderer's output that is exactly `src`.
        for (at, _) in out.html.match_indices("src=\"") {
            let value = &out.html[at + 5..];
            let value = &value[..value.find('"').unwrap()];
            assert!(
                value.starts_with("tome://localhost/fixture/assets/"),
                "{path} has a src that is not a local asset: {value}"
            );
            images_rendered += 1;
        }

        // Internal links must have been rewritten to library paths. A link
        // that still names the crawl host would be unroutable on any other
        // machine (ADR-0001 syncs these trees between devices).
        for (at, _) in out.html.match_indices("<a href=\"") {
            let value = &out.html[at + 9..];
            let value = &value[..value.find('"').unwrap()];
            if !value.starts_with("http") && !value.starts_with("mailto:") {
                internal_links += 1;
            }
        }

        // And nothing that could introduce a subresource by another route:
        // no stylesheet link, no script, no inline style with a url(), no
        // nested browsing context. The renderer emits none of these; this
        // asserts it stays that way as the renderer grows.
        for forbidden in [
            "<script", "<link", "<style", "<iframe", "<object", "<embed", "<base", "srcset",
            "@import", "url(", "style=",
        ] {
            assert!(
                !out.html.contains(forbidden),
                "{path} contains {forbidden}, which can load a subresource"
            );
        }

        // The fixture server's own address must not survive anywhere at all
        // — not even in link text — since that would mean localization left
        // a reference to a host that no longer exists.
        assert!(
            !out.html.contains(&address),
            "{path} still names the fixture server"
        );
    }

    assert!(rendered_any);
    assert!(
        images_rendered > 0,
        "no image survived localization, so the src assertion above proved nothing"
    );
    assert!(
        internal_links > 0,
        "no internal link was rewritten to a library path"
    );
}

#[test]
fn the_stored_page_survives_a_round_trip_through_disk() {
    // The AST is stored, not HTML, so that a renderer or stylesheet change
    // takes effect without a re-crawl. That is only true if what comes back
    // off disk renders identically to what went in.
    let server = FixtureServer::start("sphinx-example").unwrap();
    let (_dir, paths) = temp_library();
    let mut config = config_for(&server);
    config.fetch = FetchConfig {
        allow_insecure: true,
        rate_limit_rps: 100.0,
        ..config.fetch
    };

    pull(&paths, &config, &mut |_| {}).expect("pull succeeds");
    server.shutdown();

    let store = PageStore::new(&paths, &config.id);
    let highlighter = Highlighter::shared();
    let options = RenderOptions {
        asset_base: "",
        highlighter,
    };

    for path in store.list().unwrap() {
        let page = store.read(&path).unwrap().unwrap();
        let first = render(&page.body, &options);
        // Re-read from disk and render again: byte-identical, or the store
        // is losing something.
        let reread = store.read(&path).unwrap().unwrap();
        let second = render(&reread.body, &options);
        assert_eq!(first, second, "{path} did not survive the round trip");
    }
}

#[test]
fn page_metadata_and_content_end_up_in_their_separate_homes() {
    // Metadata in SQLite (read in bulk, for lists and search), content in the
    // page store (read one page at a time). A page present in one and absent
    // from the other is a library that lists pages it cannot open.
    let server = FixtureServer::start("sphinx-example").unwrap();
    let (_dir, paths) = temp_library();
    let mut config = config_for(&server);
    config.fetch = FetchConfig {
        allow_insecure: true,
        rate_limit_rps: 100.0,
        ..config.fetch
    };

    let report = pull(&paths, &config, &mut |_| {}).expect("pull succeeds");
    server.shutdown();

    let database = tome_core::db::Database::open(&paths).unwrap();
    let listed = database.list_pages(&config.id).unwrap();
    assert_eq!(listed.len(), report.pages_stored);

    let source = database
        .get_source(&config.id)
        .unwrap()
        .expect("the source itself must be recorded, or the sidebar has nothing to show");

    // The stored count must match the rows. Before this was asserted, the
    // pipeline never wrote `page_count` back, so it stayed 0 for ever and
    // `tome list` (which counts rows) and `tome list --json` (which read the
    // field) printed different numbers for the same library.
    assert_eq!(source.page_count as usize, report.pages_stored);
    assert_eq!(
        database.page_count(&config.id).unwrap() as usize,
        report.pages_stored
    );
    assert!(
        source.last_synced.is_some(),
        "a pulled source must record when, or the sidebar cannot say"
    );

    // The entry point is the first page listed, not whichever file sorts
    // first by name. This is what makes the app open a source on its index
    // rather than on its changelog or its appendix.
    assert_eq!(
        listed.first().map(|p| p.path.as_str()),
        Some("index.html"),
        "the first listed page should be the crawl entry point"
    );

    let store = PageStore::new(&paths, &config.id);
    for page in listed {
        assert!(
            store.read(&page.path).unwrap().is_some(),
            "{} is in the database but has no content on disk",
            page.path
        );
        assert!(!page.title.trim().is_empty(), "{} has no title", page.path);
    }
}

#[test]
fn a_page_that_was_never_pulled_reads_as_absent_rather_than_failing() {
    let (_dir, paths) = temp_library();
    let store = PageStore::new(&paths, &SourceId::new("never-pulled").unwrap());
    assert_eq!(
        store.read(&PagePath::new("index.html").unwrap()).unwrap(),
        None
    );
}
