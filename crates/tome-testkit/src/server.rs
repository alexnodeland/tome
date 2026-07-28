//! The fixture HTTP server — implementation-plan **S0-6**.
//!
//! Serves a directory of committed documentation fixtures over loopback, with
//! no network access, so that crawler, scraper, and normalization tests run
//! against something realistic and deterministic.
//!
//! ```no_run
//! use tome_testkit::FixtureServer;
//!
//! let server = FixtureServer::start("sphinx-example").expect("start fixture server");
//! let url = server.url_for("/api/reference.html");
//! // ... fetch, crawl, normalize ...
//!
//! // The offline guarantee is an assertion, not a vibe: shut the server down,
//! // then render. Anything still reaching for the network now fails loudly.
//! server.shutdown();
//! assert!(!server.is_running());
//! ```
//!
//! # What it does
//!
//! | | |
//! |---|---|
//! | Static files | From the fixture root, content type by extension |
//! | Directory index | `/dir/` serves `dir/index.html`; `/dir` redirects to `/dir/` |
//! | Conditional GET | `ETag` + `Last-Modified` out, `If-None-Match` / `If-Modified-Since` in, `304` when they match |
//! | `HEAD` | Headers and `Content-Length`, no body |
//! | Scripted responses | Per-path overrides: any status, headers, delay, truncated body |
//! | Request log | Method, path, headers, arrival instant — assert rate limiting and conditional GET from the server side |
//! | Shutdown | Stops accepting and closes the port, so later connections are *refused* rather than hanging |
//!
//! # What it deliberately does not do
//!
//! No keep-alive: every response carries `Connection: close`. No chunked
//! transfer encoding, no compression, no HTTP/2, no TLS. Requests bodies are
//! read and discarded; anything other than `GET` or `HEAD` gets a `405`.
//!
//! # Path traversal
//!
//! A fixture server that can be talked into serving `/etc/passwd` is a bad
//! joke in a project whose threat model includes hostile documentation sites.
//! Request targets are percent-decoded **first**, then rejected if any
//! component is `..` (so `%2e%2e%2f` is caught), then resolved and checked
//! against the canonicalized root — which also catches a symlink pointing out
//! of the fixture tree.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a handler waits for a client to finish sending its request before
/// giving up. Only reached by a client that opens a connection and stalls.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Accept-loop poll interval. The listener is non-blocking so that
/// [`FixtureServer::shutdown`] is prompt and does not depend on a request
/// arriving to unblock `accept`.
const ACCEPT_POLL: Duration = Duration::from_millis(1);

// ---------------------------------------------------------------------------
// Requests, as recorded
// ---------------------------------------------------------------------------

/// One request as the server saw it.
#[derive(Debug, Clone)]
pub struct Request {
    /// `GET`, `HEAD`, or whatever the client actually sent.
    pub method: String,
    /// The raw request target, query string included.
    pub target: String,
    /// The decoded path, query string removed. This is what scripts match on.
    pub path: String,
    /// Query string, if any, undecoded.
    pub query: Option<String>,
    /// Header names lowercased; values as sent.
    pub headers: HashMap<String, String>,
    /// When the request arrived. Rate-limit tests assert on the gaps between
    /// these rather than on wall-clock timing inside the client.
    pub at: Instant,
}

impl Request {
    /// Case-insensitive header lookup.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Scripted responses
// ---------------------------------------------------------------------------

/// A response the server returns instead of reading a file.
///
/// Registered per path with [`FixtureServer::script`]. Applies indefinitely
/// unless limited with [`Scripted::times`] — a limited script that runs out is
/// removed, and the request falls through to the next script for that path, or
/// to the file on disk.
#[derive(Debug, Clone)]
pub struct Scripted {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    delay: Option<Duration>,
    truncate_to: Option<usize>,
    remaining: Option<u32>,
}

impl Scripted {
    /// A response with this status, no body.
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            delay: None,
            truncate_to: None,
            remaining: None,
        }
    }

    /// `301`/`302`/`307`/`308` to `location`.
    pub fn redirect(status: u16, location: &str) -> Self {
        Self::new(status).header("location", location)
    }

    /// `200` with a body and an explicit content type.
    pub fn ok(content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self::new(200)
            .header("content-type", content_type)
            .body(body)
    }

    /// Add a response header. Repeatable.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Set the response body.
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// Sleep before responding. For timeout and rate-limit tests.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Advertise the full `Content-Length` but send only `bytes` of the body,
    /// then close the connection. This is what a dropped connection looks like
    /// to a client, and it is the case a naive fetcher silently treats as a
    /// complete page.
    pub fn truncated(mut self, bytes: usize) -> Self {
        self.truncate_to = Some(bytes);
        self
    }

    /// Apply at most `n` times, then fall through. `Scripted::new(503).times(2)`
    /// is the shape a retry test wants: fail twice, then serve the real file.
    pub fn times(mut self, n: u32) -> Self {
        self.remaining = Some(n);
        self
    }
}

// ---------------------------------------------------------------------------
// The server
// ---------------------------------------------------------------------------

struct Shared {
    root: PathBuf,
    running: AtomicBool,
    log: Mutex<Vec<Request>>,
    scripts: Mutex<HashMap<String, Vec<Scripted>>>,
}

/// An HTTP server serving fixtures from a directory. See the [module
/// docs](self) for the supported surface and its deliberate omissions.
///
/// Bound to `127.0.0.1` on an ephemeral port, so any number of these can run
/// concurrently — including in parallel `cargo test` threads.
///
/// Dropping the server shuts it down.
pub struct FixtureServer {
    addr: SocketAddr,
    shared: Arc<Shared>,
    accept: Option<JoinHandle<()>>,
}

impl FixtureServer {
    /// Serve one of this crate's committed fixture sites by directory name,
    /// e.g. `"sphinx-example"`. See `crates/tome-testkit/fixtures/README.md`.
    pub fn start(site: &str) -> std::io::Result<Self> {
        Self::serve(fixtures_dir().join(site))
    }

    /// Serve an arbitrary directory — a temporary one built by the test, or a
    /// corpus checked out elsewhere.
    pub fn serve(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        let root = root.canonicalize().map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("fixture root {} is not readable: {e}", root.display()),
            )
        })?;

        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;

        let shared = Arc::new(Shared {
            root,
            running: AtomicBool::new(true),
            log: Mutex::new(Vec::new()),
            scripts: Mutex::new(HashMap::new()),
        });

        let accept = std::thread::spawn({
            let shared = Arc::clone(&shared);
            move || accept_loop(&listener, &shared)
        });

        Ok(Self {
            addr,
            shared,
            accept: Some(accept),
        })
    }

    /// Base URL, no trailing slash: `http://127.0.0.1:52001`.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Absolute URL for a path: `server.url_for("/api/reference.html")`.
    pub fn url_for(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{path}", self.url())
        } else {
            format!("{}/{path}", self.url())
        }
    }

    /// The bound address, for tests that assert the port is refused after
    /// [`shutdown`](Self::shutdown).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Register a scripted response for an exact decoded path. Query strings
    /// are ignored when matching. Repeated calls queue up.
    pub fn script(&self, path: &str, response: Scripted) {
        lock(&self.shared.scripts)
            .entry(path.to_string())
            .or_default()
            .push(response);
    }

    /// Drop every scripted response, restoring plain file serving.
    pub fn clear_scripts(&self) {
        lock(&self.shared.scripts).clear();
    }

    /// Every request received so far, in arrival order.
    pub fn requests(&self) -> Vec<Request> {
        lock(&self.shared.log).clone()
    }

    /// Requests for one decoded path. The usual use is asserting that a
    /// conditional GET happened, or that a crawler did not fetch a page twice.
    pub fn requests_for(&self, path: &str) -> Vec<Request> {
        lock(&self.shared.log)
            .iter()
            .filter(|r| r.path == path)
            .cloned()
            .collect()
    }

    /// Total requests received.
    pub fn request_count(&self) -> usize {
        lock(&self.shared.log).len()
    }

    /// Forget the request log. Useful between phases of a longer test.
    pub fn clear_requests(&self) {
        lock(&self.shared.log).clear();
    }

    /// Whether the server is still accepting.
    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::SeqCst)
    }

    /// Stop accepting and close the port.
    ///
    /// This is the operation the offline assertion is built on: after it
    /// returns, connections to [`addr`](Self::addr) are **refused**, so a page
    /// that still references a remote asset fails immediately and visibly
    /// rather than quietly succeeding because the fixture was still up.
    ///
    /// In-flight handlers finish on their own; no new connection is accepted.
    /// Idempotent.
    pub fn shutdown(&self) {
        self.shared.running.store(false, Ordering::SeqCst);
        // The accept thread owns the listener and drops it on exit, which is
        // what closes the port. Joining is `shutdown_and_join`'s business
        // (Drop) — here we only need the flag, and the loop notices within
        // ACCEPT_POLL.
        while self.accept.as_ref().is_some_and(|h| !h.is_finished()) {
            std::thread::sleep(ACCEPT_POLL);
        }
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.accept.take() {
            let _ = handle.join();
        }
    }
}

fn accept_loop(listener: &TcpListener, shared: &Arc<Shared>) {
    while shared.running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                // macOS inherits O_NONBLOCK from the listener onto accepted
                // sockets (BSD behaviour; Linux does not). Without this, every
                // read below returns WouldBlock and no request is ever served.
                if stream.set_nonblocking(false).is_err() {
                    continue;
                }
                let shared = Arc::clone(shared);
                std::thread::spawn(move || {
                    let _ = handle_connection(stream, &shared);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_POLL);
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(mut stream: TcpStream, shared: &Shared) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;

    let request = match read_request(&stream)? {
        Some(request) => request,
        None => return Ok(()),
    };
    lock(&shared.log).push(request.clone());

    let response = match take_script(shared, &request.path) {
        Some(script) => {
            if let Some(delay) = script.delay {
                std::thread::sleep(delay);
            }
            Response {
                status: script.status,
                headers: script.headers.clone(),
                body: script.body.clone(),
                truncate_to: script.truncate_to,
            }
        }
        None => serve_file(shared, &request),
    };

    write_response(&mut stream, &request, &response)
}

/// Pop the next applicable script for `path`, if any.
fn take_script(shared: &Shared, path: &str) -> Option<Scripted> {
    let mut scripts = lock(&shared.scripts);
    let queue = scripts.get_mut(path)?;

    let taken = loop {
        let script = queue.first_mut()?;
        match script.remaining {
            // Unlimited: applies to every request for this path.
            None => break script.clone(),
            // `.times(0)` is a caller mistake; drop it and try the next script
            // rather than letting it wedge the queue.
            Some(0) => {
                queue.remove(0);
            }
            Some(n) => {
                script.remaining = Some(n - 1);
                let taken = script.clone();
                if n == 1 {
                    queue.remove(0);
                }
                break taken;
            }
        }
    };

    if queue.is_empty() {
        scripts.remove(path);
    }
    Some(taken)
}

// ---------------------------------------------------------------------------
// Request parsing
// ---------------------------------------------------------------------------

fn read_request(stream: &TcpStream) -> std::io::Result<Option<Request>> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    if reader.read_line(&mut line)? == 0 {
        return Ok(None); // Client connected and closed without sending.
    }

    let mut parts = line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return Ok(None);
    };

    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            break;
        }
        let header = header.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let (raw_path, query) = match target.split_once('?') {
        Some((p, q)) => (p, Some(q.to_string())),
        None => (target, None),
    };
    // Fragments never reach a server, but a hand-written test client might send
    // one; strip it rather than 404 on a path that ends in `#anchor`.
    let raw_path = raw_path.split('#').next().unwrap_or(raw_path);

    Ok(Some(Request {
        method: method.to_string(),
        target: target.to_string(),
        path: percent_decode(raw_path).unwrap_or_else(|| raw_path.to_string()),
        query,
        headers,
        at: Instant::now(),
    }))
}

/// Decode `%XX` escapes. Returns `None` if the result is not valid UTF-8 —
/// which is a 400, not something to paper over with lossy decoding.
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        // A stray or malformed `%` is a literal `%` in practice, not an error.
        let escape = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[i + 1..i + 3]).ok())
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());

        match escape {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }

    String::from_utf8(out).ok()
}

// ---------------------------------------------------------------------------
// File serving
// ---------------------------------------------------------------------------

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    truncate_to: Option<usize>,
}

impl Response {
    fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            truncate_to: None,
        }
    }

    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    fn body_of(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }
}

fn serve_file(shared: &Shared, request: &Request) -> Response {
    if !matches!(request.method.as_str(), "GET" | "HEAD") {
        return Response::status(405).header("allow", "GET, HEAD");
    }

    let Some(target) = safe_join(&shared.root, &request.path) else {
        return Response::status(403);
    };

    if target.is_dir() {
        // Real servers redirect `/dir` to `/dir/` so that relative links inside
        // the page resolve correctly. Crawler tests depend on that behaviour.
        if !request.path.ends_with('/') {
            // Redirect to the *undecoded* path: `Location: /a b/` is malformed,
            // and re-encoding a decoded path is a second chance to get the
            // encoding wrong. The query string is dropped, which real servers
            // preserve — no test needs it, and this is the honest simplification.
            let raw_path = request
                .target
                .split(['?', '#'])
                .next()
                .unwrap_or(&request.target);
            return Response::status(301).header("location", &format!("{raw_path}/"));
        }
        return serve_path(&target.join("index.html"), request);
    }

    serve_path(&target, request)
}

fn serve_path(path: &Path, request: &Request) -> Response {
    let Ok(bytes) = std::fs::read(path) else {
        return Response::status(404)
            .header("content-type", "text/html; charset=utf-8")
            .body_of(b"<!doctype html><title>404</title><h1>Not Found</h1>".to_vec());
    };

    let etag = etag(&bytes);
    let last_modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(http_date)
        .unwrap_or_else(|_| http_date(UNIX_EPOCH));

    let not_modified = request.header("if-none-match").is_some_and(|v| {
        // A client may send a list, or `W/"…"`. Substring containment is the
        // right check for a fixture server and avoids a header parser.
        v.contains(etag.trim_matches('"'))
    }) || request
        .header("if-modified-since")
        .is_some_and(|v| v == last_modified);

    let base = Response::status(if not_modified { 304 } else { 200 })
        .header("content-type", content_type(path))
        .header("etag", &etag)
        .header("last-modified", &last_modified);

    if not_modified {
        // 304 carries no body, and its Content-Length must be absent, not 0 —
        // `write_response` handles that by status.
        return base;
    }

    base.body_of(bytes)
}

/// Resolve a URL path against the fixture root, refusing anything that escapes.
///
/// Returns `None` for traversal attempts, absolute paths, and symlinks pointing
/// outside the root. The decode happens before this is called, so `%2e%2e%2f`
/// is already `../` by the time the components are inspected.
fn safe_join(root: &Path, url_path: &str) -> Option<PathBuf> {
    let relative = url_path.trim_start_matches('/');
    let mut resolved = root.to_path_buf();

    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            // `.` is harmless but `..`, root, and prefixes are not; refuse the
            // whole request rather than silently normalising it away.
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    // Canonicalize when the target exists, so a symlink out of the fixture tree
    // is caught too. A missing path cannot be canonicalized; the component
    // check above already guarantees it is under the root, and it will 404.
    match resolved.canonicalize() {
        Ok(canonical) => canonical.starts_with(root).then_some(canonical),
        Err(_) => Some(resolved),
    }
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "xml" => "application/xml",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

/// FNV-1a over the bytes.
///
/// **Not a security control** — an ETag only has to change when the content
/// changes, and a fixture server has no adversary. Using a real digest here
/// would mean a hashing dependency in every crate's test build.
fn etag(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("\"{hash:016x}\"")
}

// ---------------------------------------------------------------------------
// Response writing
// ---------------------------------------------------------------------------

fn write_response(
    stream: &mut TcpStream,
    request: &Request,
    response: &Response,
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason(response.status)
    );

    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }

    // 304 must not advertise a length; every other status must, including for
    // HEAD, where the length describes the body the client would have got.
    if response.status != 304 {
        head.push_str(&format!("content-length: {}\r\n", response.body.len()));
    }
    head.push_str("connection: close\r\n\r\n");

    stream.write_all(head.as_bytes())?;

    let send_body = request.method != "HEAD" && response.status != 304;
    if send_body {
        let body = match response.truncate_to {
            Some(n) => &response.body[..n.min(response.body.len())],
            None => &response.body[..],
        };
        stream.write_all(body)?;
    }

    stream.flush()?;
    // Half-close so the client sees EOF immediately. A truncated body relies on
    // this: the client has been promised more bytes than it will ever get.
    let _ = stream.shutdown(Shutdown::Write);
    Ok(())
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

// ---------------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------------

/// Format a `SystemTime` as an RFC 9110 IMF-fixdate: `Sun, 06 Nov 1994 08:49:37 GMT`.
///
/// Hand-rolled to avoid a date dependency in the test build. Note the matching
/// simplification in `serve_path`: `If-Modified-Since` is compared as a string
/// against the `Last-Modified` we emitted, rather than parsed. That is what a
/// well-behaved client sends back, and it keeps a date *parser* out of here.
fn http_date(time: SystemTime) -> String {
    const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let days = seconds.div_euclid(86_400);
    let time_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);

    // 1970-01-01 was a Thursday, hence the rotated weekday table.
    let weekday = WEEKDAYS[days.rem_euclid(7) as usize];
    let month_name = MONTHS[(month - 1) as usize];

    format!(
        "{weekday}, {day:02} {month_name} {year} {:02}:{:02}:{:02} GMT",
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60,
    )
}

/// Days since the Unix epoch to `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days`, which is exact for the whole range we
/// could ever see and has no leap-year special cases to get wrong.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;

    (if month <= 2 { year + 1 } else { year }, month, day)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Lock through poisoning.
///
/// A panicking test thread must not turn every later `lock()` into a panic of
/// its own — that replaces one clear failure with a cascade of confusing ones.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Directory holding this crate's committed fixture sites.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_dates_match_the_imf_fixdate_format() {
        assert_eq!(
            http_date(UNIX_EPOCH),
            "Thu, 01 Jan 1970 00:00:00 GMT",
            "the epoch itself is the easiest case to get wrong"
        );
        assert_eq!(
            http_date(UNIX_EPOCH + Duration::from_secs(1_700_000_000)),
            "Tue, 14 Nov 2023 22:13:20 GMT"
        );
        // 29 February — the case a hand-rolled calendar exists to get right.
        assert_eq!(
            http_date(UNIX_EPOCH + Duration::from_secs(1_709_208_000)),
            "Thu, 29 Feb 2024 12:00:00 GMT"
        );
    }

    #[test]
    fn percent_decoding_handles_escapes_and_strays() {
        assert_eq!(percent_decode("/a%20b.html").as_deref(), Some("/a b.html"));
        assert_eq!(percent_decode("/%2e%2e/x").as_deref(), Some("/../x"));
        assert_eq!(percent_decode("/100%").as_deref(), Some("/100%"));
        assert_eq!(percent_decode("/%zz").as_deref(), Some("/%zz"));
        // Invalid UTF-8 is a bad request, not something to decode lossily.
        assert_eq!(percent_decode("/%ff%fe"), None);
    }

    #[test]
    fn safe_join_refuses_anything_leaving_the_root() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));

        assert!(safe_join(root, "/Cargo.toml").is_some());
        assert!(safe_join(root, "/nested/../Cargo.toml").is_none());
        assert!(safe_join(root, "/../../etc/passwd").is_none());

        // A doubled leading slash is not an absolute filesystem path: it stays
        // under the root and 404s there, which is what a real server does too.
        let doubled = safe_join(root, "//etc/passwd");
        assert_eq!(doubled.as_deref(), Some(root.join("etc/passwd").as_path()));
    }
}
