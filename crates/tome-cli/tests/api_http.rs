//! The HTTP API, tested as a process (P4-009..012).
//!
//! Every test speaks raw HTTP/1.1 over a `TcpStream` — no HTTP client
//! dependency, and nothing between the test and the bytes: a spoofed `Host`
//! header or a missing CORS header is exactly what got sent and exactly what
//! came back.
//!
//! The security assertions here are the ticket: no token → 401 on
//! everything but status; rotated-away tokens rejected; `Host` spoof → 403
//! (DNS rebinding); disallowed `Origin` → 403; **no CORS headers by
//! default**; rate limit closes over everything including auth failures.
//!
//! **A real browser confirmed the CORS half**, because S3's verification
//! plan says reading the config and concluding it is fine is how the
//! original plan got this wrong. 2026-07-30, Chromium on a hostile page at
//! `http://127.0.0.1:8999` against `tome serve` on 7431:
//!
//! | Attempt from the hostile origin | Result |
//! |---|---|
//! | `fetch('/api/v1/status')` | `TypeError: Failed to fetch` — unreadable |
//! | `fetch('/api/v1/sources')` **with a valid stolen token** | `TypeError: Failed to fetch` — unreadable |
//! | `fetch('/api/v1/status', {mode:'no-cors'})` | `type=opaque`, `status=0`, empty body — sent, unreadable, **and refused 403 server-side by the Origin guard** |
//!
//! The third row is the one worth keeping: `no-cors` is how a page sends a
//! request it cannot read, and the origin guard means it has no effect
//! either. Re-run this by hand when the middleware order changes.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use tome_testkit::FixtureServer;

fn tome_bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().expect("test binary has a path");
    p.pop();
    p.pop();
    p.push("tome");
    p
}

/// A running `tome serve` on an ephemeral port, killed on drop.
struct Server {
    child: Child,
    addr: String,
}

impl Server {
    fn start(home: &Path, extra_args: &[&str]) -> Self {
        let mut child = Command::new(tome_bin())
            .args(["serve", "--port", "0"])
            .args(extra_args)
            .env("TOME_HOME", home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("tome serve starts");
        // The listening line is printed to stderr once the socket is bound;
        // reading it doubles as the readiness gate.
        let stderr = child.stderr.take().expect("stderr piped");
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        let addr = loop {
            line.clear();
            let n = reader.read_line(&mut line).expect("read serve stderr");
            assert!(n > 0, "serve exited before listening");
            if let Some(rest) = line.trim().strip_prefix("tome serve: listening on http://") {
                break rest.trim_end_matches("/api/v1").to_owned();
            }
        };
        // Keep draining stderr so the server never blocks on a full pipe.
        std::thread::spawn(move || {
            let mut sink = String::new();
            let _ = reader.read_to_string(&mut sink);
        });
        Self { child, addr }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Reply {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

impl Reply {
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("body is not JSON ({e}): {}", self.body))
    }
    /// Every error must wear the one envelope.
    fn assert_error_envelope(&self) -> &Self {
        let json = self.json();
        assert!(
            json["error"]["code"].is_string() && json["error"]["message"].is_string(),
            "uniform error envelope: {}",
            self.body
        );
        self
    }
}

/// One raw HTTP/1.1 request. `headers` REPLACES nothing — `Host` is included
/// only if the caller passes it, which is the point: tests control every byte.
fn raw_request(
    addr: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Reply {
    let mut stream = TcpStream::connect(addr).expect("connect");
    let mut request = format!("{method} {path} HTTP/1.1\r\n");
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(body) = body {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("Connection: close\r\n\r\n");
    if let Some(body) = body {
        request.push_str(body);
    }
    stream.write_all(request.as_bytes()).expect("send");

    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("receive");
    let response = String::from_utf8_lossy(&response).into_owned();
    let (head, body) = response.split_once("\r\n\r\n").expect("header/body split");
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .expect("status line");
    let headers = lines
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.to_owned()))
        .collect();
    Reply {
        status,
        headers,
        body: body.to_owned(),
    }
}

/// The default request: correct Host, optional bearer token.
fn get(server: &Server, path: &str, token: Option<&str>) -> Reply {
    let auth;
    let mut headers = vec![("Host", "127.0.0.1")];
    if let Some(token) = token {
        auth = format!("Bearer {token}");
        headers.push(("Authorization", &auth));
    }
    raw_request(&server.addr, "GET", path, &headers, None)
}

const TOKEN: &str = "test-token-0123456789abcdef0123456789abcdef";

fn home_with_token() -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("tempdir");
    // The file store (TOME_HOME is set), written the same way the token
    // module writes it.
    let state = home.path().join("state");
    std::fs::create_dir_all(&state).expect("state dir");
    std::fs::write(state.join("api-token"), TOKEN).expect("token file");
    home
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[test]
fn status_is_the_only_unauthenticated_route_and_says_almost_nothing() {
    let home = home_with_token();
    let server = Server::start(home.path(), &[]);

    let reply = get(&server, "/api/v1/status", None);
    assert_eq!(reply.status, 200);
    let json = reply.json();
    assert_eq!(json["status"], "ok");
    assert!(json["version"].is_string());
    // Nothing but status and version: no paths, no counts, no names.
    assert_eq!(
        json.as_object().expect("object").len(),
        2,
        "status leaks nothing: {json}"
    );

    // Everything else is 401 without a token — including status/detail.
    for path in [
        "/api/v1/status/detail",
        "/api/v1/search?q=x",
        "/api/v1/sources",
        "/api/v1/sources/x",
        "/api/v1/sources/x/pages",
        "/api/v1/sources/x/pages/index.html",
    ] {
        let reply = get(&server, path, None);
        assert_eq!(reply.status, 401, "{path} unauthenticated");
        reply.assert_error_envelope();
    }
}

#[test]
fn wrong_and_rotated_tokens_are_rejected_and_the_error_does_not_say_why() {
    // Create a token, rotate it away, and start the server on the new one.
    let home = tempfile::tempdir().expect("tempdir");
    let show = |args: &[&str]| -> String {
        let out = Command::new(tome_bin())
            .args(args)
            .env("TOME_HOME", home.path())
            .output()
            .expect("tome runs");
        assert!(out.status.success());
        String::from_utf8(out.stdout)
            .expect("utf8")
            .trim()
            .to_owned()
    };
    let old_token = show(&["status", "--show-token"]);
    show(&["config", "rotate-token"]);
    let new_token = show(&["status", "--show-token"]);
    assert_ne!(old_token, new_token, "rotation replaced the token");

    let server = Server::start(home.path(), &[]);
    assert_eq!(
        get(&server, "/api/v1/sources", Some(&new_token)).status,
        200
    );

    for bad in [Some(old_token.as_str()), Some("wrong"), Some(""), None] {
        let reply = get(&server, "/api/v1/sources", bad);
        assert_eq!(reply.status, 401, "{bad:?} rejected");
        // Non-leaky: missing, malformed, wrong and stale all read identically.
        assert_eq!(
            reply.json()["error"]["message"],
            "Missing or invalid bearer token.",
            "one message for every failure shape"
        );
    }
}

// ---------------------------------------------------------------------------
// Host / Origin / CORS
// ---------------------------------------------------------------------------

#[test]
fn a_spoofed_host_is_rejected_even_with_a_valid_token() {
    let home = home_with_token();
    let server = Server::start(home.path(), &[]);
    let auth = format!("Bearer {TOKEN}");

    for host in ["evil.com", "localhost.evil.com", "127.0.0.1.evil.com"] {
        let reply = raw_request(
            &server.addr,
            "GET",
            "/api/v1/sources",
            &[("Host", host), ("Authorization", &auth)],
            None,
        );
        assert_eq!(reply.status, 403, "Host {host} is DNS rebinding");
        reply.assert_error_envelope();
    }

    // And a missing Host entirely.
    let reply = raw_request(
        &server.addr,
        "GET",
        "/api/v1/sources",
        &[("Authorization", &auth)],
        None,
    );
    assert_eq!(reply.status, 403, "no Host header");

    // The legitimate spellings all pass.
    for host in ["127.0.0.1", "localhost", "[::1]", "localhost:7431"] {
        let reply = raw_request(
            &server.addr,
            "GET",
            "/api/v1/status",
            &[("Host", host)],
            None,
        );
        assert_eq!(reply.status, 200, "Host {host} is this machine");
    }
}

#[test]
fn no_cors_headers_by_default_and_foreign_origins_are_refused() {
    let home = home_with_token();
    let server = Server::start(home.path(), &[]);
    let auth = format!("Bearer {TOKEN}");

    // A same-machine request gets no CORS headers at all.
    let reply = get(&server, "/api/v1/status", None);
    assert!(
        !reply
            .headers
            .keys()
            .any(|k| k.starts_with("access-control-")),
        "no CORS headers by default: {:?}",
        reply.headers
    );

    // A browser-originated cross-origin request is refused outright — the
    // request, not just the response: absence of CORS only hides the body,
    // this also stops side effects.
    let reply = raw_request(
        &server.addr,
        "GET",
        "/api/v1/status",
        &[("Host", "127.0.0.1"), ("Origin", "https://evil.example")],
        None,
    );
    assert_eq!(reply.status, 403);

    // Even with the token: a page that stole the token still cannot use it
    // from a browser context.
    let reply = raw_request(
        &server.addr,
        "GET",
        "/api/v1/sources",
        &[
            ("Host", "127.0.0.1"),
            ("Origin", "https://evil.example"),
            ("Authorization", &auth),
        ],
        None,
    );
    assert_eq!(reply.status, 403);
}

#[test]
fn the_origin_allowlist_echoes_exactly_that_origin() {
    let home = home_with_token();
    let server = Server::start(home.path(), &["--allow-origin", "http://ok.example"]);
    let auth = format!("Bearer {TOKEN}");

    let reply = raw_request(
        &server.addr,
        "GET",
        "/api/v1/sources",
        &[
            ("Host", "127.0.0.1"),
            ("Origin", "http://ok.example"),
            ("Authorization", &auth),
        ],
        None,
    );
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply
            .headers
            .get("access-control-allow-origin")
            .map(|s| s.as_str()),
        Some("http://ok.example"),
        "the origin is echoed, never *"
    );

    // Preflight for the allowlisted origin.
    let reply = raw_request(
        &server.addr,
        "OPTIONS",
        "/api/v1/sources",
        &[("Host", "127.0.0.1"), ("Origin", "http://ok.example")],
        None,
    );
    assert_eq!(reply.status, 204);

    // Any other origin is still refused.
    let reply = raw_request(
        &server.addr,
        "GET",
        "/api/v1/sources",
        &[
            ("Host", "127.0.0.1"),
            ("Origin", "http://other.example"),
            ("Authorization", &auth),
        ],
        None,
    );
    assert_eq!(reply.status, 403);
}

#[test]
fn a_wildcard_origin_refuses_to_start() {
    let home = home_with_token();
    let out = Command::new(tome_bin())
        .args(["serve", "--port", "0", "--allow-origin", "*"])
        .env("TOME_HOME", home.path())
        .output()
        .expect("tome runs");
    assert!(!out.status.success(), "wildcard must not start a server");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--allow-origin"),
        "the error names the flag"
    );
}

// ---------------------------------------------------------------------------
// Rate limiting
// ---------------------------------------------------------------------------

#[test]
fn the_rate_limit_closes_over_auth_failures_too() {
    let home = home_with_token();
    let server = Server::start(home.path(), &[]);

    // Wrong-token requests burn the same budget — brute-force is bounded.
    let mut saw_429 = false;
    for _ in 0..150 {
        let reply = get(&server, "/api/v1/sources", Some("wrong"));
        if reply.status == 429 {
            assert!(
                reply.headers.contains_key("retry-after"),
                "429 carries Retry-After"
            );
            reply.assert_error_envelope();
            saw_429 = true;
            break;
        }
        assert_eq!(reply.status, 401);
    }
    assert!(saw_429, "150 bad requests must trip the limiter");
}

// ---------------------------------------------------------------------------
// The API over a real library
// ---------------------------------------------------------------------------

#[test]
fn a_pulled_library_is_readable_and_mutable_over_http() {
    let fixture = FixtureServer::start("sphinx-example").expect("fixture server");
    let home = home_with_token();
    let url = format!("{}/", fixture.url());

    let add = Command::new(tome_bin())
        .args([
            "add",
            &url,
            "--yes",
            "--insecure",
            "--quiet",
            "--name",
            "widget",
        ])
        .env("TOME_HOME", home.path())
        .stdin(Stdio::null())
        .output()
        .expect("tome add runs");
    assert!(add.status.success());

    let server = Server::start(home.path(), &[]);
    let token = Some(TOKEN);

    // Search: results with snippets and an honest total.
    let reply = get(&server, "/api/v1/search?q=widget", token);
    assert_eq!(reply.status, 200);
    let json = reply.json();
    assert!(json["results"].as_array().is_some_and(|r| !r.is_empty()));
    assert_eq!(json["returned"], json["results"].as_array().unwrap().len());
    assert!(json["total_hits"].as_u64().unwrap() >= 1);
    assert_eq!(json["results"][0]["source_id"], "widget");

    // Sources and detail.
    let json = get(&server, "/api/v1/sources", token).json();
    assert_eq!(json["sources"][0]["id"], "widget");
    let json = get(&server, "/api/v1/sources/widget", token).json();
    assert_eq!(json["pulled"], true);

    // Pages, paginated shape.
    let json = get(&server, "/api/v1/sources/widget/pages", token).json();
    assert!(json["total"].as_u64().unwrap() >= 3);

    // A page: sanitized rendered HTML plus its outline, never raw upstream.
    let reply = get(&server, "/api/v1/sources/widget/pages/index.html", token);
    assert_eq!(reply.status, 200);
    let json = reply.json();
    assert!(json["content"].as_str().unwrap().contains("tome-page"));
    assert!(json["toc"].is_array());

    // Unknowns are 404 with the envelope.
    let reply = get(&server, "/api/v1/sources/nope", token);
    assert_eq!(reply.status, 404);
    reply.assert_error_envelope();
    let reply = get(&server, "/api/v1/sources/widget/pages/nope.html", token);
    assert_eq!(reply.status, 404);

    // DELETE removes it everywhere; a follow-up GET is 404.
    let auth = format!("Bearer {TOKEN}");
    let reply = raw_request(
        &server.addr,
        "DELETE",
        "/api/v1/sources/widget",
        &[("Host", "127.0.0.1"), ("Authorization", &auth)],
        None,
    );
    assert_eq!(reply.status, 200);
    assert_eq!(get(&server, "/api/v1/sources/widget", token).status, 404);
    let json = get(&server, "/api/v1/search?q=widget", token).json();
    assert_eq!(
        json["results"].as_array().unwrap().len(),
        0,
        "no ghost results after DELETE"
    );
}

#[test]
fn post_sources_validates_and_rejects_blocked_addresses() {
    let home = home_with_token();
    let server = Server::start(home.path(), &[]);
    let auth = format!("Bearer {TOKEN}");
    let post = |id: &str, body: &str| {
        raw_request(
            &server.addr,
            "POST",
            &format!("/api/v1/sources?id={id}"),
            &[
                ("Host", "127.0.0.1"),
                ("Authorization", &auth),
                ("Content-Type", "application/yaml"),
            ],
            Some(body),
        )
    };

    // A literal-IP URL into a blocked range: the SSRF primitive P4-010
    // warns about. Rejected before anything is written.
    let metadata = "schema_version: 1\nname: sneaky\nsource:\n  type: generic\n  url: https://169.254.169.254/\n";
    let reply = post("sneaky", metadata);
    assert_eq!(reply.status, 400, "{}", reply.body);
    reply.assert_error_envelope();
    assert_eq!(
        get(&server, "/api/v1/sources", Some(TOKEN)).json()["sources"]
            .as_array()
            .unwrap()
            .len(),
        0,
        "nothing was written"
    );

    // An invalid config is 400 with the parser's message.
    assert_eq!(post("bad", "not: a: config").status, 400);

    // A valid config is 201 and shows up.
    let valid = "schema_version: 1\nname: Example\nsource:\n  type: generic\n  url: https://docs.example.org/\n";
    assert_eq!(post("example", valid).status, 201);
    let json = get(&server, "/api/v1/sources/example", Some(TOKEN)).json();
    assert_eq!(json["pulled"], false);

    // The same id again is a conflict.
    assert_eq!(post("example", valid).status, 409);
}
