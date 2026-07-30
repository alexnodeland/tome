# Phase 4: Automation & Integration (v0.4)

**Goal:** Programmable access and developer tool integration
**Tickets:** 18
**Effort:** ~71.5 person-days
**Prerequisites:** Phase 2 complete (can run parallel with Phase 3)
**Exit Criteria:** Claude Code can add and search docs; MCP tools work with AI agents

> **Two corrections shape this whole phase.**
>
> 1. **The local HTTP API was open to every web page the user visits.** `CorsLayer::permissive()`
>    plus "localhost is always trusted" means any site could read the user's library and drive
>    `POST /sources` — an SSRF primitive into their private network. It also contradicted
>    `12-security-considerations.md`, which specified no CORS. Fixed in P4-009 and P4-012.
> 2. **MCP over a Unix socket is not a thing.** MCP defines stdio and Streamable HTTP. As
>    originally specified, the flagship Claude Code integration could not have connected. Fixed in
>    P4-013 and P4-014.

---

## Ticket Summary

| ID | Title | Complexity | Priority | Dependencies |
|----|-------|------------|----------|--------------|
| P4-001 | Design CLI architecture | M | Critical | P2-001 |
| P4-002 | Implement CLI scaffolding (clap) | M | Critical | P4-001 |
| P4-003 | Build `tome add` command | M | High | P4-002, P2-014 |
| P4-004 | Build `tome pull` command | M | High | P4-002, P1-008 |
| P4-005 | Build `tome search` command | M | Critical | P4-002, P2-001 |
| P4-006 | Build `tome list` and `tome remove` commands | S | High | P4-002 |
| P4-007 | Add JSON output mode for CLI | S | High | P4-002 |
| P4-008 | Design local HTTP API | M | High | P2-001 |
| P4-009 | Implement HTTP server with Axum | L | Critical | P4-008 |
| P4-010 | Build API endpoints (search, sources) | M | High | P4-009 |
| P4-011 | Build API endpoints (pages, bookmarks) | M | High | P4-009 |
| P4-012 | Add API authentication (optional token) | S | Medium | P4-009 |
| P4-013 | Design MCP server architecture | M | Critical | P4-008 |
| P4-014 | Implement MCP protocol handler | L | High | P4-013 |
| P4-015 | Build MCP tools (search, get_page, list) | M | High | P4-014 |
| P4-016 | Build MCP tools (bookmark, lookup_symbol) | M | High | P4-014 |
| P4-017 | Create Claude Code plugin specification | M | High | P4-005, P4-015 |
| P4-018 | Implement sync strategy system | M | High | P1-008, P4-004 |

---

## Detailed Tickets

### P4-001: Design CLI architecture

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P2-001 (Search)
**Blocks:** P4-002

#### Description
Design the command-line interface architecture and command structure.

#### Acceptance Criteria
- [ ] Command hierarchy defined (subcommands)
- [ ] Global options specified (--json, --quiet, --config)
- [ ] Each command's arguments and options documented
- [ ] Output format specification (human and JSON)
- [ ] Error handling strategy
- [ ] Exit codes defined
- [ ] Shell completion strategy

#### Technical Notes
```
tome [global-options] <command> [command-options] [args]

Global Options:
  --json           Output as JSON (for scripting)
  --quiet          Suppress non-essential output
  --config <path>  Use alternate config file
  --help           Show help
  --version        Show version

Commands:
  add <url|path>     Add documentation source
  pull [source]      Fetch/update documentation (--all, --force, --parallel)
  search <query>     Search documentation (--scope, --limit)
  list               List all sources (--category, --json)
  remove <source>    Remove a source (--confirm)
  config [source]    View/edit configuration; `config rotate-token`
  registry           Browse/install from the source registry (list, search, add)
  serve              Start local API server (--port)
  mcp                Start MCP server (stdio; --http --port for Streamable HTTP)
  status             Show sync and index status (--show-token)
  export             Export bookmarks/annotations (--format, --output)
  import <path>      Import previously exported bookmarks/annotations
  debug <sub>        Diagnostics and recovery; hidden from top-level --help
                     check-integrity | rebuild-index | reset-sync
                     rollback-migration --version N | reset --confirm

There is deliberately no `tome sync`: `pull` fetches documentation content, and
bookmark sync is automatic and not user-invoked. Earlier drafts used `tome sync`,
`tome import`, `tome rebuild-index` and `tome debug ...` in examples across three
other documents without ever adding them here -- a CLI defined by whoever wrote the
most recent example. This list is now the complete surface.

Exit Codes:
  0: Success
  1: General error
  2: Invalid arguments
  3: Source not found
  4: Network error
  5: Configuration error
```

#### Success Metrics
- All commands documented
- Consistent option naming
- Clear error messages

---

### P4-002: Implement CLI scaffolding (clap)

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P4-001
**Blocks:** P4-003, P4-004, P4-005, P4-006, P4-007

#### Description
Set up the CLI application using clap with derive macros.

#### Acceptance Criteria
- [ ] Clap derive-based CLI structure
- [ ] Global options implemented
- [ ] Subcommand routing working
- [ ] Help text generation
- [ ] Version info from Cargo.toml
- [ ] Shell completion generation (bash, zsh, fish)
- [ ] Binary named `tome`

#### Technical Notes
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tome")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress non-essential output
    #[arg(long, short, global = true)]
    pub quiet: bool,

    /// Use alternate config file
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add documentation source
    Add {
        /// URL or local path
        target: String,

        /// Override detected platform
        #[arg(long)]
        platform: Option<String>,

        /// Category for the source
        #[arg(long)]
        category: Option<String>,
    },

    /// Fetch/update documentation
    Pull {
        /// Source name (or --all)
        source: Option<String>,

        /// Pull all sources
        #[arg(long)]
        all: bool,
    },

    /// Search documentation
    Search {
        /// Search query
        query: String,

        /// Limit to source
        #[arg(long)]
        scope: Option<String>,

        /// Max results
        #[arg(long, default_value = "10")]
        limit: usize,
    },

    // ... more commands
}
```

#### Success Metrics
- `tome --help` shows all commands
- Shell completions work
- Binary < 10MB

---

### P4-003: Build `tome add` command

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P4-002, P2-014 (Platform detection)
**Blocks:** None

#### Description
Implement the command to add new documentation sources.

#### Acceptance Criteria
- [~] Accept URL or local path — URLs done (S3-1). Local paths are *refused with an honest
      message*: `local`/`docset` ingestion does not exist in the pipeline, and accepting the
      path would write a config every pull rejects
- [x] Auto-detect platform type — `detect_site` through the ordinary fetch path; a confident
      detection picks the platform scraper, anything less falls back to generic (S3-1)
- [x] Interactive confirmation (unless --yes) — and `--yes` is *required* with `--json` or a
      non-terminal stdin, checked before any network traffic
- [x] Create config file in the sources directory (path via P1-006) — round-tripped through
      `SourceConfig::parse_file` before use, so `add` cannot write what `pull` rejects
- [x] Trigger initial pull after adding
- [x] Show progress during pull — on stderr, where every progress line goes
- [x] Handle duplicate detection — by id and by URL (the same site under two spellings)

#### Technical Notes
```rust
pub async fn add(target: &str, options: AddOptions) -> Result<()> {
    // 1. Detect if URL or path
    let source_type = if target.starts_with("http") {
        detect_platform(target).await?
    } else {
        SourceType::Local { path: PathBuf::from(target) }
    };

    // 2. Generate config
    let config = SourceConfig {
        name: derive_name(target, &source_type),
        source: source_type,
        category: options.category,
        sync: default_sync_config(),
    };

    // 3. Confirm with user
    if !options.yes {
        println!("Detected: {:?}", config.source);
        println!("Name: {}", config.name);
        if !confirm("Add this source?")? {
            return Ok(());
        }
    }

    // 4. Write config file
    let config_path = write_source_config(&config)?;
    println!("Created: {}", config_path.display());

    // 5. Initial pull
    println!("Fetching documentation...");
    pull(&config.name, PullOptions::default()).await?;

    println!("Done! {} is now available in Tome.", config.name);
    Ok(())
}
```

**Example Session:**
```bash
$ tome add https://docs.python.org/3/

Analyzing https://docs.python.org/3/...
Detected: Sphinx/ReadTheDocs documentation
Suggested name: python-3

Add this source? [Y/n] y

Created: ~/Library/Application Support/Tome/sources/python-3.yaml
Fetching documentation...
  [=====>                    ] 234/1847 pages

Done! python-3 is now available in Tome.
```

#### Success Metrics
- Detection accurate 95%+
- Config file valid YAML
- Initial pull completes

---

### P4-004: Build `tome pull` command

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P4-002, P1-008 (Scraper)
**Blocks:** P4-018

#### Description
Implement the command to fetch/update documentation.

#### Acceptance Criteria
- [ ] Pull single source by name
- [ ] Pull all sources with --all
- [ ] Show progress (pages fetched, indexed)
- [ ] Report errors per source
- [ ] Skip up-to-date sources (unless --force)
- [ ] Update last_synced timestamp
- [ ] Parallel pulling with --parallel

#### Technical Notes
```rust
pub async fn pull(source: Option<&str>, options: PullOptions) -> Result<()> {
    let sources = if options.all {
        list_all_sources()?
    } else if let Some(name) = source {
        vec![get_source(name)?]
    } else {
        return Err(anyhow!("Specify source name or use --all"));
    };

    let pb = ProgressBar::new(sources.len() as u64);

    for source in sources {
        pb.set_message(format!("Pulling {}...", source.name));

        match pull_source(&source, &options).await {
            Ok(stats) => {
                pb.println(format!(
                    "✓ {} - {} pages ({} new, {} updated)",
                    source.name, stats.total, stats.added, stats.updated
                ));
            }
            Err(e) => {
                pb.println(format!("✗ {} - {}", source.name, e));
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("Pull complete");
    Ok(())
}
```

**Example Session:**
```bash
$ tome pull rust-std

Pulling rust-std...
  [=========================] 4521/4521 pages
✓ rust-std - 4521 pages (0 new, 23 updated)

$ tome pull --all

Pulling 5 sources...
✓ rust-std - 4521 pages (0 new, 0 updated)
✓ python-3 - 1847 pages (12 new, 45 updated)
✓ react - 312 pages (0 new, 0 updated)
✗ old-lib - Error: Connection refused
✓ man-pages - 5234 pages (0 new, 0 updated)

Pull complete (4/5 succeeded)
```

#### Success Metrics
- Progress visible within 2s
- Parallel pull faster than sequential
- Errors don't stop other pulls

---

### P4-005: Build `tome search` command

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P4-002, P2-001 (Search engine)
**Blocks:** P4-017

#### Description
Implement the command to search documentation.

#### Acceptance Criteria
- [ ] Search across all sources
- [ ] Scope to single source
- [ ] Limit results (default 10)
- [ ] Show source, title, snippet
- [ ] Highlight matches in snippet
- [ ] JSON output for scripting
- [ ] Interactive mode (pick result to open)

#### Technical Notes
```rust
pub async fn search(query: &str, options: SearchOptions) -> Result<()> {
    let results = search_engine.search(
        query,
        options.scope.as_deref(),
        options.limit,
    ).await?;

    if options.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("No results for \"{}\"", query);
        return Ok(());
    }

    println!("Found {} results for \"{}\":\n", results.len(), query);

    for (i, result) in results.iter().enumerate() {
        println!("{}. {} - {}", i + 1, result.source_name.dimmed(), result.title.bold());
        println!("   {}", highlight_snippet(&result.snippet, query));
        println!("   {}\n", result.path.dimmed());
    }

    if options.interactive {
        let choice = prompt_select("Open result:", 1..=results.len())?;
        open_in_tome(&results[choice - 1])?;
    }

    Ok(())
}
```

**Example Session:**
```bash
$ tome search "async iterator"

Found 23 results for "async iterator":

1. rust-std - AsyncIterator in std::async_iter
   A trait for dealing with **async** **iterator**s.
   /std/async_iter/trait.AsyncIterator.html

2. python-3 - Asynchronous Iterators
   An **async** **iterator** can call asynchronous code in its __anext__ method.
   /library/collections.abc.html#async-iterators

3. react - Suspense for Data Fetching
   Using **async** **iterator**s with React Suspense...
   /docs/concurrent-mode-suspense.html

$ tome search "Vec::new" --scope rust-std --json
[
  {
    "source_id": "abc123",
    "source_name": "rust-std",
    "path": "/std/vec/struct.Vec.html#method.new",
    "title": "Vec::new",
    "snippet": "Constructs a new, empty Vec<T>...",
    "score": 0.98
  }
]
```

#### Success Metrics
- Results in < 200ms
- Snippets useful and highlighted
- JSON output parseable

---

### P4-006: Build `tome list` and `tome remove` commands

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P4-002
**Blocks:** None

#### Description
Implement commands to list and remove sources.

#### Acceptance Criteria
- [x] List all sources with metadata — id, live page count, category, name, sync recency (S3-1)
- [x] Filter by category — `--category`, reading the *config's* category: a source that has
      never been pulled still has one, and the database's copy is a pull-time snapshot
- [x] Show sync status — "synced 2h ago" / "never pulled"; `last_synced` (RFC 3339, `null`
      until first pull) under `--json`
- [x] JSON output for scripting
- [x] Remove by name
- [x] Confirmation before remove — default **No** (destructive; the opposite of `add`), `--yes`
      to skip, required when non-interactive
- [x] Clean up data on remove — index entries, database rows, cached content, then the config
      file **last**, so a partial failure leaves a state `remove` can be run against again

#### Technical Notes
```rust
pub async fn list(options: ListOptions) -> Result<()> {
    let sources = list_sources(options.category.as_deref())?;

    if options.json {
        println!("{}", serde_json::to_string_pretty(&sources)?);
        return Ok(());
    }

    for source in sources {
        let sync_status = format_sync_status(&source);
        println!(
            "{} ({}) - {} pages, {}",
            source.name.bold(),
            source.category.dimmed(),
            source.page_count,
            sync_status
        );
    }

    Ok(())
}

pub async fn remove(name: &str, options: RemoveOptions) -> Result<()> {
    let source = get_source(name)?;

    if !options.confirm {
        println!("This will remove {} and all its cached data.", name);
        if !confirm("Continue?")? {
            return Ok(());
        }
    }

    // Remove config file
    fs::remove_file(&source.config_path)?;

    // Remove cached data
    fs::remove_dir_all(&source.data_path)?;

    // Update index
    search_engine.remove_source(&source.id).await?;

    println!("Removed {}", name);
    Ok(())
}
```

**Example Session:**
```bash
$ tome list

rust-std (Rust) - 4521 pages, synced 2 hours ago
python-3 (Python) - 1847 pages, synced 1 day ago
react (JavaScript) - 312 pages, synced 3 days ago
man-pages (System) - 5234 pages, synced 1 week ago

$ tome remove old-lib

This will remove old-lib and all its cached data.
Continue? [y/N] y

Removed old-lib
```

#### Success Metrics
- List shows all sources
- Remove cleans up completely
- Confirmation prevents accidents

---

### P4-007: Add JSON output mode for CLI

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P4-002
**Blocks:** None

#### Description
Ensure all CLI commands support structured JSON output.

#### Acceptance Criteria
- [x] --json flag on all relevant commands — `add`, `pull`, `list`, `search`, `remove`,
      `status` (S3-1)
- [x] Consistent JSON structure — one stable shape per command, every key present even when
      empty, so `jq` needs no special cases
- [x] Errors as JSON (with --json) — `{"error": {"message": …}}` on **stderr**, stdout empty,
      so a piped stdout never receives half a result and then an error
- [~] Streaming JSON for large outputs — nothing the CLI emits today is large enough to
      stream; a single document is easier for every consumer. Revisit if an output ever
      exceeds what a script would buffer anyway
- [x] Exit codes preserved with JSON — non-zero on error, asserted by test
- [x] Documented JSON schemas — the shapes and the error contract are in
      [PRD § CLI Specification](../PRD.md#cli-specification), prose rather than JSON Schema:
      six small stable shapes, and a schema document would be a second copy of the truth

#### Technical Notes
```rust
#[derive(Serialize)]
struct CliOutput<T: Serialize> {
    success: bool,
    data: Option<T>,
    error: Option<CliError>,
}

#[derive(Serialize)]
struct CliError {
    code: String,
    message: String,
    details: Option<Value>,
}

fn output<T: Serialize>(result: Result<T>, json_mode: bool) {
    match (result, json_mode) {
        (Ok(data), true) => {
            println!("{}", serde_json::to_string(&CliOutput {
                success: true,
                data: Some(data),
                error: None,
            }).unwrap());
        }
        (Err(e), true) => {
            eprintln!("{}", serde_json::to_string(&CliOutput::<()> {
                success: false,
                data: None,
                error: Some(CliError::from(e)),
            }).unwrap());
            std::process::exit(1);
        }
        (Ok(data), false) => { /* human output */ }
        (Err(e), false) => { /* human error */ }
    }
}
```

#### Success Metrics
- All commands support --json
- JSON is valid and parseable
- jq-compatible output

---

### P4-008: Design local HTTP API

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P2-001 (Search)
**Blocks:** P4-009, P4-013

#### Description
Design the REST API for local programmatic access.

#### Acceptance Criteria
- [ ] RESTful resource design
- [ ] OpenAPI specification
- [ ] Endpoint documentation
- [ ] Request/response schemas
- [ ] Error response format
- [ ] Rate limiting strategy
- [ ] CORS configuration

#### Technical Notes
```yaml
# OpenAPI Specification (partial)
openapi: 3.0.0
info:
  title: Tome Local API
  version: 1.0.0
  description: Local HTTP API for Tome documentation manager

servers:
  - url: http://localhost:7431/api

paths:
  /search:
    get:
      summary: Search documentation
      parameters:
        - name: q
          in: query
          required: true
          schema:
            type: string
        - name: scope
          in: query
          schema:
            type: string
        - name: limit
          in: query
          schema:
            type: integer
            default: 10
      responses:
        '200':
          description: Search results
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/SearchResponse'

  /sources:
    get:
      summary: List all sources
    post:
      summary: Add a new source

  /sources/{id}:
    get:
      summary: Get source details
    delete:
      summary: Remove source

  /sources/{id}/pages:
    get:
      summary: List pages in source

  /sources/{id}/pages/{path}:
    get:
      summary: Get page content

  /sources/{id}/sync:
    post:
      summary: Trigger sync

  /bookmarks:
    get:
      summary: List bookmarks
    post:
      summary: Create bookmark

  /status:
    get:
      summary: Health and status

components:
  schemas:
    SearchResponse:
      type: object
      properties:
        results:
          type: array
          items:
            $ref: '#/components/schemas/SearchResult'
        total:
          type: integer
        query_time_ms:
          type: integer

    SearchResult:
      type: object
      properties:
        source_id:
          type: string
        source_name:
          type: string
        page_path:
          type: string
        title:
          type: string
        snippet:
          type: string
        score:
          type: number
```

#### Success Metrics
- OpenAPI spec complete
- All endpoints documented
- Schema validation passes

---

### P4-009: Implement HTTP server with Axum

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P4-008
**Blocks:** P4-010, P4-011, P4-012

#### Description
Build the HTTP server using Axum framework.

#### Acceptance Criteria
- [x] Axum server setup (S3-5, `tome-cli/src/serve.rs`)
- [x] **Server does not start unless explicitly enabled** — it exists only behind `tome serve`
- [x] Configurable port (default 7431; `--port 0` picks an ephemeral one, printed to stderr);
      bind failure names the likely cause
- [x] Binds `127.0.0.1` only; `--bind` elsewhere logs a warning that the token is now all that
      stands between the network and the library
- [x] **Bearer token required on every route except `GET /api/v1/status`** — including loopback,
      no bypass, no opt-out
- [x] **No CORS headers by default.** Allowlist opt-in via `--allow-origin` (repeatable); `*` is
      rejected before the listener exists. An allowlisted origin is echoed exactly, never `*`
- [x] `Host` and `Origin` validated (DNS-rebinding defence) — and a request carrying a
      non-allowlisted `Origin` is refused outright, not merely denied CORS headers: header
      absence only hides the response, refusal also stops side effects
- [x] All routes under `/api/v1/`
- [x] Uniform JSON error envelope (see PRD Appendix B) — asserted on every error path in test
- [x] Per-token rate limiting — one fixed window (there is one token), placed **before** auth so
      token brute-force burns the same budget as everything else; tested with 150 wrong-token
      requests
- [x] Request logging that never records query strings or page paths (they are user content)
- [x] Graceful shutdown (ctrl-c)

#### Technical Notes
```rust
use axum::{Router, routing::{get, post, delete}, Extension};
use tower_http::cors::CorsLayer;

pub async fn run_server(config: ServerConfig) -> Result<()> {
    let app_state = Arc::new(AppState::new()?);

    let app = Router::new()
        // Search
        .route("/api/search", get(handlers::search))

        // Sources
        .route("/api/sources", get(handlers::list_sources))
        .route("/api/sources", post(handlers::add_source))
        .route("/api/sources/:id", get(handlers::get_source))
        .route("/api/sources/:id", delete(handlers::delete_source))
        .route("/api/sources/:id/pages", get(handlers::list_pages))
        .route("/api/sources/:id/pages/*path", get(handlers::get_page))
        .route("/api/sources/:id/sync", post(handlers::sync_source))

        // Bookmarks
        .route("/api/bookmarks", get(handlers::list_bookmarks))
        .route("/api/bookmarks", post(handlers::create_bookmark))
        .route("/api/bookmarks/:id", delete(handlers::delete_bookmark))

        // Status
        .route("/api/status", get(handlers::status))

        // Middleware. Order matters: outermost runs first.
        .layer(middleware::from_fn(guard_origin_and_host)) // DNS-rebinding defence
        .layer(middleware::from_fn_with_state(state.clone(), require_bearer_token))
        .layer(cors_layer(&config))                        // strict allowlist; NEVER permissive
        .layer(TraceLayer::new_for_http())
        .layer(Extension(app_state));

    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// No allowlist configured => emit no CORS headers at all, so browsers cannot
/// read responses cross-origin. `CorsLayer::permissive()` is never correct for a
/// localhost service holding user data.
fn cors_layer(config: &ServerConfig) -> CorsLayer {
    match config.allowed_origins.as_slice() {
        [] => CorsLayer::new(),
        origins => CorsLayer::new()
            .allow_origin(origins.iter().map(|o| o.parse().unwrap()).collect::<Vec<_>>())
            .allow_methods([Method::GET, Method::POST, Method::DELETE])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
    }
}

/// Binding to 127.0.0.1 does not stop a hostile page: an attacker-controlled name
/// can resolve to 127.0.0.1 (DNS rebinding) and the request still arrives here.
/// Rejecting unexpected Host/Origin values is what actually closes that door.
async fn guard_origin_and_host(req: Request, next: Next) -> Result<Response, ApiError> {
    let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok());
    match host.map(|h| h.split(':').next().unwrap_or(h)) {
        Some("localhost") | Some("127.0.0.1") | Some("[::1]") => {}
        _ => return Err(ApiError::Forbidden("unexpected Host header")),
    }
    if let Some(origin) = req.headers().get(header::ORIGIN) {
        if !is_allowed_origin(origin) {
            return Err(ApiError::Forbidden("origin not allowed"));
        }
    }
    Ok(next.run(req).await)
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.ok();
}
```

#### Success Metrics
- Server starts in < 1s
- Handles 100 concurrent requests
- Graceful shutdown works

---

### P4-010: Build API endpoints (search, sources)

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P4-009
**Blocks:** None

#### Description
Implement the core API endpoints for search and sources.

#### Acceptance Criteria
- [x] GET /api/v1/search — q/scope/limit; `total_hits` counts matching documents (capped at
      1000, flagged by `total_capped`), never the page size; snippets from the store (the index
      stores no bodies)
- [x] GET /api/v1/sources — list all sources
- [x] POST /api/v1/sources?id=… — body is the YAML config (JSON parses for free: YAML is a
      superset, so there is one parser and no content sniffing); the id is a query parameter
      because the file name IS the identity everywhere else in Tome
- [x] GET /api/v1/sources/{id} — source details
- [x] DELETE /api/v1/sources/{id} — same four-place cleanup as `tome remove`, shared code
- [x] POST /api/v1/sources/{id}/sync — 202, pull runs in the background, 409 while one is
      already running for that source
- [x] Proper status codes — including 409 on duplicate id and 413 on oversized config
- [x] Validation errors with details — the config parser's own message, which names the field

#### Technical Notes
```rust
pub async fn search(
    Query(params): Query<SearchParams>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<SearchResponse>, ApiError> {
    let limit = params.limit.unwrap_or(10).min(MAX_LIMIT);   // bound it: `limit=1000000` is a DoS
    let started = Instant::now();

    // `total_hits` is the count of matching documents, which is NOT `results.len()`
    // once a limit is applied. The original returned the page size as the total, so
    // every paginated response reported the wrong number.
    let hits = state.search_engine
        .search(&params.q, params.scope.as_deref(), limit)
        .await?;

    Ok(Json(SearchResponse {
        returned: hits.results.len(),
        total_hits: hits.total,
        // Originally `0, // TODO: measure` -- a placeholder in the published API contract.
        query_time_ms: started.elapsed().as_millis() as u64,
        results: hits.results,   // moved last: the original read `results.len()` after moving it
    }))
}

pub async fn add_source(
    Extension(state): Extension<Arc<AppState>>,
    body: String, // Accept YAML or JSON
) -> Result<Json<Source>, ApiError> {
    if body.len() > MAX_CONFIG_BYTES {
        return Err(ApiError::PayloadTooLarge);
    }

    // Sniffing on a leading '{' is fragile -- leading whitespace, a BOM, or a JSON
    // array all defeat it. Prefer the Content-Type header and fall back to trying
    // JSON then YAML.
    let config: SourceConfig = parse_source_config(&body, content_type)?;

    // THE critical check. Without it this endpoint is a server-side request forgery
    // primitive: any local process -- or any web page, before the CORS fix -- could
    // make Tome fetch http://169.254.169.254/ or an internal admin host and then read
    // the result back through GET /pages.
    validate_source_url(config.url())?;

    let source = state.source_manager.add(config).await?;
    Ok(Json(source))
}

pub async fn sync_source(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<SyncResult>, ApiError> {
    let result = state.sync_manager
        .sync_source(&id)
        .await?;

    Ok(Json(result))
}
```

#### Success Metrics
- Search endpoint < 100ms
- Source CRUD working
- Sync triggers correctly

---

### P4-011: Build API endpoints (pages, bookmarks)

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P4-009
**Blocks:** None

#### Description
Implement API endpoints for pages and bookmarks.

#### Acceptance Criteria
- [x] GET /api/v1/sources/{id}/pages — list pages, navigation order
- [x] GET /api/v1/sources/{id}/pages/{path} — get page content
- [~] GET/POST/DELETE /api/v1/bookmarks — **not built**: no bookmark model until Phase 3; the
      routes land with P3's bookmark work
- [x] Page content as rendered HTML — the same normalize→sanitize→render path the reader uses,
      never raw upstream HTML, with the page outline alongside
- [x] Pagination for large page lists — offset/limit, bounded

#### Technical Notes
```rust
pub async fn get_page(
    Path((source_id, page_path)): Path<(String, String)>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<PageContent>, ApiError> {
    let page = state.page_repository
        .get(&source_id, &page_path)
        .await?
        .ok_or(ApiError::NotFound)?;

    let content = state.renderer
        .render_page(&page)
        .await?;

    Ok(Json(PageContent {
        title: page.title,
        content,
        toc: extract_toc(&content),
    }))
}

pub async fn create_bookmark(
    Extension(state): Extension<Arc<AppState>>,
    Json(input): Json<CreateBookmarkInput>,
) -> Result<Json<Bookmark>, ApiError> {
    let bookmark = Bookmark {
        id: Uuid::new_v4(),
        source_id: input.source_id,
        page_path: input.page_path,
        title: input.title,
        note: input.note,
        scroll_position: None,
        highlights: vec![],
        collections: vec![],
        created_at: Utc::now(),
        modified_at: Utc::now(),
        sync_status: SyncStatus::Pending,
        device_id: state.device_id.clone(),
    };

    state.bookmark_repository.create(&bookmark).await?;

    Ok(Json(bookmark))
}
```

#### Success Metrics
- Page retrieval < 50ms
- Bookmark CRUD complete
- Pagination works

---

### P4-012: Add API authentication (optional token)

**Priority:** Critical
**Complexity:** S (1-2 days)
**Dependencies:** P4-009
**Blocks:** None

#### Description
Implement mandatory token authentication for API access.

**Priority raised from Medium to Critical.** Authentication was specified as optional and
Medium-priority for a service that exposes the user's entire reading history and can be made to
fetch arbitrary URLs. It is the control that makes the rest of the API safe to ship.

#### Acceptance Criteria
- [x] **Token required for every request, loopback included.** No bypass, no opt-out. (S3-5)
- [x] Token generated on first run from a CSPRNG (≥ 256 bits, `/dev/urandom`), stored in the
      macOS Keychain via `/usr/bin/security` (absolute path, argument vector — the man-ingest
      rule). When `TOME_HOME` is set (tests, throwaway libraries) a `0600` file under the state
      root is used instead, because a test run must not write to the user's real Keychain
- [x] Token in `Authorization: Bearer` header; compared in constant time (both sides hashed,
      branch-free digest compare; the server keeps only the hash after startup)
- [x] `tome status --show-token` prints it; `tome config rotate-token` replaces it — and says
      that a running server keeps its startup token until restarted
- [x] Token never written to logs, never included in error messages, never in the config YAML —
      `--show-token` is the one place it is ever printed
- [x] `GET /api/v1/status` is the only unauthenticated route and returns only status + version —
      the test counts the keys
- [x] Clear, non-leaky error for missing/invalid token (401, no hint about why) — missing,
      malformed, wrong and rotated-away tokens produce byte-identical responses, pinned by test
- [x] Test asserts that a request with no token, a wrong token, and a token from a previous
      rotation are all rejected

#### Technical Notes
```rust
pub async fn require_bearer_token(
    headers: HeaderMap,
    Extension(state): Extension<Arc<AppState>>,
    request: Request<Body>,
    next: Next<Body>,
) -> Result<Response, ApiError> {
    // NOTE: there is deliberately no loopback bypass and no `require_auth` opt-out.
    //
    // The original middleware returned early for `addr.ip().is_loopback()`. On a
    // desktop machine loopback is not a trust boundary: every other process on the
    // system, and every web page in the user's browser, can originate a loopback
    // request. The bypass exempted precisely the attacker we care about.
    //
    // Unauthenticated status is routed around this layer, not special-cased here.

    // Validate token
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match token {
        // Constant-time comparison: a naive `==` on a secret leaks its prefix
        // through timing to any local process willing to measure.
        Some(t) if state.validate_token(t) => Ok(next.run(request).await),
        _ => Err(ApiError::Unauthorized),
    }
}
```

#### Success Metrics
- Localhost always works
- Invalid token rejected
- Token rotation works

---

### P4-013: Design MCP server architecture

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P4-008
**Blocks:** P4-014

#### Description
Design the Model Context Protocol server architecture.

#### Acceptance Criteria
- [x] MCP specification reviewed against the **current** protocol revision (SPIKE-008) — and the
      review's conclusion is to *not* build the current revision: 2026-07-28 abolished the
      handshake, but the shipping client (Claude Code 2.1.220, measured) still speaks the legacy
      `2025-11-25` handshake and has no fall-forward against a modern-only server. Legacy now,
      dual-era additively later; [the spike write-up](../spikes/008-mcp-protocol.md) owns this
- [x] Tool definitions with JSON Schema inputs and documented outputs (S3-2/3)
- [~] Transport: stdio default (done), Streamable HTTP optional — `--http` bails with a pointer
      until S3-5: it reuses the HTTP API's bearer auth and Host/Origin guard, and shipping it
      without them would be the unauthenticated listener P4-012 exists to prevent
- [x] Concurrency model defined for multiple simultaneous server processes — spawned per client,
      short-lived, no exclusive locks: the index opens read-only and lazily, SQLite's own locking
      covers the database. *Within* a session requests are handled serially, deliberately: the
      real client issues them serially (measured), and every tool call is a local read measured
      in microseconds
- [x] Error handling that returns actionable tool errors rather than JSON-RPC transport errors —
      tool failures are `isError` results whose text names the remedy; JSON-RPC errors are
      reserved for protocol faults (unknown tool, malformed request)
- [x] Result size budget and truncation semantics defined per tool (S3-4) — 48 KiB for
      `tome_get_page`, cut at a block boundary, notice listing the sections; `section` selects a
      heading subtree; other tools bound themselves via `limit`. See "Tool result sizing" below
- [~] Timeouts on every tool call — no timeout mechanism, deliberately: every tool call is a
      synchronous local read (index, SQLite, page store) with no network and no user input; the
      hang a timeout would guard against has no source. Revisit if a tool ever fetches
- [~] **Write-capable tools (`tome_bookmark`) are opt-in**, disabled by default — currently
      **absent** rather than opt-in: no bookmark model exists until Phase 3, so no write-capable
      tool exists at all. The opt-in requirement binds whoever adds the first one; the threat
      note stands: the docs Tome ingests are untrusted text that agents will read

#### Technical Notes
```typescript
// MCP Tool Definitions
interface TomeTools {
  // Search across all or scoped documentation
  tome_search: {
    description: "Search Tome documentation library";
    parameters: {
      query: string;        // required
      scope?: string;       // source ID to limit search
      limit?: number;       // max results (default 10)
    };
    returns: SearchResult[];
  };

  // Get page content
  tome_get_page: {
    description: "Retrieve documentation page content";
    parameters: {
      source_id: string;    // required
      page_path: string;    // required
    };
    returns: { title: string; content: string; toc: Section[] };
  };

  // List sources
  tome_list_sources: {
    description: "List all available documentation sources";
    parameters: {};
    returns: Source[];
  };

  // Get TOC
  tome_get_toc: {
    description: "Get table of contents for a documentation source";
    parameters: {
      source_id: string;    // required
    };
    returns: TableOfContents;
  };

  // Bookmark
  tome_bookmark: {
    description: "Bookmark a documentation page";
    parameters: {
      source_id: string;    // required
      page_path: string;    // required
      note?: string;        // optional note
    };
    returns: Bookmark;
  };

  // Symbol lookup
  tome_lookup_symbol: {
    description: "Look up a symbol (function, type, module) in documentation";
    parameters: {
      symbol: string;       // required
      language?: string;    // optional language filter
    };
    returns: SymbolResult[];
  };
}
```

**Transport: stdio by default.**

> **Correction.** The original design specified a Unix domain socket. MCP does not define a raw
> Unix socket transport — it defines **stdio** and **Streamable HTTP**. No MCP client, including
> Claude Code, can connect to `~/.tome/mcp.sock`. The headline integration of this entire phase
> would not have worked with its headline client. There is no `mcp.sock`.

| Transport | Status | How it is used |
|-----------|--------|----------------|
| **stdio** | Default | The client spawns `tome mcp`; JSON-RPC framed over stdin/stdout |
| **Streamable HTTP** | Opt-in | `tome mcp --http --port 7432`; same bearer token, same Host/Origin guard as the HTTP API |

What the user actually configures:

```json
{ "mcpServers": { "tome": { "command": "tome", "args": ["mcp"] } } }
```

**Consequences of stdio that the design must account for:**

- The server is **spawned per client and is short-lived**. It cannot hold an exclusive lock on the
  index or the database: open Tantivy read-only, and let SQLite's normal locking handle writes.
  Multiple MCP clients plus the app may run concurrently.
- **Nothing may be written to stdout except protocol messages.** A stray `println!` corrupts the
  stream and the client disconnects with an opaque parse error. Logging goes to stderr, always.
  This deserves a lint or a wrapper type around stdout.
- Startup must be fast — the client waits on `initialize`. Defer index opening until the first
  tool call.

**Tool result sizing** (implemented by S3-4). A documentation page can be enormous, and SPIKE-008
measured what happens: an oversized result survives the transport but the client diverts it to a
file — the model receives a filename instead of the page. `tome_get_page` results are therefore
capped at **48 KiB** (~12k tokens, inside Claude Code's default 25k-token cap with room for the
turn), cut at a **block boundary** — half a code fence swallows the rest of the conversation —
with a `[truncated: …]` notice that lists the page's sections, so the remedy is in the text the
model just read. The `section` argument takes a heading anchor id (`{#id}` in page output) and
returns that heading's subtree, up to the next heading of the same or higher level. A section id
lives either on the heading itself or — Sphinx's shape — on an `Anchor` block immediately before
it; both resolve. The other tools bound themselves through their `limit` parameters.

#### Success Metrics
- All tools defined clearly
- Protocol compatible with MCP spec
- Transport configurable

---

### P4-014: Implement MCP protocol handler

**Priority:** High
**Complexity:** L (1-2 weeks)
**Dependencies:** P4-013
**Blocks:** P4-015, P4-016

#### Description
Implement the MCP protocol server.

#### Acceptance Criteria
- [x] JSON-RPC 2.0 over stdio, correctly framed (S3-2, `tome-cli/src/mcp.rs`)
- [x] `initialize` handshake with version negotiation; accepts the client's version when
      supported — echoing the client's own version is load-bearing: SPIKE-008 measured that an
      answer the client does not recognise is a **silent drop**, tools absent, no error anywhere
- [x] `notifications/initialized` handled — and never answered; a reply to a notification is
      itself a protocol violation, pinned by test
- [x] `tools/list` and `tools/call` implemented
- [x] Error responses per spec, distinguishing protocol errors from tool errors
- [ ] Optional Streamable HTTP listener behind `--http`, reusing the API's auth and Host/Origin
      guard — **S3-5**, which builds that auth
- [x] **Nothing but protocol messages on stdout**; all logging to stderr — enforced by a test
      that runs the binary with `RUST_LOG=trace` (the configuration most likely to produce a
      stray write) and asserts every stdout line parses as JSON-RPC
- [~] Concurrent request handling within a session — serial, deliberately: the real client sends
      strictly serially (measured in every SPIKE-008 session) and tool calls are microsecond
      local reads. The serve loop is the single place a worker pool would go if a client ever
      interleaves
- [x] Clean exit when stdin closes — pinned by test; `wait_with_output` would hang forever on a
      server that looped on EOF

#### Technical Notes
```rust
use tokio::io::{stdin, stdout, AsyncBufReadExt, BufReader, BufWriter};

pub struct McpServer {
    tools: HashMap<String, Box<dyn McpTool>>,
    state: Arc<AppState>,
}

impl McpServer {
    /// One process, one client, stdio. No listener, no accept loop, no `self.clone()`
    /// (the original sample cloned a struct that is not Clone).
    pub async fn run_stdio(self) -> Result<()> {
        let mut reader = BufReader::new(stdin());
        let mut writer = BufWriter::new(stdout());

        // Everything diagnostic goes to stderr. A single stray write to stdout
        // corrupts the JSON-RPC stream and the client disconnects.
        tracing::info!(target: "stderr", "MCP server ready (stdio)");

        while let Some(request) = read_message(&mut reader).await? {   // None => stdin closed
            let response = match request.method.as_str() {
                "initialize"              => self.handle_initialize(&request).await,
                "notifications/initialized" => { continue }            // notification: no reply
                "tools/list"              => self.handle_list_tools(&request).await,
                "tools/call"              => self.handle_call_tool(&request).await,
                _ => JsonRpcResponse::error(request.id, -32601, "Method not found"),
            };
            write_message(&mut writer, &response).await?;              // flush every message
        }
        Ok(())   // stdin closed: exit cleanly rather than looping on EOF
    }
}
```

Note the original loop had no EOF handling: `read_message` on a closed stream would either error
every iteration or spin forever, leaving orphaned processes behind every disconnected client.

#### Success Metrics
- Handles MCP initialize handshake
- Tools discoverable
- Concurrent requests work

---

### P4-015: Build MCP tools (search, get_page, list)

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P4-014
**Blocks:** P4-017

#### Description
Implement core MCP tools for search and retrieval.

#### Acceptance Criteria
- [x] tome_search tool working (S3-3) — results as text lines, corrections announced first
- [x] tome_get_page tool working — the stored AST rendered as markdown with heading anchors
      kept (`{#id}`), which is what S3-4's `section` argument will address into
- [x] tome_list_sources tool working
- [x] tome_get_toc tool working — pages in navigation order, the same ordinal that stops the
      Cargo Book opening on its changelog
- [x] Proper error handling — tool errors name the remedy ("call tome_list_sources", "tome_search
      finds pages that exist"): the consumer is a model, and it can act on a sentence but not on
      a code
- [~] Reasonable timeouts — none, for P4-013's reason: synchronous local reads with no network
      have no hang to guard against
- [x] Documentation for each tool — the `description` fields are the documentation the client
      shows the model; specified in P4-013

#### Technical Notes
```rust
pub struct SearchTool {
    search_engine: Arc<SearchEngine>,
}

#[async_trait]
impl McpTool for SearchTool {
    fn name(&self) -> &str { "tome_search" }

    fn description(&self) -> &str {
        "Search Tome documentation library"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "scope": { "type": "string", "description": "Source ID to limit search" },
                "limit": { "type": "integer", "description": "Max results", "default": 10 }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value> {
        let query = params["query"].as_str().ok_or(anyhow!("query required"))?;
        let scope = params["scope"].as_str();
        let limit = params["limit"].as_u64().unwrap_or(10) as usize;

        let results = self.search_engine.search(query, scope, limit).await?;

        Ok(serde_json::to_value(results)?)
    }
}
```

#### Success Metrics
- Search returns relevant results
- Page content retrieved correctly
- Tools usable by Claude

---

### P4-016: Build MCP tools (bookmark, lookup_symbol)

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P4-014
**Blocks:** None

#### Description
Implement additional MCP tools for bookmarks and symbols.

#### Acceptance Criteria
- [~] tome_bookmark tool working — **not built**: there is no bookmark model until Phase 3, and
      P4-013 makes write-capable tools opt-in besides. Building it lands with P3's bookmark work
- [x] tome_lookup_symbol tool working (S3-3)
- [~] Bookmark creates entry in database — with tome_bookmark, above
- [x] Symbol lookup uses search index — the `@symbol` declared-only query (P2-015)
- [~] Language filtering works — **dropped**: symbols are extracted from headings and carry no
      language, so a `language` parameter would be accepted and ignored, which is worse than its
      absence. `scope` (a source id) is the filter that exists and is implemented
- [x] Error handling for not found — "no page declares X; tome_search finds pages that merely
      mention it"

#### Technical Notes
```rust
pub struct BookmarkTool {
    bookmark_repo: Arc<BookmarkRepository>,
    page_repo: Arc<PageRepository>,
}

#[async_trait]
impl McpTool for BookmarkTool {
    fn name(&self) -> &str { "tome_bookmark" }

    async fn call(&self, params: Value) -> Result<Value> {
        let source_id = params["source_id"].as_str().ok_or(anyhow!("source_id required"))?;
        let page_path = params["page_path"].as_str().ok_or(anyhow!("page_path required"))?;
        let note = params["note"].as_str();

        // Get page title
        let page = self.page_repo.get(source_id, page_path).await?
            .ok_or(anyhow!("Page not found"))?;

        let bookmark = Bookmark::new(source_id, page_path, &page.title, note);
        self.bookmark_repo.create(&bookmark).await?;

        Ok(serde_json::to_value(bookmark)?)
    }
}

pub struct LookupSymbolTool {
    search_engine: Arc<SearchEngine>,
}

#[async_trait]
impl McpTool for LookupSymbolTool {
    fn name(&self) -> &str { "tome_lookup_symbol" }

    async fn call(&self, params: Value) -> Result<Value> {
        let symbol = params["symbol"].as_str().ok_or(anyhow!("symbol required"))?;
        let language = params["language"].as_str();

        // Use symbol-aware search
        let query = format!("@{}", symbol);
        let scope = language.map(|l| format!("language:{}", l));

        let results = self.search_engine.search_symbols(&query, scope.as_deref()).await?;

        Ok(serde_json::to_value(results)?)
    }
}
```

#### Success Metrics
- Bookmarks created via MCP
- Symbol lookup accurate
- Language filter works

---

### P4-017: Create Claude Code plugin specification

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P4-005 (CLI search), P4-015 (MCP tools)
**Blocks:** None

#### Description
Define and document the Claude Code plugin integration.

#### Acceptance Criteria
- [x] Plugin manifest specification — `dist/claude-plugin/.claude-plugin/plugin.json`, and
      **the format below was wrong**: see the correction
- [x] Slash commands defined — `/tome:add`, `/tome:search`, `/tome:list`, `/tome:pull`,
      `/tome:remove`, as markdown files under `commands/`
- [x] MCP tool integration documented — bundled via `.mcp.json`, no user config needed
- [x] Installation instructions (`dist/claude-plugin/README.md`)
- [x] Example workflows documented
- [x] Error handling guidelines — a table mapping each tool error to what it means, because
      the tools' errors are written to be relayed rather than worked around

#### Technical Notes

> **Correction: the manifest format below never existed.** There is no
> `slash_commands:` YAML and no `handler:` field; a Claude Code plugin is a **directory**, its
> manifest is JSON at `.claude-plugin/plugin.json` (only `name` is required), slash commands are
> **markdown files** under `commands/` whose frontmatter carries `description`, `argument-hint`
> and `allowed-tools`, and a bundled MCP server is standard MCP config in `.mcp.json` at the
> plugin root. This is the same class of error as the Unix-socket MCP transport: a surface
> specified from imagination rather than from the tool. The shipped plugin is built to the real
> format and passes `claude plugin validate`.

```
dist/claude-plugin/
├── .claude-plugin/plugin.json   # name (required), version, description, author, …
├── .mcp.json                    # { "mcpServers": { "tome": { "command": "tome", "args": ["mcp"] } } }
├── commands/{add,search,list,pull,remove}.md
└── README.md
```

```markdown
---
description: Search the Tome documentation library
argument-hint: <query> [--scope <source-id>]
allowed-tools: mcp__plugin_tome_tome__tome_search, mcp__plugin_tome_tome__tome_get_page
---
Search the user's Tome library for: **$ARGUMENTS**
```

Note the tool names: a plugin's own MCP tools are scoped
`mcp__plugin_<plugin>_<server>__<tool>`, so an `allowed-tools` entry written against the bare
name silently never matches.

Two behaviours the command bodies encode, both learned from the CLI's own design:

- **`--yes` is mandatory in every command that mutates.** A Claude Code session has no terminal,
  so an interactive confirmation cannot be answered and the command refuses. For `/tome:remove`
  that makes the assistant's own confirmation the only one there is, which is why its body asks
  first and runs second.
- **`/tome:search` reads the best result** rather than answering from titles. A title match is
  not an answer, and opening a local page is free.

**Example Workflow:**
```
User: /tome:add https://docs.pola.rs/

Claude Code:
  → Invokes: tome add https://docs.pola.rs/ --yes --json
  → Parses response
  → "I've added Polars documentation to Tome. Detected MkDocs
     (confidence 0.90), so it uses the MkDocs scraper.

     Found 847 pages. Would you like me to search for something
     specific?"

User: How do I create a DataFrame?

Claude Code:
  → Uses MCP tool: tome_search("create DataFrame", scope: "polars")
  → Gets relevant results
  → Uses MCP tool: tome_get_page(source_id, top_result.path)
  → "Here's how to create a DataFrame in Polars:
     [formatted content from docs]"
```

#### Success Metrics
- Plugin spec complete
- All commands documented
- MCP integration clear

---

### P4-018: Implement sync strategy system

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-008 (Scraper), P4-004 (Pull command)
**Blocks:** None

#### Description
Implement the configurable sync strategy system.

Implemented by **S3-7** as `tome_core::sync::due` — a pure decision function plus one caller,
`tome pull --all --due`. **It decides; it never fetches.** That split is what makes every case
below a unit test with its own clock rather than a timing-dependent hope, and it is the direct
answer to the polling defect recorded in the technical note.

- [x] Manual sync (only when triggered) — `Due::OnlyManually`
- [x] On-launch sync (check at app start) — and **not** on every tick, which is the defect a
      single `bool` invites: an on-launch source re-fetched every 15 minutes for as long as the
      app stays open. `Trigger::{Launch, Tick}` distinguishes them and a test pins it
- [x] Scheduled sync (daily, weekly, monthly), with missed schedules coalesced — not replayed.
      Coalescing falls out of comparing `last_synced` against a threshold rather than counting
      elapsed periods; the test closes the laptop for six weeks and asserts one sync, then
      asserts it is not due again
- [~] Watch sync: registry checked at most daily per source, jittered, conditional requests —
      **not built, on purpose**
- [~] Per DEC-006, `watch` notifies by default rather than fetching — **DEC-006 is still open
      and is the owner's call.** `due` returns `Due::WatchUndecided`, which `should_fetch()`
      reports as false: the conservative direction makes no request, and a test asserts it, so
      implementing `watch` here instead of deciding it first fails loudly
- [x] Version pinning (ignore updates) — `pin_version` beats every strategy and both triggers,
      tested across the cross-product
- [~] Background sync is cancellable and yields to user-initiated work — no background sync
      loop exists yet; `--due` is invoked, not scheduled. The daemon that would need cancelling
      belongs with the app's launch path (Stage 4)
- [x] Sync state persisted across restarts — `last_synced` is a database column, and `due` takes
      it as a parameter, so restarts are not a special case
- [~] Concurrency capped: a `--all` pull must not open 50 simultaneous crawls — `pull` is
      sequential, which caps it at one. The cap becomes a real decision when a background
      scheduler exists

Also implemented: a clock-skew guard. A `last_synced` in the future — a config synced from a
machine ahead of this one — yields "not yet" rather than a fetch loop, which is the safe
direction and is pinned by test.

#### Technical Notes
```rust
pub enum SyncStrategy {
    Manual,
    OnLaunch,
    Scheduled(Schedule),
    Watch { source: String },  // e.g., "crates:serde"
}

pub enum Schedule {
    Daily,
    Weekly,
    Monthly,
}

pub struct SyncScheduler {
    sources: Vec<SourceConfig>,
    running: Arc<AtomicBool>,
}

impl SyncScheduler {
    pub async fn start(&self) {
        // The tick only decides what is DUE. It must never make a network call
        // itself -- the original design polled every watched package's registry on
        // a 60-second interval, which for 30 watched crates is ~43,000 requests a
        // day to crates.io for information that changes weekly. That is both abusive
        // and a direct contradiction of the non-functional requirement "no background
        // network activity without user action".
        let mut interval = tokio::time::interval(Duration::from_secs(15 * 60));

        loop {
            interval.tick().await;
            for source in self.sources.iter().filter(|s| self.is_due(s)) {
                self.enqueue(source).await;     // bounded worker pool does the work
            }
        }
    }

    fn is_due(&self, source: &SourceConfig) -> bool {
        if source.sync.pin_version {
            return false;
        }

        match &source.sync.strategy {
            SyncStrategy::Manual   => false,
            SyncStrategy::OnLaunch => false, // handled at startup
            SyncStrategy::Scheduled(schedule) => {
                let threshold = match schedule {
                    Schedule::Daily   => ChronoDuration::days(1),
                    Schedule::Weekly  => ChronoDuration::weeks(1),
                    Schedule::Monthly => ChronoDuration::days(30),
                };
                // `last_synced` is an Option<DateTime<Utc>>, not an Instant: it is
                // persisted across restarts. The original called `.elapsed()` on it,
                // which does not compile, and treated "never synced" as unhandled.
                match source.last_synced {
                    None => true,                                   // never synced => due
                    Some(t) => Utc::now() - t > threshold,
                }
            }
            // Registry checks are rate-limited per source (at most daily, jittered),
            // and are decided here, not performed here.
            SyncStrategy::Watch { .. } => self.registry_check_due(source),
        }
    }
}
```

**Package Registry Watching:**
```rust
async fn check_registry_update(package: &str) -> Result<bool, WatchError> {
    // `split_once(':').unwrap()` panicked on any malformed watch spec -- and the
    // spec comes from a user-edited YAML file.
    let (registry, name) = package
        .split_once(':')
        .ok_or_else(|| WatchError::MalformedSpec(package.to_string()))?;

    match registry {
        // Each of these must send a User-Agent identifying Tome with contact info
        // (crates.io rejects requests without one), use conditional requests, and
        // honour Retry-After. Results are cached for at least 24h.
        "crates" => check_crates_io(name).await,
        "npm"    => check_npm(name).await,
        "pypi"   => check_pypi(name).await,
        other    => Err(WatchError::UnknownRegistry(other.to_string())),
    }
}
```

#### Success Metrics
- All strategies working
- Schedule accuracy within 1 hour of the due time
- **Watch detects a new version within 24 hours of publish.** The original target of "< 1 hour"
  could only be met by polling registries aggressively, which is unfriendly and buys the user
  nothing — documentation for a release published an hour ago is rarely urgent.
- A malformed `watch_source` produces a config validation error, not a panic
- Registry request volume stays under 1 request per watched source per day

---

## Phase 4 Dependency Graph

```
P2-001 (Search Engine)
    │
    ├──── P4-001 (CLI Design) ──── P4-002 (CLI Scaffold)
    │                                    │
    │     ┌────────┬────────┬────────────┼────────┬────────┐
    │     │        │        │            │        │        │
    │     ▼        ▼        ▼            ▼        ▼        ▼
    │  P4-003   P4-004   P4-005      P4-006   P4-007   P4-018
    │  (add)   (pull)   (search)    (list)   (json)   (sync)
    │     │        │        │
    │     ▼        │        │
    │  P2-014     │        │
    │  (detect)   │        │
    │             │        │
    │             │        └──────────────────────────┐
    │             │                                   │
    │             └──── P1-008 (Scraper)              │
    │                                                 │
    └──── P4-008 (API Design) ──────────────────────────┐
              │                                        │
              ├──── P4-009 (Axum Server) ─────────────────┤
              │         │                              │
              │         ├──── P4-010 (API: search, sources)
              │         │                              │
              │         ├──── P4-011 (API: pages, bookmarks)
              │         │                              │
              │         └──── P4-012 (Auth)            │
              │                                        │
              └──── P4-013 (MCP Design) ─────────────────┤
                         │                              │
                         └──── P4-014 (MCP Handler)     │
                                   │                    │
                              ┌────┴────┐               │
                              ▼         ▼               │
                         P4-015     P4-016              │
                       (search,   (bookmark,            │
                        page,      symbol)              │
                        list)          │                │
                              │        │                │
                              └───┬────┘                │
                                  ▼                     │
                             P4-017 (Claude Code Plugin)◄┘
```

---

## Exit Criteria Checklist

- [ ] `tome` CLI binary works
- [ ] `tome add <url>` detects platform and adds source
- [ ] `tome pull` fetches documentation
- [ ] `tome search` returns relevant results
- [ ] `tome list` shows all sources
- [ ] All CLI commands support --json
- [ ] HTTP API server runs on `127.0.0.1:7431`, **off by default**, all routes under `/api/v1/`
- [ ] API endpoints work (search, sources, pages, bookmarks)
- [ ] **A request without a bearer token is rejected, including from loopback**
- [ ] **A cross-origin `fetch()` from a web page cannot read any API response** — verified by an
      actual browser test, not by reading the config
- [ ] `POST /api/v1/sources` rejects private, loopback, and link-local targets, including via redirect
- [ ] **MCP server runs over stdio and Claude Code connects to it** — this is the exit criterion
      that the original Unix-socket design could not have satisfied
- [ ] MCP tools callable (tome_search, tome_get_page, etc.); write tools disabled by default
- [ ] `tome mcp` emits nothing on stdout but JSON-RPC
- [ ] Claude Code can invoke `/tome` commands
- [ ] Sync strategies work (manual, scheduled, watch) within their rate budgets
- [ ] Every command, route, and tool that exists is present in its specification document
