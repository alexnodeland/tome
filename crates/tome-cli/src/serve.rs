//! The local HTTP API (P4-008..012): `tome serve`.
//!
//! **Off by default; loopback is not a trust boundary.** Every process on
//! the machine and every page in the user's browser can originate a loopback
//! request, so the design assumes a hostile localhost:
//!
//! * **Bearer token on every request** except `GET /api/v1/status`, loopback
//!   included, compared in constant time ([`crate::token`]). No bypass, no
//!   opt-out.
//! * **No CORS headers by default** — a browser can send the request but
//!   cannot read the response. The allowlist is opt-in per origin; `*` is
//!   rejected at flag parse, before the server starts.
//! * **`Host` and `Origin` are validated** (DNS-rebinding defence): binding
//!   127.0.0.1 does not stop a hostile page when an attacker-controlled name
//!   resolves to 127.0.0.1 — rejecting unexpected `Host` values is what
//!   closes that door.
//! * **Request logs never record query strings or page paths** — they are
//!   user content. Method, route result and status only.
//!
//! Middleware runs outside-in: **host/origin guard → rate limit → CORS →
//! auth**, and each position is load-bearing:
//!
//! * The guard is first, so a rebound or foreign-origin request is rejected
//!   before it can touch the limiter's counters or any handler.
//! * The rate limit precedes auth, so token brute-forcing burns the same
//!   budget as everything else. (429 for everyone is acceptable on a
//!   single-user localhost service; auth-first would let an attacker spend
//!   the whole window learning which tokens are wrong.)
//! * CORS precedes auth because a browser **preflight carries no
//!   `Authorization` header** — it must be answered before the token check,
//!   and it returns no data. The origin guard has already refused every
//!   origin that is not on the allowlist by the time this runs.
//!
//! Axum applies layers innermost-first, so the `.layer()` calls appear in the
//! reverse of that order.

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tome_core::Paths;

use crate::token::TokenValidator;

/// Requests allowed per token per window. Generous for a human-driven local
/// tool, tight enough to bound token brute-force and runaway scripts: at 100
/// per 10 s, a 256-bit token does not fall to online guessing in the lifetime
/// of the machine.
const RATE_LIMIT: u32 = 100;
const RATE_WINDOW_SECS: u64 = 10;

/// `limit=1000000` is a DoS; the search fetch is bounded regardless of what
/// the client asks for. `total_hits` is therefore also capped here, and the
/// response says so via `total_capped`.
const MAX_LIMIT: usize = 100;
const COUNT_CEILING: usize = 1000;

pub(crate) struct ServeOptions {
    pub port: u16,
    pub bind: IpAddr,
    pub allowed_origins: Vec<String>,
}

struct AppState {
    paths: Paths,
    validator: TokenValidator,
    allowed_origins: Vec<String>,
    engine: Mutex<Option<Arc<tome_core::search::SearchEngine>>>,
    rate_window_start: AtomicU64,
    rate_count: AtomicU32,
    started: Instant,
    /// Source ids with a sync in flight, so a second `POST …/sync` gets 409
    /// instead of a second crawl of the same site.
    syncing: Mutex<std::collections::BTreeSet<String>>,
}

impl AppState {
    /// The search engine, opened on first use and shared. Read-only open:
    /// the server must not hold the index's write lock — the app and the CLI
    /// may be indexing concurrently.
    fn engine(&self) -> Result<Arc<tome_core::search::SearchEngine>, ApiError> {
        let mut slot = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(engine) = slot.as_ref() {
            return Ok(Arc::clone(engine));
        }
        if !self.paths.index_dir().exists() {
            return Err(ApiError::index_unavailable());
        }
        let engine = tome_core::search::SearchEngine::open(&self.paths)
            .map_err(|_| ApiError::index_unavailable())?;
        let engine = Arc::new(engine);
        *slot = Some(Arc::clone(&engine));
        Ok(engine)
    }
}

/// The uniform error envelope (PRD Appendix B): one shape for every failure,
/// so clients handle errors generically. Messages never carry the token or
/// echo request content beyond identifiers the client itself sent.
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    retry_after: Option<u64>,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }
    fn unauthorized() -> Self {
        // Deliberately non-specific: missing, malformed and wrong tokens all
        // read the same, so the response teaches an attacker nothing.
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Missing or invalid bearer token.",
        )
    }
    fn forbidden(message: &'static str) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }
    fn not_found(what: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("No such {what}."),
        )
    }
    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }
    fn index_unavailable() -> Self {
        let mut e = Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "index_unavailable",
            "The search index is not available. Pull a source first.",
        );
        e.retry_after = Some(5);
        e
    }
    fn internal() -> Self {
        // Internal details go to the server log, never into the response.
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Internal error.",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": { "code": self.code, "message": self.message, "details": null }
        });
        let mut response = (self.status, Json(body)).into_response();
        if let Some(secs) = self.retry_after {
            if let Ok(value) = HeaderValue::from_str(&secs.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }
        response
    }
}

/// Run the server until ctrl-c. Blocks the calling (sync) thread.
pub(crate) fn run(paths: &Paths, options: ServeOptions) -> Result<()> {
    // The token must exist before the listener does: a server that comes up
    // even briefly without an expected token would accept anything.
    let api_token = crate::token::load_or_create(paths)?;
    let validator = TokenValidator::new(&api_token);
    drop(api_token); // the server holds only the hash from here on

    if !options.bind.is_loopback() {
        tracing::warn!(
            "binding {} exposes the API beyond this machine; every request still requires \
             the bearer token, but the token is now all that stands between the network \
             and your library",
            options.bind
        );
    }

    let state = Arc::new(AppState {
        paths: paths.clone(),
        validator,
        allowed_origins: options.allowed_origins,
        engine: Mutex::new(None),
        rate_window_start: AtomicU64::new(0),
        rate_count: AtomicU32::new(0),
        started: Instant::now(),
        syncing: Mutex::new(std::collections::BTreeSet::new()),
    });

    let app = Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/status/detail", get(status_detail))
        .route("/api/v1/search", get(search))
        .route("/api/v1/sources", get(list_sources).post(add_source))
        .route(
            "/api/v1/sources/{id}",
            get(get_source).delete(delete_source),
        )
        .route("/api/v1/sources/{id}/pages", get(list_pages))
        .route("/api/v1/sources/{id}/pages/{*path}", get(get_page))
        .route("/api/v1/sources/{id}/sync", post(sync_source))
        // Outside-in: guard → rate limit → CORS → auth. CORS sits outside
        // auth because a browser preflight carries no Authorization header —
        // it must be answered (for an allowlisted origin; the guard already
        // rejected the rest) before the token check, and a preflight returns
        // no data. Axum applies layers innermost-first, so the ADD order is
        // the reverse of the RUN order.
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            require_bearer_token,
        ))
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            cors_headers,
        ))
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            rate_limit,
        ))
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            guard_host_and_origin,
        ))
        .with_state(Arc::clone(&state));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the async runtime")?;
    runtime.block_on(async {
        let addr = SocketAddr::from((options.bind, options.port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("could not bind {addr} — is another server running?"))?;
        let bound = listener.local_addr()?;
        // stderr, structured and greppable: tests and scripts parse this
        // line to learn an ephemeral port (`--port 0`).
        eprintln!("tome serve: listening on http://{bound}/api/v1");
        tracing::info!("serving on {bound}");
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
                tracing::info!("shutting down");
            })
            .await
            .context("server error")
    })
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/// DNS-rebinding defence. `Host` must name this machine; an `Origin`, when
/// present, must be on the allowlist. Runs before everything else so a
/// rebound request never reaches a handler, the limiter, or auth.
async fn guard_host_and_origin(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(strip_port);
    match host {
        Some("localhost" | "127.0.0.1" | "[::1]") => {}
        // No Host at all is an HTTP/1.0-style client; nothing legitimate
        // speaks that to this API.
        _ => return Err(ApiError::forbidden("unexpected Host header")),
    }

    if let Some(origin) = request.headers().get(header::ORIGIN) {
        let allowed = origin
            .to_str()
            .is_ok_and(|o| state.allowed_origins.iter().any(|a| a == o));
        if !allowed {
            // Browsers attach Origin to cross-origin and non-GET requests.
            // Without an allowlist entry the request is refused outright —
            // CORS absence alone only hides the *response*; this also stops
            // the request from having effects.
            return Err(ApiError::forbidden("origin not allowed"));
        }
    }
    Ok(next.run(request).await)
}

/// One fixed window for the one token. Before auth, so brute-force is
/// bounded by the same ceiling as everything else.
async fn rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let now = state.started.elapsed().as_secs() / RATE_WINDOW_SECS;
    let window = state.rate_window_start.load(Ordering::Relaxed);
    if now != window {
        // A stale window resets the count. The race between two threads both
        // resetting is benign: the worst case is a slightly generous window.
        state.rate_window_start.store(now, Ordering::Relaxed);
        state.rate_count.store(0, Ordering::Relaxed);
    }
    if state.rate_count.fetch_add(1, Ordering::Relaxed) >= RATE_LIMIT {
        let mut e = ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many requests.",
        );
        e.retry_after = Some(RATE_WINDOW_SECS);
        return Err(e);
    }
    Ok(next.run(request).await)
}

/// The token check. `GET /api/v1/status` is the single unauthenticated
/// route, matched exactly — everything else needs the bearer token,
/// loopback included (the module docs say why there is no loopback bypass).
async fn require_bearer_token(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if request.method() == Method::GET && request.uri().path() == "/api/v1/status" {
        return Ok(next.run(request).await);
    }
    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    match presented {
        Some(token) if state.validator.validate(token) => Ok(next.run(request).await),
        _ => Err(ApiError::unauthorized()),
    }
}

/// CORS response headers — emitted only for an allowlisted Origin, echoing
/// that origin (never `*`). With no allowlist this layer adds nothing, and
/// preflights die in the router. Runs innermost so the headers land on both
/// success and error responses.
async fn cors_headers(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|o| o.to_str().ok())
        .map(str::to_owned)
        .filter(|o| state.allowed_origins.iter().any(|a| a == o));

    // Preflight for an allowlisted origin: answer here; the router has no
    // OPTIONS routes. (The origin guard already rejected non-allowlisted
    // origins before this layer.)
    if request.method() == Method::OPTIONS {
        if let Some(origin) = origin {
            return cors_ok(&origin);
        }
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let mut response = next.run(request).await;
    if let Some(origin) = origin {
        apply_cors(&mut response, &origin);
    }
    response
}

fn cors_ok(origin: &str) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    apply_cors(&mut response, origin);
    response
}

fn apply_cors(response: &mut Response, origin: &str) {
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(origin) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
    }
    headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, DELETE"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("authorization, content-type"),
    );
}

/// `Host` minus any port, IPv6-aware: `[::1]:7431` → `[::1]`.
fn strip_port(host: &str) -> &str {
    if let Some(bracketed) = host.strip_prefix('[') {
        if let Some(end) = bracketed.find(']') {
            return &host[..end + 2];
        }
    }
    host.split(':').next().unwrap_or(host)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn status() -> Json<Value> {
    // Unauthenticated, so it says nothing but "a Tome is here": no paths, no
    // stats, no source names.
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn status_detail(State(state): State<Arc<AppState>>) -> Result<Json<Value>, ApiError> {
    let indexed = state
        .engine()
        .ok()
        .and_then(|engine| engine.len().ok())
        .map(|n| json!(n))
        .unwrap_or(Value::Null);
    let sources = crate::source_configs(&state.paths)
        .map_err(|_| ApiError::internal())?
        .len();
    Ok(Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "sources": sources,
        "indexed_pages": indexed,
        "uptime_secs": state.started.elapsed().as_secs(),
    })))
}

#[derive(serde::Deserialize)]
struct SearchParams {
    q: String,
    scope: Option<String>,
    limit: Option<usize>,
}

async fn search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Value>, ApiError> {
    let limit = params.limit.unwrap_or(10).clamp(1, MAX_LIMIT);
    let started = Instant::now();
    let engine = state.engine()?;

    // Fetch up to the ceiling so `total_hits` means "matching documents",
    // not "page size" — the number the original design got wrong. The
    // ceiling keeps `limit=1000000` from being a DoS; `total_capped` says
    // when the count saturated.
    let all_hits: Vec<_> = engine
        .search(&params.q, COUNT_CEILING)
        .map_err(|_| ApiError::internal())?
        .into_iter()
        .filter(|hit| {
            params
                .scope
                .as_deref()
                .is_none_or(|s| hit.source.as_str() == s)
        })
        .collect();
    let total = all_hits.len();

    let db = tome_core::db::Database::open(&state.paths).ok();
    let results: Vec<Value> = all_hits
        .iter()
        .take(limit)
        .map(|hit| {
            let source_name = db
                .as_ref()
                .and_then(|db| db.get_source(&hit.source).ok().flatten())
                .map(|s| s.name);
            json!({
                "source_id": hit.source.as_str(),
                "source_name": source_name,
                "page_path": hit.path,
                "title": hit.title,
                "snippet": snippet_for(&state, hit),
                "score": hit.score,
                "symbol_kind": hit.symbol_kind.map(|k| k.as_str()),
            })
        })
        .collect();

    Ok(Json(json!({
        "results": results,
        "returned": results.len(),
        "total_hits": total,
        "total_capped": total >= COUNT_CEILING,
        "query_time_ms": started.elapsed().as_millis() as u64,
    })))
}

/// A plain-text snippet for one hit, from the stored page — the index stores
/// no bodies (that is what keeps it at SPIKE-003's size), so snippets read
/// the store, which holds structured nodes and can respect block boundaries.
fn snippet_for(state: &AppState, hit: &tome_core::search::Hit) -> Value {
    let Ok(path) = tome_core::model::PagePath::new(hit.path.clone()) else {
        return Value::Null;
    };
    let store = tome_core::store::PageStore::new(&state.paths, &hit.source);
    let Ok(Some(page)) = store.read(&path) else {
        return Value::Null;
    };
    let terms = state
        .engine()
        .ok()
        .and_then(|engine| engine.highlight_terms(&hit.title).ok())
        .unwrap_or_default();
    let spans = tome_core::search::snippet::snippet(&page.body, &terms, 240);
    let text: String = spans.into_iter().map(|s| s.text).collect();
    if text.is_empty() {
        Value::Null
    } else {
        Value::String(text)
    }
}

async fn list_sources(State(state): State<Arc<AppState>>) -> Result<Json<Value>, ApiError> {
    let configs = crate::source_configs(&state.paths).map_err(|_| ApiError::internal())?;
    let db = state
        .paths
        .database_file()
        .exists()
        .then(|| tome_core::db::Database::open(&state.paths))
        .transpose()
        .map_err(|_| ApiError::internal())?;
    let pulled = match &db {
        Some(db) => db.list_sources().map_err(|_| ApiError::internal())?,
        None => Vec::new(),
    };
    let sources: Vec<Value> = configs
        .iter()
        .map(|(id, _)| {
            let row = pulled.iter().find(|s| s.id == *id);
            json!({
                "id": id.as_str(),
                "name": row.map(|s| s.name.clone()),
                "category": row.map(|s| s.category.clone()),
                "pages": row
                    .and_then(|s| db.as_ref().and_then(|db| db.page_count(&s.id).ok()))
                    .unwrap_or(0),
                "pulled": row.is_some(),
                "last_synced": row
                    .and_then(|s| s.last_synced.as_ref())
                    .map(|t| t.to_rfc3339()),
            })
        })
        .collect();
    Ok(Json(json!({ "sources": sources })))
}

async fn get_source(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let id = parse_source_id(&id)?;
    let configs = crate::source_configs(&state.paths).map_err(|_| ApiError::internal())?;
    if !configs.iter().any(|(existing, _)| *existing == id) {
        return Err(ApiError::not_found("source"));
    }
    let db = tome_core::db::Database::open(&state.paths).ok();
    let row = db.as_ref().and_then(|db| db.get_source(&id).ok().flatten());
    let pages = db.as_ref().and_then(|db| db.page_count(&id).ok());
    Ok(Json(json!({
        "id": id.as_str(),
        "name": row.as_ref().map(|s| s.name.clone()),
        "category": row.as_ref().map(|s| s.category.clone()),
        "pages": pages.unwrap_or(0),
        "pulled": row.is_some(),
        "last_synced": row
            .as_ref()
            .and_then(|s| s.last_synced.as_ref())
            .map(|t| t.to_rfc3339()),
    })))
}

#[derive(serde::Deserialize)]
struct AddSourceParams {
    /// The source id — the config file's stem, because the file name IS the
    /// identity everywhere else in Tome.
    id: String,
}

/// `POST /api/v1/sources?id=<id>`, body = the YAML config (JSON accepted for
/// free: YAML is a superset, and one parser means no content sniffing).
async fn add_source(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AddSourceParams>,
    body: String,
) -> Result<Response, ApiError> {
    const MAX_CONFIG_BYTES: usize = 64 * 1024;
    if body.len() > MAX_CONFIG_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "Config exceeds 64 KiB.",
        ));
    }
    let id = parse_source_id(&params.id)?;
    let config_file = state.paths.source_config_file(&id);
    if config_file.exists() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "source_exists",
            "A source with this id already exists.",
        ));
    }

    // The real parser, same as every other path — scheme rules included
    // (https, or http only with allow_insecure).
    let config = tome_core::config::SourceConfig::parse_str(id.clone(), &body, &config_file)
        .map_err(|e| ApiError::bad_request(format!("invalid config: {e}")))?;

    // THE critical check (P4-010): without it this endpoint is an SSRF
    // primitive — a request could make Tome fetch an internal admin host and
    // read the result back through GET /pages. A literal-IP URL is judged
    // here; a hostname is judged at fetch time by the pinned resolver, which
    // is the only judgement that survives DNS rebinding.
    if let Some(url) = config.spec.url() {
        if let Some(ip) = literal_ip(url) {
            let policy = if config.fetch.allow_insecure {
                tome_core::ssrf::AddressPolicy::allow_private()
            } else {
                tome_core::ssrf::AddressPolicy::public_only()
            };
            if !policy.permits(ip) {
                return Err(ApiError::bad_request(
                    "the source URL points at a blocked address range",
                ));
            }
        }
    }

    state
        .paths
        .ensure_created()
        .map_err(|_| ApiError::internal())?;
    std::fs::create_dir_all(state.paths.sources_dir()).map_err(|_| ApiError::internal())?;
    std::fs::write(&config_file, &body).map_err(|_| ApiError::internal())?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id.as_str(), "created": true })),
    )
        .into_response())
}

async fn delete_source(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let id = parse_source_id(&id)?;
    let configs = crate::source_configs(&state.paths).map_err(|_| ApiError::internal())?;
    let Some((_, config_path)) = configs.iter().find(|(existing, _)| *existing == id) else {
        return Err(ApiError::not_found("source"));
    };
    // The index this server may hold open is refreshed by the deletion
    // commit; drop our cached engine so later searches reopen a fresh view.
    let pages = crate::remove::delete_source_data(&state.paths, &id, config_path)
        .map_err(|_| ApiError::internal())?;
    *state.engine.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(Json(json!({ "removed": id.as_str(), "pages": pages })))
}

#[derive(serde::Deserialize)]
struct PageListParams {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_pages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<PageListParams>,
) -> Result<Json<Value>, ApiError> {
    let id = parse_source_id(&id)?;
    if !state.paths.database_file().exists() {
        return Err(ApiError::not_found("source"));
    }
    let db = tome_core::db::Database::open(&state.paths).map_err(|_| ApiError::internal())?;
    if db
        .get_source(&id)
        .map_err(|_| ApiError::internal())?
        .is_none()
    {
        return Err(ApiError::not_found("source"));
    }
    let pages = db.list_pages(&id).map_err(|_| ApiError::internal())?;
    let total = pages.len();
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 1000);
    let items: Vec<Value> = pages
        .iter()
        .skip(offset)
        .take(limit)
        .map(|page| json!({ "path": page.path, "title": page.title }))
        .collect();
    Ok(Json(json!({
        "pages": items,
        "total": total,
        "offset": offset,
    })))
}

async fn get_page(
    State(state): State<Arc<AppState>>,
    Path((id, path)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let id = parse_source_id(&id)?;
    let path = tome_core::model::PagePath::new(path)
        .map_err(|e| ApiError::bad_request(format!("invalid page path: {e}")))?;
    let store = tome_core::store::PageStore::new(&state.paths, &id);
    let page = store
        .read(&path)
        .map_err(|_| ApiError::internal())?
        .ok_or_else(|| ApiError::not_found("page"))?;

    // Normalized, sanitized HTML — never raw upstream HTML (Appendix B).
    // Assets keep their relative `assets/…` references; an API consumer
    // resolves them against its own choice of base.
    let rendered = tome_core::render::render(
        &page.body,
        &tome_core::render::RenderOptions {
            asset_base: "",
            highlighter: tome_core::highlight::Highlighter::shared(),
        },
    );
    Ok(Json(json!({
        "title": page.title,
        "content": rendered.html,
        "toc": rendered
            .outline
            .iter()
            .map(|entry| json!({ "id": entry.id, "title": entry.title, "level": entry.level }))
            .collect::<Vec<_>>(),
    })))
}

async fn sync_source(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_source_id(&id)?;
    let configs = crate::source_configs(&state.paths).map_err(|_| ApiError::internal())?;
    let Some((_, config_path)) = configs.iter().find(|(existing, _)| *existing == id) else {
        return Err(ApiError::not_found("source"));
    };
    let config = tome_core::config::SourceConfig::parse_file(config_path)
        .map_err(|e| ApiError::bad_request(format!("the source's config is invalid: {e}")))?;

    {
        let mut syncing = state.syncing.lock().unwrap_or_else(|e| e.into_inner());
        if !syncing.insert(id.as_str().to_owned()) {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "sync_in_progress",
                "A sync for this source is already running.",
            ));
        }
    }

    // A pull is minutes of polite crawling; holding the HTTP request open
    // for it helps nobody. 202, run in the blocking pool, log the outcome.
    let task_state = Arc::clone(&state);
    let task_id = id.as_str().to_owned();
    tokio::task::spawn_blocking(move || {
        let result = tome_core::pipeline::pull(&task_state.paths, &config, &mut |_| {});
        match &result {
            Ok(report) => tracing::info!(
                source = %task_id,
                pages = report.pages_stored,
                "sync finished"
            ),
            Err(e) => tracing::warn!(source = %task_id, "sync failed: {e}"),
        }
        // Fresh view for subsequent searches, and release the in-flight slot.
        *task_state.engine.lock().unwrap_or_else(|e| e.into_inner()) = None;
        task_state
            .syncing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&task_id);
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "syncing": id.as_str() })),
    )
        .into_response())
}

fn parse_source_id(text: &str) -> Result<tome_core::model::SourceId, ApiError> {
    tome_core::model::SourceId::new(text)
        .map_err(|e| ApiError::bad_request(format!("invalid source id: {e}")))
}

/// The URL's host as an IP, when it is a literal one.
fn literal_ip(url: &url::Url) -> Option<IpAddr> {
    match url.host()? {
        url::Host::Ipv4(ip) => Some(IpAddr::V4(ip)),
        url::Host::Ipv6(ip) => Some(IpAddr::V6(ip)),
        url::Host::Domain(_) => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn strip_port_handles_ipv6() {
        assert_eq!(strip_port("localhost:7431"), "localhost");
        assert_eq!(strip_port("127.0.0.1"), "127.0.0.1");
        assert_eq!(strip_port("[::1]:7431"), "[::1]");
        assert_eq!(strip_port("[::1]"), "[::1]");
        // A spoofed host is not accidentally truncated into a valid one.
        assert_eq!(strip_port("evil.com:80"), "evil.com");
        assert_eq!(strip_port("localhost.evil.com"), "localhost.evil.com");
    }
}
