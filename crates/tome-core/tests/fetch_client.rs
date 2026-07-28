//! The HTTP client against the fixture server (S1-4) — every etiquette rule
//! the PRD calls non-negotiable, exercised over a real socket.
//!
//! This is what `tome-testkit` was built for in S0: the server misbehaves on
//! purpose (scripted 429s, redirect loops, robots outages) so this client's
//! manners can be asserted rather than assumed.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::{Duration, Instant};

use tome_core::config::FetchConfig;
use tome_core::fetch::{FetchOutcome, Fetched, Fetcher, Validators};
use tome_core::Error;
use tome_testkit::{FixtureServer, Scripted};
use url::Url;

/// Fast test config: effectively no rate limit, millisecond backoff.
///
/// `allow_insecure` is set because the fixture server runs on `127.0.0.1`,
/// and the SSRF filter (S1-5) blocks loopback under the default policy —
/// which is correct: a loopback fixture is exactly the "a host you own"
/// case that `allow_insecure` exists for. The SSRF behaviour itself is
/// tested in `ssrf_filter.rs`, not here.
fn fast_fetcher() -> Fetcher {
    let config = FetchConfig {
        rate_limit_rps: 1000.0,
        allow_insecure: true,
        ..FetchConfig::default()
    };
    Fetcher::with_backoff_base(config, Duration::from_millis(5))
}

fn page_url(server: &FixtureServer, path: &str) -> Url {
    server.url_for(path).parse().unwrap()
}

fn expect_fetched(outcome: FetchOutcome) -> Box<Fetched> {
    match outcome {
        FetchOutcome::Fetched(fetched) => fetched,
        FetchOutcome::NotModified => panic!("expected content, got NotModified"),
    }
}

const MB: u64 = 1024 * 1024;

// ---- identity and conditional GET ------------------------------------------

#[test]
fn sends_the_honest_user_agent_on_every_request() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();

    let fetched = expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/index.html"), MB, None)
            .unwrap(),
    );
    assert_eq!(fetched.status, 200);
    assert!(!fetched.body.is_empty());

    // Every request — including the robots.txt fetch — identifies the tool
    // and carries the project URL, per the PRD's crawl etiquette table.
    for request in server.requests() {
        let ua = request.header("user-agent").expect("UA header present");
        assert!(ua.starts_with("Tome/"), "got UA {ua:?}");
        assert!(ua.contains("+https://"), "got UA {ua:?}");
    }
}

#[test]
fn conditional_refetch_returns_not_modified() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();
    let url = page_url(&server, "/index.html");

    let first = expect_fetched(fetcher.fetch(&url, MB, None).unwrap());
    assert!(first.etag.is_some(), "fixture server serves ETags");

    let validators = Validators {
        etag: first.etag.clone(),
        last_modified: first.last_modified.clone(),
    };
    match fetcher.fetch(&url, MB, Some(&validators)).unwrap() {
        FetchOutcome::NotModified => {}
        FetchOutcome::Fetched(_) => panic!("expected 304 NotModified on matching validators"),
    }
}

// ---- robots.txt ---------------------------------------------------------------

#[test]
fn robots_disallow_blocks_before_any_request_is_made() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();

    let err = fetcher
        .fetch(&page_url(&server, "/private/internal.html"), MB, None)
        .unwrap_err();
    assert!(matches!(err, Error::BlockedByRobots));

    // The disallowed URL was never requested — blocking after fetching
    // would be theatre.
    assert!(server.requests_for("/private/internal.html").is_empty());
}

#[test]
fn unreachable_robots_means_disallow() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    server.script("/robots.txt", Scripted::new(500));
    let fetcher = fast_fetcher();

    let err = fetcher
        .fetch(&page_url(&server, "/index.html"), MB, None)
        .unwrap_err();
    assert!(matches!(err, Error::BlockedByRobots));
    assert!(server.requests_for("/index.html").is_empty());
}

#[test]
fn absent_robots_means_allow() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("page.html"), "<h1>hi</h1>").unwrap();
    let server = FixtureServer::serve(dir.path()).unwrap();
    let fetcher = fast_fetcher();

    let fetched = expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/page.html"), MB, None)
            .unwrap(),
    );
    assert_eq!(fetched.body, b"<h1>hi</h1>");
}

#[test]
fn robots_is_fetched_once_per_origin_not_per_page() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();

    for path in ["/index.html", "/guide/index.html", "/api/reference.html"] {
        expect_fetched(fetcher.fetch(&page_url(&server, path), MB, None).unwrap());
    }
    assert_eq!(server.requests_for("/robots.txt").len(), 1);
}

#[test]
fn crawl_delay_stretches_the_rate_limit() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    server.script(
        "/robots.txt",
        Scripted::ok("text/plain", "User-agent: *\nCrawl-delay: 0.3\n"),
    );
    let fetcher = fast_fetcher(); // configured rate would allow 1000/s

    let start = Instant::now();
    expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/index.html"), MB, None)
            .unwrap(),
    );
    expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/guide/index.html"), MB, None)
            .unwrap(),
    );
    // Second page fetch must wait out the crawl delay.
    assert!(
        start.elapsed() >= Duration::from_millis(250),
        "crawl-delay was not honoured: {:?}",
        start.elapsed()
    );
}

#[test]
fn configured_rate_limit_spaces_requests() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let config = FetchConfig {
        rate_limit_rps: 4.0,  // the cap: 250ms between requests to one host
        allow_insecure: true, // loopback fixture — see fast_fetcher
        ..FetchConfig::default()
    };
    let fetcher = Fetcher::with_backoff_base(config, Duration::from_millis(5));

    let start = Instant::now();
    expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/index.html"), MB, None)
            .unwrap(),
    );
    expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/guide/index.html"), MB, None)
            .unwrap(),
    );
    // robots + page1 + page2 = at least two full intervals before the last
    // request completes.
    assert!(
        start.elapsed() >= Duration::from_millis(400),
        "requests were not rate limited: {:?}",
        start.elapsed()
    );
}

// ---- retries ---------------------------------------------------------------------

#[test]
fn retries_429_honouring_retry_after_then_succeeds() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    server.script(
        "/index.html",
        Scripted::new(429).header("Retry-After", "0").times(1),
    );
    let fetcher = fast_fetcher();

    let fetched = expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/index.html"), MB, None)
            .unwrap(),
    );
    assert_eq!(fetched.status, 200);
    assert_eq!(server.requests_for("/index.html").len(), 2);
}

#[test]
fn retries_5xx_with_backoff_then_succeeds() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    server.script("/index.html", Scripted::new(503).times(2));
    let fetcher = fast_fetcher();

    expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/index.html"), MB, None)
            .unwrap(),
    );
    assert_eq!(server.requests_for("/index.html").len(), 3);
}

#[test]
fn persistent_5xx_gives_up_after_bounded_attempts() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    server.script("/index.html", Scripted::new(500));
    let fetcher = fast_fetcher();

    let err = fetcher
        .fetch(&page_url(&server, "/index.html"), MB, None)
        .unwrap_err();
    assert!(matches!(err, Error::Http { status: 500 }));
    assert_eq!(
        server.requests_for("/index.html").len(),
        4,
        "1 attempt + 3 retries, no more"
    );
}

#[test]
fn client_errors_are_never_retried() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();

    let err = fetcher
        .fetch(&page_url(&server, "/does-not-exist.html"), MB, None)
        .unwrap_err();
    assert!(matches!(err, Error::Http { status: 404 }));
    assert_eq!(server.requests_for("/does-not-exist.html").len(), 1);
}

// ---- redirects --------------------------------------------------------------------

#[test]
fn redirects_are_followed_and_the_final_url_reported() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    server.script("/moved.html", Scripted::redirect(301, "/index.html"));
    let fetcher = fast_fetcher();

    let fetched = expect_fetched(
        fetcher
            .fetch(&page_url(&server, "/moved.html"), MB, None)
            .unwrap(),
    );
    assert!(fetched.final_url.path().ends_with("/index.html"));
    assert_eq!(fetched.status, 200);
}

#[test]
fn a_redirect_into_disallowed_territory_is_blocked() {
    // The reason redirects are followed manually: every hop passes the
    // robots check. A library that chased redirects itself would follow
    // this one straight past the policy.
    let server = FixtureServer::start("sphinx-example").unwrap();
    server.script(
        "/moved.html",
        Scripted::redirect(302, "/private/internal.html"),
    );
    let fetcher = fast_fetcher();

    let err = fetcher
        .fetch(&page_url(&server, "/moved.html"), MB, None)
        .unwrap_err();
    assert!(matches!(err, Error::BlockedByRobots));
    assert!(server.requests_for("/private/internal.html").is_empty());
}

#[test]
fn redirect_loops_end_in_an_error_not_a_hang() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    server.script("/a.html", Scripted::redirect(302, "/b.html"));
    server.script("/b.html", Scripted::redirect(302, "/a.html"));
    let fetcher = fast_fetcher();

    let err = fetcher
        .fetch(&page_url(&server, "/a.html"), MB, None)
        .unwrap_err();
    assert!(matches!(err, Error::Fetch { .. }));
}

// ---- body limits ---------------------------------------------------------------------

#[test]
fn oversized_bodies_are_refused_not_truncated() {
    let server = FixtureServer::start("sphinx-example").unwrap();
    let fetcher = fast_fetcher();

    let err = fetcher
        .fetch(&page_url(&server, "/index.html"), 16, None)
        .unwrap_err();
    assert!(matches!(err, Error::TooLarge { limit: 16 }));
}

// ---- the offline assertion, client half ------------------------------------------------

#[test]
fn a_shut_down_server_fails_loudly() {
    // FixtureServer::shutdown() makes the port refuse connections precisely
    // so that anything still reaching for the network fails instead of
    // quietly passing. The client's side of that bargain: a refused
    // connection is an error after bounded retries, not a hang.
    let server = FixtureServer::start("sphinx-example").unwrap();
    let url = page_url(&server, "/index.html");
    let fetcher = fast_fetcher();
    expect_fetched(fetcher.fetch(&url, MB, None).unwrap());

    server.shutdown();

    let fresh = fast_fetcher(); // fresh robots cache: hits the network for robots
    let err = fresh.fetch(&url, MB, None).unwrap_err();
    // Refused robots fetch reads as unreachable robots → disallow; either
    // that or a transport error is acceptable, silence is not.
    assert!(matches!(err, Error::BlockedByRobots | Error::Fetch { .. }));
}
