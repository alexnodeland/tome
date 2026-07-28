//! Tests for the fixture HTTP server (S0-6).
//!
//! These use a ~50-line HTTP client written here rather than a real one on
//! purpose: the fixture server is what every later test trusts, so its own
//! tests should not be able to pass because a sophisticated client papered
//! over a malformed response. Reading raw bytes off the socket is the point.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use tome_testkit::{FixtureServer, Scripted};

// ---------------------------------------------------------------------------
// Serving files
// ---------------------------------------------------------------------------

#[test]
fn serves_a_committed_fixture_page() {
    let server = FixtureServer::start("sphinx-example").expect("start server");

    let response = get(&server.url_for("/api/reference.html"));

    assert_eq!(response.status, 200);
    assert_eq!(
        response.header("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
    assert!(response.text().contains("API reference"));
    assert_eq!(
        response.header("content-length").unwrap(),
        response.body.len().to_string(),
        "advertised length must match what was actually sent"
    );
}

#[test]
fn serves_assets_with_their_own_content_types() {
    let server = FixtureServer::start("sphinx-example").expect("start server");

    let css = get(&server.url_for("/_static/style.css"));
    assert_eq!(
        css.header("content-type").unwrap(),
        "text/css; charset=utf-8"
    );

    let svg = get(&server.url_for("/_static/logo.svg"));
    assert_eq!(svg.header("content-type").unwrap(), "image/svg+xml");

    // robots.txt is served like any other file. It is not special-cased here,
    // so a crawler test exercises the same path a real fetch would.
    let robots = get(&server.url_for("/robots.txt"));
    assert_eq!(robots.status, 200);
    assert!(robots.text().contains("Disallow: /private/"));
}

#[test]
fn directory_requests_redirect_then_serve_the_index() {
    let server = FixtureServer::start("sphinx-example").expect("start server");

    // Without the redirect, relative links inside /guide/index.html would
    // resolve against / and a crawler would silently fetch the wrong pages.
    let redirect = get(&server.url_for("/guide"));
    assert_eq!(redirect.status, 301);
    assert_eq!(redirect.header("location").unwrap(), "/guide/");

    let indexed = get(&server.url_for("/guide/"));
    assert_eq!(indexed.status, 200);
    assert!(indexed.text().contains("User guide"));
}

#[test]
fn missing_paths_are_404_and_other_methods_are_405() {
    let server = FixtureServer::start("sphinx-example").expect("start server");

    assert_eq!(get(&server.url_for("/nope.html")).status, 404);

    let post = request("POST", &server.url_for("/index.html"), &[]);
    assert_eq!(post.status, 405);
    assert_eq!(post.header("allow").unwrap(), "GET, HEAD");
}

#[test]
fn head_reports_the_length_it_would_have_sent() {
    let server = FixtureServer::start("sphinx-example").expect("start server");

    let body = get(&server.url_for("/index.html"));
    let head = request("HEAD", &server.url_for("/index.html"), &[]);

    assert_eq!(head.status, 200);
    assert!(head.body.is_empty(), "HEAD must not carry a body");
    assert_eq!(
        head.header("content-length").unwrap(),
        body.body.len().to_string()
    );
}

// ---------------------------------------------------------------------------
// Path traversal
// ---------------------------------------------------------------------------

#[test]
fn traversal_out_of_the_fixture_root_is_refused() {
    let server = FixtureServer::start("sphinx-example").expect("start server");

    for target in [
        "/../../Cargo.toml",
        "/api/../../../Cargo.toml",
        // Percent-encoded, because a filter that inspects the raw target before
        // decoding is the classic way this check gets defeated.
        "/%2e%2e/%2e%2e/Cargo.toml",
        "/api/%2e%2e%2f%2e%2e%2fCargo.toml",
    ] {
        let response = raw_get(&server.url(), target);
        assert_eq!(
            response.status, 403,
            "{target} should be refused, got {}",
            response.status
        );
        assert!(!response.text().contains("[workspace]"));
    }
}

// ---------------------------------------------------------------------------
// Conditional GET
// ---------------------------------------------------------------------------

#[test]
fn if_none_match_and_if_modified_since_produce_304() {
    let server = FixtureServer::start("sphinx-example").expect("start server");
    let url = server.url_for("/index.html");

    let first = get(&url);
    let etag = first.header("etag").expect("etag").to_string();
    let last_modified = first
        .header("last-modified")
        .expect("last-modified")
        .to_string();

    let by_etag = request("GET", &url, &[("If-None-Match", &etag)]);
    assert_eq!(by_etag.status, 304);
    assert!(by_etag.body.is_empty(), "304 carries no body");
    assert!(
        by_etag.header("content-length").is_none(),
        "304 must not advertise a length"
    );

    let by_date = request("GET", &url, &[("If-Modified-Since", &last_modified)]);
    assert_eq!(by_date.status, 304);

    // A stale validator still gets the full response.
    let stale = request("GET", &url, &[("If-None-Match", "\"0000000000000000\"")]);
    assert_eq!(stale.status, 200);
    assert!(!stale.body.is_empty());
}

#[test]
fn etags_differ_when_content_differs() {
    let server = FixtureServer::start("sphinx-example").expect("start server");

    let index = get(&server.url_for("/index.html"));
    let reference = get(&server.url_for("/api/reference.html"));

    assert_ne!(
        index.header("etag").unwrap(),
        reference.header("etag").unwrap()
    );
}

// ---------------------------------------------------------------------------
// Scripted misbehaviour
// ---------------------------------------------------------------------------

#[test]
fn a_limited_script_runs_out_and_falls_through_to_the_file() {
    let server = FixtureServer::start("sphinx-example").expect("start server");
    server.script(
        "/index.html",
        Scripted::new(503).header("retry-after", "1").times(2),
    );

    assert_eq!(get(&server.url_for("/index.html")).status, 503);
    assert_eq!(get(&server.url_for("/index.html")).status, 503);

    // Third request: the script is exhausted, so the real fixture is served.
    // This is the shape a retry-with-backoff test needs.
    let recovered = get(&server.url_for("/index.html"));
    assert_eq!(recovered.status, 200);
    assert!(recovered.text().contains("Widget"));
}

#[test]
fn an_unlimited_script_applies_to_every_request() {
    let server = FixtureServer::start("sphinx-example").expect("start server");
    server.script(
        "/robots.txt",
        Scripted::ok("text/plain", "User-agent: *\nDisallow: /\n"),
    );

    for _ in 0..3 {
        assert!(get(&server.url_for("/robots.txt"))
            .text()
            .contains("Disallow: /\n"));
    }

    server.clear_scripts();
    assert!(get(&server.url_for("/robots.txt"))
        .text()
        .contains("Disallow: /private/"));
}

#[test]
fn scripted_redirects_and_delays_work() {
    let server = FixtureServer::start("sphinx-example").expect("start server");
    server.script("/moved.html", Scripted::redirect(302, "/index.html"));
    server.script(
        "/slow.html",
        Scripted::ok("text/html", "<p>slow</p>").delay(Duration::from_millis(120)),
    );

    let redirect = get(&server.url_for("/moved.html"));
    assert_eq!(redirect.status, 302);
    assert_eq!(redirect.header("location").unwrap(), "/index.html");

    let started = std::time::Instant::now();
    assert_eq!(get(&server.url_for("/slow.html")).status, 200);
    assert!(started.elapsed() >= Duration::from_millis(100));
}

#[test]
fn a_truncated_body_looks_like_a_dropped_connection() {
    let server = FixtureServer::start("sphinx-example").expect("start server");
    server.script(
        "/partial.html",
        Scripted::ok("text/html", "0123456789").truncated(4),
    );

    let response = get(&server.url_for("/partial.html"));

    // The fetcher is promised ten bytes and gets four. Treating this as a
    // complete page is the silent failure this fixture exists to catch.
    assert_eq!(response.header("content-length").unwrap(), "10");
    assert_eq!(response.body.len(), 4);
}

// ---------------------------------------------------------------------------
// The request log
// ---------------------------------------------------------------------------

#[test]
fn the_request_log_records_what_the_server_saw() {
    let server = FixtureServer::start("sphinx-example").expect("start server");

    request(
        "GET",
        &server.url_for("/index.html?highlight=widget"),
        &[("User-Agent", "tome/0.0.0 (+https://example.invalid)")],
    );
    get(&server.url_for("/api/reference.html"));

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(server.request_count(), 2);

    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/index.html");
    assert_eq!(requests[0].query.as_deref(), Some("highlight=widget"));
    assert!(requests[0]
        .header("user-agent")
        .unwrap()
        .starts_with("tome/"));
    assert!(
        requests[0].at <= requests[1].at,
        "arrival instants must be ordered"
    );

    assert_eq!(server.requests_for("/api/reference.html").len(), 1);

    server.clear_requests();
    assert_eq!(server.request_count(), 0);
}

// ---------------------------------------------------------------------------
// Shutdown — the offline guarantee
// ---------------------------------------------------------------------------

#[test]
fn shutdown_makes_the_port_refuse_connections() {
    let server = FixtureServer::start("sphinx-example").expect("start server");
    let addr = server.addr();
    assert_eq!(get(&server.url_for("/index.html")).status, 200);

    server.shutdown();

    assert!(!server.is_running());
    let refused = TcpStream::connect_timeout(&addr, Duration::from_millis(500));
    assert!(
        refused.is_err(),
        "after shutdown the port must refuse connections — otherwise the \
         offline assertion in the ingestion tests proves nothing"
    );

    server.shutdown(); // idempotent
}

#[test]
fn servers_get_distinct_ports_so_tests_can_run_in_parallel() {
    let a = FixtureServer::start("sphinx-example").expect("start a");
    let b = FixtureServer::start("sphinx-example").expect("start b");

    assert_ne!(a.addr().port(), b.addr().port());
    assert_eq!(get(&a.url_for("/index.html")).status, 200);
    assert_eq!(get(&b.url_for("/index.html")).status, 200);
}

#[test]
fn serving_an_arbitrary_directory_works() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("page.html"), "<h1>generated</h1>").expect("write");

    let server = FixtureServer::serve(dir.path()).expect("start server");

    assert_eq!(
        get(&server.url_for("/page.html")).text(),
        "<h1>generated</h1>"
    );
}

// ---------------------------------------------------------------------------
// A deliberately unhelpful HTTP client
// ---------------------------------------------------------------------------

struct Response {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Response {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn get(url: &str) -> Response {
    request("GET", url, &[])
}

fn request(method: &str, url: &str, headers: &[(&str, &str)]) -> Response {
    let rest = url.strip_prefix("http://").expect("http:// url");
    let (authority, target) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    raw_request(method, authority, target, headers)
}

/// Send a target verbatim, without the normalisation a real client applies.
/// Traversal tests depend on `..` actually reaching the server.
fn raw_get(base_url: &str, target: &str) -> Response {
    let authority = base_url.strip_prefix("http://").expect("http:// url");
    raw_request("GET", authority, target, &[])
}

fn raw_request(method: &str, authority: &str, target: &str, headers: &[(&str, &str)]) -> Response {
    let mut stream = TcpStream::connect(authority).expect("connect to fixture server");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set timeout");

    let mut request = format!("{method} {target} HTTP/1.1\r\nHost: {authority}\r\n");
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("Connection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).expect("write request");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");

    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response has a header/body boundary");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let body = raw[split + 4..].to_vec();

    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .expect("status line");

    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();

    Response {
        status,
        headers,
        body,
    }
}
