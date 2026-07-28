//! The BFS crawler over the fixture server (S1-6).
//!
//! The crawler inherits politeness and the SSRF filter from the Fetcher, so
//! these tests are about the crawl itself: BFS coverage, depth limits, the
//! page cap, visited-dedup, scope filtering, and that one bad page does not
//! sink the run.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;
use std::time::Duration;

use tome_core::config::{FetchConfig, SourceConfig};
use tome_core::crawl::Crawler;
use tome_core::fetch::Fetcher;
use tome_core::model::SourceId;
use tome_testkit::{FixtureServer, Scripted};

fn fast_fetcher() -> Fetcher {
    // allow_insecure: the fixture server is loopback http, which the SSRF
    // filter blocks by default (an owned host is exactly the exception).
    let config = FetchConfig {
        rate_limit_rps: 1000.0,
        allow_insecure: true,
        timeout: Duration::from_millis(800),
        ..FetchConfig::default()
    };
    Fetcher::with_backoff_base(config, Duration::from_millis(5))
}

/// A generic-scraper config pointed at the fixture server. `extra` is
/// appended under `source.generic:` (entry_points, patterns).
fn config_for(server: &FixtureServer, extra: &str) -> SourceConfig {
    let yaml = format!(
        "schema_version: 1\n\
         name: Fixture\n\
         source:\n  type: generic\n  url: {}\n  generic:\n{}\n\
         fetch:\n  allow_insecure: true\n",
        server.url(),
        extra,
    );
    SourceConfig::parse_str(
        SourceId::new("fixture").unwrap(),
        &yaml,
        Path::new("fixture.yaml"),
    )
    .expect("fixture config parses")
}

fn crawl(fetcher: &Fetcher, config: &SourceConfig) -> tome_core::crawl::CrawlOutcome {
    let crawler = Crawler::new(fetcher, config).expect("generic source is crawlable");
    let mut last_progress = None;
    let outcome = crawler.crawl(&mut |p| last_progress = Some(p));
    // Progress must have been reported at least once for a non-empty crawl.
    if !outcome.docset.pages.is_empty() {
        assert!(
            last_progress.is_some(),
            "progress callback was never called"
        );
    }
    outcome
}

#[test]
fn crawls_the_whole_fixture_site_following_links() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();
    // Content selector matches the fixture's <div class="document">.
    let config = config_for(&server, "    content_selector: \"div.document\"");

    let outcome = crawl(&fetcher, &config);

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert!(!outcome.hit_page_cap);

    // The fixture's index links to api/reference.html and guide/index.html;
    // BFS should reach all three (and not the robots-disallowed /private/).
    let paths: Vec<&str> = outcome
        .docset
        .pages
        .iter()
        .map(|p| p.meta.path.as_str())
        .collect();
    assert!(paths.iter().any(|p| p.contains("index.html")));
    assert!(
        paths.iter().any(|p| p.contains("reference.html")),
        "should have followed the link to the API reference, got {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.contains("private")),
        "robots-disallowed pages must not be crawled: {paths:?}"
    );
}

#[test]
fn each_page_is_fetched_once_despite_multiple_inbound_links() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();
    let config = config_for(&server, "    content_selector: \"div.document\"");

    crawl(&fetcher, &config);

    // The index page is linked from itself and from sub-pages (nav), but must
    // be fetched exactly once.
    assert_eq!(
        server.requests_for("/index.html").len(),
        1,
        "visited-dedup failed: index.html fetched more than once"
    );
}

#[test]
fn depth_one_fetches_only_the_entry_page() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();
    // Entry pages are depth 1, so max_depth 1 (the config floor) means the
    // entry point and nothing it links to.
    let config = config_for(
        &server,
        "    max_depth: 1\n    content_selector: \"div.document\"",
    );

    let outcome = crawl(&fetcher, &config);
    assert_eq!(
        outcome.docset.pages.len(),
        1,
        "max_depth 1 must fetch only the entry point"
    );
}

#[test]
fn the_page_cap_stops_the_crawl_and_is_reported() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();
    let config = config_for(
        &server,
        "    max_pages: 1\n    content_selector: \"div.document\"",
    );

    let outcome = crawl(&fetcher, &config);
    assert_eq!(outcome.docset.pages.len(), 1);
    assert!(
        outcome.hit_page_cap,
        "hitting the cap must be reported, not silently truncated"
    );
}

#[test]
fn a_broken_link_is_recorded_not_fatal() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    // Make one linked page 500 so the crawl hits an error but continues.
    server.script("/api/reference.html", Scripted::new(500));
    let fetcher = fast_fetcher();
    let config = config_for(&server, "    content_selector: \"div.document\"");

    let outcome = crawl(&fetcher, &config);
    // The rest of the site still came through.
    assert!(!outcome.docset.pages.is_empty());
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.url.path().contains("reference.html")),
        "the failing page should be in the error list"
    );
}

#[test]
fn exclude_patterns_keep_a_subtree_out_of_the_crawl() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();
    // Exclude the guide subtree; it must never be requested.
    let config = config_for(
        &server,
        "    exclude_patterns: [\"^/guide/\"]\n    content_selector: \"div.document\"",
    );

    crawl(&fetcher, &config);
    assert!(
        server.requests_for("/guide/index.html").is_empty(),
        "an excluded path must never be fetched"
    );
}

#[test]
fn a_non_crawlable_source_type_yields_no_crawler() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let yaml = "schema_version: 1\nname: Local\nsource:\n  type: local\n  path: /docs\n";
    let config = SourceConfig::parse_str(
        SourceId::new("local").unwrap(),
        yaml,
        Path::new("local.yaml"),
    )
    .unwrap();
    let fetcher = fast_fetcher();
    assert!(
        Crawler::new(&fetcher, &config).is_none(),
        "local sources are not crawled"
    );
    let _ = server; // keep the server alive for symmetry
}
