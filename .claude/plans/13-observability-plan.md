# Observability Plan

**User Telemetry:** None (privacy-first)
**Developer Observability:** OpenTelemetry for internal development/debugging
**Production Monitoring:** Local-only logging and metrics

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
// When user enables "Debug Mode" in preferences
let file_appender = tracing_appender::rolling::daily(
    "~/.tome/logs",
    "tome.log"
);
let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

tracing_subscriber::registry()
    .with(fmt::layer().with_writer(non_blocking))
    .init();
```

**Log location:** `~/.tome/logs/`

**Retention:** 7 days (auto-cleanup)

### Log Format

```
2026-01-25T10:30:45.123Z INFO  tome::scraper > Starting scrape url="https://docs.rs/serde"
2026-01-25T10:30:45.456Z DEBUG tome::scraper > Fetched page path="/serde/index.html" size=45678
2026-01-25T10:30:45.789Z INFO  tome::scraper > Scrape complete pages=234 duration=12.3s
2026-01-25T10:30:46.012Z WARN  tome::search  > Index segment corrupted, rebuilding segment=3
```

### What We Log

**Do log:**
- Operation start/complete (sync, index, search)
- Errors with context
- Performance metrics (duration, counts)
- Configuration changes

**Never log:**
- User content (bookmarks, notes, highlights)
- Full URLs (just domain for debugging)
- File paths containing usernames
- iCloud account information

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
// Only in development builds
#[cfg(debug_assertions)]
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

fn append_to_error_log(error: &TomeError) -> Result<()> {
    let path = dirs::data_dir()?.join("tome/error.log");
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
// Set up panic hook for crash reports
std::panic::set_hook(Box::new(|info| {
    let backtrace = std::backtrace::Backtrace::capture();

    let crash_report = format!(
        "Tome Crash Report\n\
         ================\n\
         Time: {}\n\
         Info: {}\n\
         Backtrace:\n{}\n",
        chrono::Utc::now(),
        info,
        backtrace
    );

    // Write to crash log
    let path = dirs::data_dir()
        .map(|p| p.join("tome/crash.log"))
        .unwrap_or_else(|| PathBuf::from("/tmp/tome-crash.log"));

    let _ = std::fs::write(&path, &crash_report);

    eprintln!("Tome crashed. Report saved to: {}", path.display());
}));
```

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
// Show notification for critical errors (user can disable)
func showErrorNotification(_ error: TomeError) {
    guard UserDefaults.standard.bool(forKey: "showErrorNotifications") else {
        return
    }

    let notification = NSUserNotification()
    notification.title = "Tome"
    notification.informativeText = error.userMessage
    notification.soundName = nil  // Silent by default

    NSUserNotificationCenter.default.deliver(notification)
}
```

---

## Debug Mode

### Enabling Debug Mode

In Preferences → Advanced:

```
[ ] Enable debug mode
    Enables verbose logging and developer tools.
    Logs are stored in ~/.tome/logs/

[ ] Show performance overlay
    Display frame rate and memory usage.
```

### Debug Mode Features

When enabled:
- `TRACE` level logging
- File logging to `~/.tome/logs/`
- Performance overlay in app
- "Copy Debug Info" in Help menu
- Expose internal state via API

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
