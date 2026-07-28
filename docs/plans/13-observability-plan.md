# Observability Plan

**User Telemetry:** None (privacy-first) — **no exceptions**
**Developer Observability:** OpenTelemetry, development builds only
**Production Monitoring:** Local-only logging and metrics

> **One contradiction resolved.** `05-phase-5-polish-launch.md` P5-004 previously listed "error
> telemetry (opt-in)" as an acceptance criterion, against this document's "no external crash
> reporting" and the NFR's "zero data collection or phone-home". Opt-in telemetry is still
> telemetry, still needs a server, still needs a privacy policy, and still contradicts the
> product's stated position. It has been removed from Phase 5. Users share a diagnostics bundle by
> hand if they choose to.

---

## Philosophy

Tome collects **zero data** from users. All observability is:
1. **Local-only** - Logs and metrics stay on user's machine
2. **Opt-in for debug** - Verbose logging only when user enables it
3. **Developer use only** - OpenTelemetry for development, not production telemetry

---

## Logging

### Log Levels

| Level | Usage | Default State |
|-------|-------|---------------|
| `ERROR` | Unrecoverable errors, crashes | Always on |
| `WARN` | Recoverable errors, degraded state | Always on |
| `INFO` | Key operations (sync start/end) | On |
| `DEBUG` | Detailed operation flow | Off |
| `TRACE` | Verbose internals | Off (dev only) |

### Log Configuration

```rust
// src-tauri/src/logging.rs
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Default: info for tome, warn for dependencies
            EnvFilter::new("warn,tome=info")
        });

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(filter)
        .init();
}
```

### Log Output

**Default:** stderr (visible in Console.app on macOS)

**Optional file logging:**
```rust
// When the user enables "Debug Mode" in preferences.
// The path comes from the path module (P1-006). A literal "~/.tome/logs" is NOT
// expanded by tracing_appender -- it would create a directory named `~` in the
// process's working directory, which is a different place for the app and the CLI.
let file_appender = tracing_appender::rolling::daily(paths.logs_dir(), "tome.log");
let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

// `guard` MUST be kept alive for the process lifetime. Binding it to `_guard`
// as the original sample did drops it immediately, and the non-blocking writer
// stops flushing -- so debug logging produces an empty file, which is the worst
// possible outcome for a diagnostic feature.
std::mem::forget(guard);   // or store it in application state

tracing_subscriber::registry()
    .with(fmt::layer().with_writer(non_blocking))
    .init();
```

**Log location:** `~/Library/Application Support/Tome/logs/` — see
[PRD § File System Layout](../PRD.md#file-system-layout). The plan previously scattered logs,
error logs, and crash logs across three different roots.

**Retention:** 7 days, size-capped as well as age-capped (a tight crawl loop can produce a great
deal of output in an hour)

### Log Format

```
2026-01-25T10:30:45.123Z INFO  tome::scraper > Starting scrape url="https://docs.rs/serde"
2026-01-25T10:30:45.456Z DEBUG tome::scraper > Fetched page path="/serde/index.html" size=45678
2026-01-25T10:30:45.789Z INFO  tome::scraper > Scrape complete pages=234 duration=12.3s
2026-01-25T10:30:46.012Z WARN  tome::search  > Index segment corrupted, rebuilding segment=3
```

### What We Log

Log lines are written to a file the user may share. Treat every field as potentially public.

**Do log:**
- Operation start/complete (sync, index, search)
- Errors with context
- Performance metrics (duration, counts)
- Configuration changes

**Never log:**
- User content (bookmarks, notes, highlights, annotation quotes)
- **Search queries** — reading history is exactly the data this product promises to keep private
- Page paths (they reveal what is being read; log a source id and a page count instead)
- Full URLs (domain only)
- File paths containing usernames (redact the home directory prefix)
- iCloud account information
- The API bearer token, in any form, at any level

```rust
// Good: anonymized logging
tracing::info!(
    domain = %url.domain().unwrap_or("unknown"),
    pages = page_count,
    "Scrape complete"
);

// Bad: leaks user data
tracing::info!("Scraped {} with bookmark {}", full_url, bookmark_title);
```

---

## Metrics (Development Only)

### OpenTelemetry Setup

For development and performance debugging:

```rust
// Only in development builds. Note the OTLP exporter and its transitive
// dependencies must not be compiled into release builds at all -- gate the
// dependency behind a Cargo feature, not just `#[cfg(debug_assertions)]`, so
// that "no telemetry" is true of the shipped binary's dependency tree and not
// merely of its control flow.
#[cfg(all(debug_assertions, feature = "otel"))]
pub fn init_otel_metrics() {
    use opentelemetry::sdk::metrics::MeterProvider;
    use opentelemetry_otlp::WithExportConfig;

    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("http://localhost:4317");

    let provider = MeterProvider::builder()
        .with_reader(
            opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
                .with_interval(Duration::from_secs(10))
                .build()
        )
        .build();

    opentelemetry::global::set_meter_provider(provider);
}
```

### Development Metrics

```rust
use opentelemetry::metrics::Meter;

lazy_static! {
    static ref METER: Meter = opentelemetry::global::meter("tome");

    // Counters
    static ref SEARCH_COUNT: Counter<u64> = METER
        .u64_counter("tome.search.count")
        .with_description("Number of searches performed")
        .init();

    // Histograms
    static ref SEARCH_LATENCY: Histogram<f64> = METER
        .f64_histogram("tome.search.latency_ms")
        .with_description("Search latency in milliseconds")
        .init();

    // Gauges
    static ref INDEX_SIZE: Gauge<u64> = METER
        .u64_gauge("tome.index.size_bytes")
        .with_description("Current index size in bytes")
        .init();
}

// Usage
pub async fn search(query: &str) -> Result<Vec<SearchResult>> {
    let start = Instant::now();

    let results = self.engine.search(query).await?;

    SEARCH_COUNT.add(1, &[]);
    SEARCH_LATENCY.record(start.elapsed().as_millis() as f64, &[]);

    Ok(results)
}
```

### Local Development Dashboard

For development, use a local observability stack:

```yaml
# docker-compose.yml (development only)
version: '3'
services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"  # UI
      - "4317:4317"    # OTLP gRPC

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
```

**Note:** This is for development only. Production builds have no telemetry.

---

## Distributed Tracing (Development)

### Trace Context

```rust
use tracing::{instrument, Span};

#[instrument(skip(self))]
pub async fn sync_source(&self, source_id: &str) -> Result<SyncStats> {
    tracing::info!("Starting sync");

    let pages = self.fetch_pages(source_id).await?;
    tracing::debug!(count = pages.len(), "Fetched pages");

    let indexed = self.index_pages(&pages).await?;
    tracing::debug!(count = indexed, "Indexed pages");

    tracing::info!("Sync complete");
    Ok(SyncStats { fetched: pages.len(), indexed })
}
```

### Span Attributes

```rust
#[instrument(
    name = "search",
    skip(self, query),
    fields(
        query_length = query.len(),
        scope = ?scope,
    )
)]
pub async fn search(&self, query: &str, scope: Option<&str>) -> Result<Vec<SearchResult>> {
    // ...

    // Add result count after execution
    Span::current().record("result_count", results.len());

    Ok(results)
}
```

---

## Health Checks

### Application Health

```rust
#[derive(Serialize)]
pub struct HealthStatus {
    pub status: HealthState,
    pub checks: HashMap<String, CheckResult>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

pub async fn check_health() -> HealthStatus {
    let mut checks = HashMap::new();

    // Database connectivity
    checks.insert("database".to_string(), check_database().await);

    // Search index
    checks.insert("search_index".to_string(), check_search_index().await);

    // iCloud sync (if enabled)
    if is_sync_enabled() {
        checks.insert("icloud_sync".to_string(), check_icloud().await);
    }

    // Determine overall status
    let status = if checks.values().all(|c| c.is_healthy()) {
        HealthState::Healthy
    } else if checks.values().any(|c| c.is_critical()) {
        HealthState::Unhealthy
    } else {
        HealthState::Degraded
    };

    HealthStatus {
        status,
        checks,
        timestamp: Utc::now(),
    }
}
```

### API Health Endpoint

```rust
// GET /api/status
pub async fn status_handler() -> Json<HealthStatus> {
    Json(check_health().await)
}
```

Response:
```json
{
  "status": "healthy",
  "checks": {
    "database": { "status": "ok", "latency_ms": 2 },
    "search_index": { "status": "ok", "documents": 15234 },
    "icloud_sync": { "status": "ok", "last_sync": "2026-01-25T10:00:00Z" }
  },
  "timestamp": "2026-01-25T10:30:00Z"
}
```

---

## Error Tracking

### Local Error Log

Errors are logged locally with context:

```rust
pub fn log_error(error: &TomeError, context: &str) {
    tracing::error!(
        error = %error,
        error_type = error.type_name(),
        context = context,
        "Operation failed"
    );

    // Also write to error log for user review
    if let Err(e) = append_to_error_log(error) {
        tracing::warn!("Failed to write error log: {}", e);
    }
}

fn append_to_error_log(paths: &Paths, error: &TomeError) -> Result<()> {
    // `dirs::data_dir()` returns Option, so `?` in a Result-returning function
    // does not compile. It also pointed at a different root than the logs above.
    // One path module, one location.
    let path = paths.logs_dir().join("error.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    writeln!(
        file,
        "[{}] {}: {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
        error.type_name(),
        error
    )?;

    Ok(())
}
```

### Crash Reports

**Local crash dumps only:**

```rust
// `Backtrace::capture()` returns `Disabled` unless RUST_BACKTRACE is set in the
// environment -- which it is not for a double-clicked .app. The original hook
// would therefore have shipped crash reports containing no backtrace: a crash
// reporting feature that reports nothing. `force_capture()` always captures.
std::panic::set_hook(Box::new(move |info| {
    let backtrace = std::backtrace::Backtrace::force_capture();

    let report = format!(
        "Tome Crash Report\n\
         =================\n\
         Version:   {}\n\
         Build:     {}\n\
         Time:      {}\n\
         Thread:    {}\n\
         Info:      {}\n\
         Backtrace:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_SHA"),
        chrono::Utc::now(),
        std::thread::current().name().unwrap_or("unnamed"),
        info,          // NOTE: a panic message can contain user data (a page path,
                       // a query). Redact before this is ever shared.
        backtrace
    );

    // Timestamped filename: `fs::write` to a fixed path overwrites the previous
    // crash, and the first crash is usually the informative one.
    let path = crash_dir().join(format!("crash-{}.log", chrono::Utc::now().timestamp()));
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(&path, &report);

    eprintln!("Tome crashed. Report saved to: {}", path.display());
}));
```

Keep the ten most recent crash reports and delete older ones. Note that a panic hook does not fire
for aborts, stack overflow, or a crash inside WebKit — those land in macOS's own
`~/Library/Logs/DiagnosticReports/`, and the diagnostics view should point users there too.

**No external crash reporting** - Users can manually share crash logs if they choose to file a bug report.

---

## Performance Monitoring

### Local Performance Metrics

```rust
pub struct PerformanceMetrics {
    // Startup
    pub cold_start_ms: u64,
    pub warm_start_ms: u64,

    // Search
    pub search_p50_ms: u64,
    pub search_p95_ms: u64,
    pub search_p99_ms: u64,

    // Indexing
    pub pages_per_second: f64,

    // Memory
    pub memory_usage_mb: u64,
}

// Collect on-demand for "About" or diagnostics view
pub fn collect_performance_metrics() -> PerformanceMetrics {
    // ... measure and return
}
```

### User-Accessible Diagnostics

In Preferences → Advanced → Diagnostics:

```
┌─────────────────────────────────────────────────────┐
│ Tome Diagnostics                                    │
├─────────────────────────────────────────────────────┤
│ Version: 1.0.0                                      │
│ Build: abc1234                                      │
│                                                     │
│ Performance                                         │
│ ─────────────────────────────────────────────────── │
│ Last startup: 423ms                                 │
│ Search latency (P95): 67ms                          │
│ Memory usage: 187 MB                                │
│                                                     │
│ Storage                                             │
│ ─────────────────────────────────────────────────── │
│ Database size: 12.4 MB                              │
│ Index size: 234.5 MB                                │
│ Cache size: 1.2 GB                                  │
│                                                     │
│ [Export Diagnostics]  [View Logs]  [Clear Cache]    │
└─────────────────────────────────────────────────────┘
```

---

## Alerting

### Local Alerts Only

No external alerting. Users see issues via:

1. **UI indicators** - Sync status, error badges
2. **macOS notifications** - For critical errors (if enabled)
3. **Log review** - In-app log viewer

```swift
// `NSUserNotification` was deprecated in macOS 10.14 and is unavailable in
// modern SDKs -- this would not compile against a macOS 12+ target.
import UserNotifications

func showErrorNotification(_ error: TomeError) {
    guard UserDefaults.standard.bool(forKey: "showErrorNotifications") else { return }

    let content = UNMutableNotificationContent()
    content.title = "Tome"
    content.body  = error.userMessage      // never the raw error: it may carry user content
    content.sound = nil                     // silent by default

    UNUserNotificationCenter.current().add(
        UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
    )
}
```

Notification authorization must be requested before the first notification, and a *denied*
authorization must not produce an error dialog — an app that nags for notification permission on
first launch is exactly the kind of thing this product's audience dislikes. Request it lazily, the
first time the user enables error notifications.

---

## Debug Mode

### Enabling Debug Mode

In Preferences → Advanced:

```
[ ] Enable debug mode
    Enables verbose logging and developer tools.
    Logs are stored in ~/Library/Application Support/Tome/logs/

[ ] Show performance overlay
    Display frame rate and memory usage.
```

### Debug Mode Features

When enabled:
- `TRACE` level logging
- File logging to the logs directory
- Performance overlay in app
- "Copy Debug Info" in Help menu — **redacted**: no page paths, no search queries, no note text,
  no home-directory username, no API token. A diagnostics bundle exists to be shared, so it must
  be safe to paste into a public issue. This is the one place where the privacy stance is most
  easily undone by accident.
- Expose internal state via a debug-only API route, **still behind the bearer token** — debug mode
  must not open an unauthenticated hole

```rust
pub fn is_debug_mode() -> bool {
    std::env::var("TOME_DEBUG").is_ok()
        || UserPreferences::get().debug_mode
}

// Enable more verbose logging in debug mode
let filter = if is_debug_mode() {
    EnvFilter::new("debug,tome=trace")
} else {
    EnvFilter::new("warn,tome=info")
};
```

---

## Summary

| Aspect | Production | Development |
|--------|------------|-------------|
| Telemetry | None | None |
| Logging | INFO level, local | TRACE level |
| Metrics | None | OpenTelemetry |
| Tracing | None | Jaeger |
| Crash reports | Local file | Local file |
| Error tracking | Local log | Local log |
| Alerting | UI/notifications | UI/notifications |
