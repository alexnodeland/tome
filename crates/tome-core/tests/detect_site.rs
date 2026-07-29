//! Detection over a real HTTP round trip (S2-10, spec P2-014).
//!
//! `tests/detection.rs` scores the classifier against 128 committed
//! homepages; this asserts the other half — that `detect_site` gets a page
//! from a server and hands the right bytes to it, through the ordinary
//! [`Fetcher`](tome_core::fetch::Fetcher).
//!
//! Which matters because detection is the **first** thing Tome does to a site
//! a user has named, and therefore exactly the wrong place to skip
//! `robots.txt`, the rate limiter or the SSRF guard. A detector that reached
//! for `ureq` directly would look identical in the corpus harness and be
//! wrong in the product.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::Duration;

use tome_core::config::FetchConfig;
use tome_core::detect::{detect_site, Platform};
use tome_core::fetch::Fetcher;
use tome_testkit::{FixtureServer, Scripted};

fn fetcher() -> Fetcher {
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

fn url(server: &FixtureServer, path: &str) -> url::Url {
    url::Url::parse(&format!("{}{path}", server.url())).expect("url")
}

const SPHINX_HOME: &str = r#"<!doctype html><html><head>
<title>Fixture documentation</title>
<script src="_static/documentation_options.js"></script>
</head><body><div class="sphinxsidebar"></div></body></html>"#;

#[test]
fn detects_a_platform_over_http() {
    let server = FixtureServer::serve(std::env::temp_dir()).expect("server");
    server.script("/", Scripted::ok("text/html; charset=utf-8", SPHINX_HOME));

    let detection = detect_site(&fetcher(), &url(&server, "/")).expect("detect");
    assert_eq!(detection.platform, Platform::Sphinx);
    assert!(detection.is_confident());
}

#[test]
fn a_site_with_no_markers_is_admitted_rather_than_guessed() {
    let server = FixtureServer::serve(std::env::temp_dir()).expect("server");
    server.script(
        "/",
        Scripted::ok("text/html", "<html><body><h1>Acme</h1></body></html>"),
    );

    let detection = detect_site(&fetcher(), &url(&server, "/")).expect("detect");
    assert_eq!(detection.platform, Platform::Generic);
    assert!(
        !detection.is_confident(),
        "a marketing page must not be a confident classification"
    );
}

#[test]
fn detection_follows_redirects_and_classifies_where_it_lands() {
    // `docs.example.com` redirecting to `example.readthedocs.io` is the normal
    // shape of a custom documentation domain. Classifying the 301 body would
    // classify nothing.
    let server = FixtureServer::serve(std::env::temp_dir()).expect("server");
    server.script("/docs", Scripted::redirect(301, "/real/"));
    server.script("/real/", Scripted::ok("text/html", SPHINX_HOME));

    let detection = detect_site(&fetcher(), &url(&server, "/docs")).expect("detect");
    assert_eq!(detection.platform, Platform::Sphinx);
}

#[test]
fn robots_txt_is_honoured_before_the_first_look() {
    // Detection is the first request Tome makes to a site somebody named, and
    // "I was only checking what it is" is not an exemption.
    let server = FixtureServer::serve(std::env::temp_dir()).expect("server");
    server.script(
        "/robots.txt",
        Scripted::ok("text/plain", "User-agent: *\nDisallow: /\n"),
    );
    server.script("/", Scripted::ok("text/html", SPHINX_HOME));

    assert!(
        detect_site(&fetcher(), &url(&server, "/")).is_err(),
        "a disallowed path must not be fetched to see what it is"
    );
}

#[test]
fn a_page_that_is_not_valid_utf8_is_still_classified() {
    // A mis-declared charset is common in old documentation. Refusing to look
    // would turn it into "this site cannot be detected", which is a worse
    // answer than the right one.
    let server = FixtureServer::serve(std::env::temp_dir()).expect("server");
    let mut body = SPHINX_HOME.as_bytes().to_vec();
    body.extend_from_slice(&[0xff, 0xfe, 0xfd]);
    server.script("/", Scripted::ok("text/html", body));

    let detection = detect_site(&fetcher(), &url(&server, "/")).expect("detect");
    assert_eq!(detection.platform, Platform::Sphinx);
}

#[test]
fn a_server_error_is_an_error_rather_than_a_generic_site() {
    // Reporting `Generic` for a 500 would tell the user their documentation
    // site is unremarkable, when what happened is that it did not answer.
    let server = FixtureServer::serve(std::env::temp_dir()).expect("server");
    server.script("/", Scripted::new(500));

    assert!(detect_site(&fetcher(), &url(&server, "/")).is_err());
}
