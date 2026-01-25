# Phase 4: Automation & Integration (v0.4)

**Goal:** Programmable access and developer tool integration
**Tickets:** 18
**Prerequisites:** Phase 2 complete (can run parallel with Phase 3)
**Exit Criteria:** Claude Code can add and search docs; MCP tools work with AI agents

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
  pull [source]      Fetch/update documentation (--all for all sources)
  search <query>     Search documentation (--scope, --limit)
  list               List all sources (--category, --json)
  remove <source>    Remove a source (--confirm)
  config [source]    View/edit configuration
  serve              Start local API server (--port)
  mcp                Start MCP server (--socket, --port)
  status             Show sync and index status
  export             Export bookmarks/annotations (--format)

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
- [ ] Accept URL or local path
- [ ] Auto-detect platform type
- [ ] Interactive confirmation (unless --yes)
- [ ] Create config file in ~/.tome/sources/
- [ ] Trigger initial pull after adding
- [ ] Show progress during pull
- [ ] Handle duplicate detection

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

Created: ~/.tome/sources/python-3.yaml
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
- [ ] List all sources with metadata
- [ ] Filter by category
- [ ] Show sync status
- [ ] JSON output for scripting
- [ ] Remove by name
- [ ] Confirmation before remove
- [ ] Clean up data on remove

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
- [ ] --json flag on all relevant commands
- [ ] Consistent JSON structure
- [ ] Errors as JSON (with --json)
- [ ] Streaming JSON for large outputs
- [ ] Exit codes preserved with JSON
- [ ] Documented JSON schemas

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
- [ ] Axum server setup
- [ ] Configurable port (default 7431)
- [ ] Localhost binding by default
- [ ] Graceful shutdown
- [ ] Request logging
- [ ] Error handling middleware
- [ ] CORS middleware
- [ ] Health check endpoint

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

        // Middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(Extension(app_state));

    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    tracing::info!("Starting server on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
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
- [ ] GET /api/search - search with filters
- [ ] GET /api/sources - list all sources
- [ ] POST /api/sources - add source (YAML/JSON body)
- [ ] GET /api/sources/:id - get source details
- [ ] DELETE /api/sources/:id - remove source
- [ ] POST /api/sources/:id/sync - trigger sync
- [ ] Proper status codes
- [ ] Validation errors with details

#### Technical Notes
```rust
pub async fn search(
    Query(params): Query<SearchParams>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<SearchResponse>, ApiError> {
    let results = state.search_engine
        .search(&params.q, params.scope.as_deref(), params.limit.unwrap_or(10))
        .await?;

    Ok(Json(SearchResponse {
        results,
        total: results.len(),
        query_time_ms: 0, // TODO: measure
    }))
}

pub async fn add_source(
    Extension(state): Extension<Arc<AppState>>,
    body: String, // Accept YAML or JSON
) -> Result<Json<Source>, ApiError> {
    let config: SourceConfig = if body.trim().starts_with('{') {
        serde_json::from_str(&body)?
    } else {
        serde_yaml::from_str(&body)?
    };

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
- [ ] GET /api/sources/:id/pages - list pages
- [ ] GET /api/sources/:id/pages/:path - get page content
- [ ] GET /api/bookmarks - list bookmarks
- [ ] POST /api/bookmarks - create bookmark
- [ ] DELETE /api/bookmarks/:id - delete bookmark
- [ ] Page content as rendered HTML
- [ ] Pagination for large page lists

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

**Priority:** Medium
**Complexity:** S (1-2 days)
**Dependencies:** P4-009
**Blocks:** None

#### Description
Add optional token-based authentication for API access.

#### Acceptance Criteria
- [ ] Localhost access always allowed
- [ ] Token required for non-localhost (when enabled)
- [ ] Token in Authorization header (Bearer)
- [ ] Token generated and stored securely
- [ ] Token rotation command
- [ ] Clear error for missing/invalid token

#### Technical Notes
```rust
pub async fn auth_middleware(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(state): Extension<Arc<AppState>>,
    request: Request<Body>,
    next: Next<Body>,
) -> Result<Response, ApiError> {
    // Always allow localhost
    if addr.ip().is_loopback() {
        return Ok(next.run(request).await);
    }

    // Check if auth is enabled
    if !state.config.require_auth {
        return Ok(next.run(request).await);
    }

    // Validate token
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match token {
        Some(t) if state.validate_token(t) => {
            Ok(next.run(request).await)
        }
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
- [ ] MCP protocol specification review
- [ ] Tool definitions (functions AI can call)
- [ ] Transport options (Unix socket, TCP)
- [ ] State management approach
- [ ] Error handling for AI clients
- [ ] Resource limits and timeouts

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

**Transport:**
```
Default: Unix socket at ~/.tome/mcp.sock
Optional: TCP on configurable port

Configuration:
mcp:
  enabled: true
  socket: ~/.tome/mcp.sock
  # or
  port: 7432
```

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
- [ ] JSON-RPC 2.0 protocol handling
- [ ] Tool registration with MCP
- [ ] Request/response handling
- [ ] Error responses per spec
- [ ] Unix socket listener
- [ ] TCP listener (optional)
- [ ] Concurrent request handling
- [ ] Graceful shutdown

#### Technical Notes
```rust
use tokio::net::UnixListener;

pub struct McpServer {
    tools: HashMap<String, Box<dyn McpTool>>,
    state: Arc<AppState>,
}

impl McpServer {
    pub async fn run(self, socket_path: &Path) -> Result<()> {
        // Remove existing socket
        let _ = fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path)?;
        tracing::info!("MCP server listening on {:?}", socket_path);

        loop {
            let (stream, _) = listener.accept().await?;
            let server = self.clone();
            tokio::spawn(async move {
                if let Err(e) = server.handle_connection(stream).await {
                    tracing::error!("MCP connection error: {}", e);
                }
            });
        }
    }

    async fn handle_connection(&self, stream: UnixStream) -> Result<()> {
        let (reader, writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut writer = BufWriter::new(writer);

        loop {
            let request: JsonRpcRequest = read_message(&mut reader).await?;

            let response = match request.method.as_str() {
                "initialize" => self.handle_initialize(&request).await,
                "tools/list" => self.handle_list_tools(&request).await,
                "tools/call" => self.handle_call_tool(&request).await,
                _ => JsonRpcResponse::error(request.id, -32601, "Method not found"),
            };

            write_message(&mut writer, &response).await?;
        }
    }
}
```

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
- [ ] tome_search tool working
- [ ] tome_get_page tool working
- [ ] tome_list_sources tool working
- [ ] tome_get_toc tool working
- [ ] Proper error handling
- [ ] Reasonable timeouts
- [ ] Documentation for each tool

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
- [ ] tome_bookmark tool working
- [ ] tome_lookup_symbol tool working
- [ ] Bookmark creates entry in database
- [ ] Symbol lookup uses search index
- [ ] Language filtering works
- [ ] Error handling for not found

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
- [ ] Plugin manifest specification
- [ ] Slash commands defined (/tome add, /tome search, etc.)
- [ ] MCP tool integration documented
- [ ] Installation instructions
- [ ] Example workflows documented
- [ ] Error handling guidelines

#### Technical Notes
```yaml
# Claude Code Plugin Manifest
name: tome
version: 1.0.0
description: Manage documentation with Tome

slash_commands:
  - name: add
    description: Add documentation source to Tome
    usage: /tome add <url>
    handler: tome_add

  - name: search
    description: Search Tome documentation
    usage: /tome search <query>
    handler: tome_search

  - name: list
    description: List available documentation sources
    usage: /tome list
    handler: tome_list

  - name: sync
    description: Sync documentation sources
    usage: /tome sync [source]
    handler: tome_sync

  - name: remove
    description: Remove documentation source
    usage: /tome remove <source>
    handler: tome_remove

mcp:
  server: ~/.tome/mcp.sock
  tools:
    - tome_search
    - tome_get_page
    - tome_list_sources
    - tome_get_toc
    - tome_bookmark
    - tome_lookup_symbol
```

**Example Workflow:**
```
User: /tome add https://docs.pola.rs/

Claude Code:
  → Invokes: tome add https://docs.pola.rs/ --json
  → Parses response
  → "I've added Polars documentation to Tome. It detected MkDocs
     with Material theme and will sync weekly.

     Found 847 pages across 12 sections. Would you like me to
     search for something specific?"

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

#### Acceptance Criteria
- [ ] Manual sync (only when triggered)
- [ ] On-launch sync (check at app start)
- [ ] Scheduled sync (daily, weekly, monthly)
- [ ] Watch sync (monitor package registry)
- [ ] Version pinning (ignore updates)
- [ ] Background sync execution
- [ ] Sync state persistence

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
        let mut interval = tokio::time::interval(Duration::from_secs(60));

        loop {
            interval.tick().await;

            for source in &self.sources {
                if self.should_sync(&source) {
                    if let Err(e) = self.sync_source(&source).await {
                        tracing::error!("Sync failed for {}: {}", source.name, e);
                    }
                }
            }
        }
    }

    fn should_sync(&self, source: &SourceConfig) -> bool {
        if source.sync.pin_version {
            return false;
        }

        match &source.sync.strategy {
            SyncStrategy::Manual => false,
            SyncStrategy::OnLaunch => false, // Handled separately
            SyncStrategy::Scheduled(schedule) => {
                let last_sync = source.last_synced;
                let threshold = match schedule {
                    Schedule::Daily => Duration::from_secs(24 * 60 * 60),
                    Schedule::Weekly => Duration::from_secs(7 * 24 * 60 * 60),
                    Schedule::Monthly => Duration::from_secs(30 * 24 * 60 * 60),
                };
                last_sync.elapsed() > threshold
            }
            SyncStrategy::Watch { source: pkg } => {
                // Check package registry for new version
                self.check_registry_update(pkg)
            }
        }
    }
}
```

**Package Registry Watching:**
```rust
async fn check_registry_update(package: &str) -> bool {
    let (registry, name) = package.split_once(':').unwrap();
    match registry {
        "crates" => check_crates_io(name).await,
        "npm" => check_npm(name).await,
        "pypi" => check_pypi(name).await,
        _ => false,
    }
}
```

#### Success Metrics
- All strategies working
- Schedule accuracy within 1 hour
- Watch detects updates < 1 hour after publish

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
- [ ] HTTP API server runs on localhost:7431
- [ ] API endpoints work (search, sources, pages, bookmarks)
- [ ] MCP server runs on Unix socket
- [ ] MCP tools callable (tome_search, tome_get_page, etc.)
- [ ] Claude Code can invoke `/tome` commands
- [ ] Sync strategies work (manual, scheduled, watch)
- [ ] Documentation complete for all integrations
