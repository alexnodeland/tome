# Phase 1: Foundation (v0.1)

**Goal:** Core reading experience with manual doc addition
**Tickets:** 22
**Exit Criteria:** Can add a ReadTheDocs site via config and read it with consistent styling

---

## Ticket Summary

| ID | Title | Complexity | Priority | Dependencies |
|----|-------|------------|----------|--------------|
| P1-001 | Initialize Tauri + Rust project | M | Critical | None |
| P1-002 | Setup Svelte frontend scaffold | M | Critical | P1-001 |
| P1-003 | Configure Tauri-Svelte integration | M | Critical | P1-001, P1-002 |
| P1-004 | Design and implement SQLite schema | M | Critical | P1-001 |
| P1-005 | Create source configuration YAML parser | M | High | P1-001 |
| P1-006 | Implement file system layout manager | S | High | P1-001 |
| P1-007 | Build configuration file watcher | S | Medium | P1-005, P1-006 |
| P1-008 | Implement generic HTTP scraper core | L | Critical | P1-001 |
| P1-009 | Add configurable CSS selector extraction | M | High | P1-008 |
| P1-010 | Implement BFS crawl with depth limits | M | High | P1-008 |
| P1-011 | Build URL pattern filtering system | S | High | P1-008 |
| P1-012 | Create HTML-to-AST parser | L | Critical | P1-001 |
| P1-013 | Build AST normalization pipeline | L | High | P1-012 |
| P1-014 | Implement syntax highlighting engine | M | High | P1-012, P1-013 |
| P1-015 | Create typography system | M | Critical | P1-002 |
| P1-016 | Build WKWebView rendering bridge | L | Critical | P1-003, P1-013, P1-015 |
| P1-017 | Implement three-panel layout shell | M | High | P1-002 |
| P1-018 | Build source library sidebar component | M | High | P1-017, P1-004 |
| P1-019 | Implement TOC sidebar extraction and display | M | High | P1-017, P1-013 |
| P1-020 | Create navigation system (back/forward/history) | M | High | P1-016, P1-018 |
| P1-021 | Build page metadata storage and retrieval | M | High | P1-004, P1-013 |
| P1-022 | Implement manual source addition workflow | M | High | P1-005, P1-007, P1-008 |

---

## Detailed Tickets

### P1-001: Initialize Tauri + Rust project

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** None
**Blocks:** P1-002, P1-003, P1-004, P1-005, P1-006, P1-008, P1-012

#### Description
Set up the foundational Tauri + Rust project structure with proper configuration, build system, and development tooling.

#### Acceptance Criteria
- [ ] Tauri project initialized with latest stable version
- [ ] Rust workspace configured with proper module structure
- [ ] Development build works (`cargo tauri dev`)
- [ ] Production build works (`cargo tauri build`)
- [ ] Hot reload configured for development
- [ ] Linting (clippy) and formatting (rustfmt) configured
- [ ] `.gitignore` properly configured for Rust/Tauri artifacts
- [ ] CI/CD workflow file for basic build validation

#### Technical Notes
```
tome/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       └── commands/     # Tauri command handlers
├── src/                  # Svelte frontend (P1-002)
└── package.json
```

#### Success Metrics
- Build completes in < 60 seconds on standard dev machine
- No clippy warnings at default level
- Memory usage of empty app < 50MB

---

### P1-002: Setup Svelte frontend scaffold

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P1-001
**Blocks:** P1-003, P1-015, P1-017

#### Description
Initialize the Svelte frontend with proper project structure, build configuration, and development tooling.

#### Acceptance Criteria
- [ ] Svelte project initialized with TypeScript support
- [ ] Vite configured for bundling
- [ ] CSS/SCSS preprocessing configured
- [ ] Component structure established
- [ ] Development server works with hot reload
- [ ] Production build generates optimized bundle
- [ ] ESLint + Prettier configured
- [ ] Basic app shell component renders

#### Technical Notes
```
src/
├── lib/
│   ├── components/       # Reusable UI components
│   ├── stores/           # Svelte stores for state
│   ├── services/         # API/Tauri command wrappers
│   └── utils/            # Utility functions
├── routes/               # Page components
├── app.html
├── app.css
└── main.ts
```

#### Success Metrics
- Bundle size < 200KB (gzipped)
- First paint < 100ms in dev mode
- TypeScript strict mode enabled with no errors

---

### P1-003: Configure Tauri-Svelte integration

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P1-001, P1-002
**Blocks:** P1-016

#### Description
Wire up the Tauri backend with the Svelte frontend, establishing the IPC bridge for commands and events.

#### Acceptance Criteria
- [ ] Tauri commands callable from Svelte
- [ ] Event system working (Rust → JS and JS → Rust)
- [ ] TypeScript types generated for Tauri commands
- [ ] Window management commands functional
- [ ] File dialog integration working
- [ ] Error handling pattern established for IPC

#### Technical Notes
```typescript
// Example command invocation pattern
import { invoke } from '@tauri-apps/api/tauri';

interface Source {
  id: string;
  name: string;
  // ...
}

const sources = await invoke<Source[]>('list_sources');
```

```rust
// Corresponding Rust handler
#[tauri::command]
fn list_sources(state: State<AppState>) -> Result<Vec<Source>, String> {
    // ...
}
```

#### Success Metrics
- Command round-trip latency < 5ms
- Type safety enforced across IPC boundary
- No TypeScript `any` types in IPC layer

---

### P1-004: Design and implement SQLite schema

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P1-001
**Blocks:** P1-018, P1-021

#### Description
Design and implement the SQLite database schema for storing sources, pages, and metadata.

#### Acceptance Criteria
- [ ] Schema supports Source, Page, and SyncState entities
- [ ] Migrations system in place (sqlx or rusqlite migrations)
- [ ] Indexes created for common queries
- [ ] Database file created in proper location (~/.tome/tome.db)
- [ ] Connection pooling configured
- [ ] CRUD operations implemented for all entities
- [ ] Foreign key constraints enforced

#### Technical Notes
```sql
-- Core schema
CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL,
    url TEXT,
    local_path TEXT,
    version TEXT,
    category TEXT DEFAULT 'Uncategorized',
    icon TEXT,
    accent_color TEXT,
    sync_strategy TEXT DEFAULT 'manual',
    sync_schedule TEXT,
    pin_version INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    last_synced TEXT,
    page_count INTEGER DEFAULT 0,
    index_size_bytes INTEGER DEFAULT 0
);

CREATE TABLE pages (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    last_modified TEXT NOT NULL,
    UNIQUE(source_id, path)
);

CREATE INDEX idx_pages_source ON pages(source_id);
CREATE INDEX idx_sources_category ON sources(category);
```

#### Success Metrics
- Query for all sources completes < 10ms
- Query for pages by source completes < 20ms for 1000 pages
- Database file size < 10MB for 1000 pages

---

### P1-005: Create source configuration YAML parser

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-001
**Blocks:** P1-007, P1-022

#### Description
Implement YAML parsing for source configuration files with validation and type-safe deserialization.

#### Acceptance Criteria
- [ ] Parse all fields from source config schema (Appendix A)
- [ ] Validate required fields with helpful error messages
- [ ] Default values applied for optional fields
- [ ] Support for generic scraper configuration
- [ ] Support for man page configuration
- [ ] Validation of URL patterns (regex)
- [ ] Validation of CSS selectors

#### Technical Notes
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct SourceConfig {
    pub name: String,
    pub source: SourceType,
    pub version: Option<String>,
    pub category: Option<String>,
    pub display: Option<DisplayConfig>,
    pub sync: Option<SyncConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SourceType {
    ReadTheDocs { url: String },
    Rustdoc { url: String },
    MdBook { url: String },
    Man { paths: Vec<PathBuf>, sections: Option<Vec<u8>> },
    Generic { url: String, generic: GenericConfig },
    Local { path: PathBuf },
}
```

#### Success Metrics
- Parse valid config < 5ms
- Clear error messages for all validation failures
- 100% coverage of schema fields

---

### P1-006: Implement file system layout manager

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P1-001
**Blocks:** P1-007

#### Description
Create a module to manage the ~/.tome directory structure, ensuring proper creation, permissions, and organization.

#### Acceptance Criteria
- [ ] Create ~/.tome directory structure on first run
- [ ] Proper subdirectories: sources/, data/, index/
- [ ] Handle permissions errors gracefully
- [ ] Support for custom base directory via config/env
- [ ] Path resolution utilities for all standard locations
- [ ] Cleanup utilities for orphaned data

#### Technical Notes
```
~/.tome/
├── config.yaml              # Global configuration
├── sources/                 # Source configurations
│   ├── rust-std.yaml
│   └── ...
├── data/                    # Document content
│   ├── rust-std/
│   │   ├── pages/          # Normalized HTML
│   │   └── raw/            # Original content
│   └── ...
├── index/                   # Tantivy index (Phase 2)
└── tome.db                  # SQLite database
```

#### Success Metrics
- Directory creation < 100ms
- Graceful handling of permission denied
- No data loss on path resolution edge cases

---

### P1-007: Build configuration file watcher

**Priority:** Medium
**Complexity:** S (1-2 days)
**Dependencies:** P1-005, P1-006
**Blocks:** P1-022

#### Description
Implement file system watching for the sources/ directory to detect config changes and trigger reloads.

#### Acceptance Criteria
- [ ] Watch ~/.tome/sources/ for file changes
- [ ] Debounce rapid changes (300ms window)
- [ ] Emit events for: add, modify, delete
- [ ] Re-validate config on change
- [ ] Notify UI of source list changes
- [ ] Handle watch errors (permissions, missing directory)

#### Technical Notes
Use `notify` crate for cross-platform file watching:
```rust
use notify::{Watcher, RecursiveMode, watcher};

fn watch_sources() -> notify::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_millis(300))?;
    watcher.watch("~/.tome/sources", RecursiveMode::NonRecursive)?;
    // ...
}
```

#### Success Metrics
- Change detected within 500ms of file write
- No duplicate events for single change
- < 1% CPU usage during idle watching

---

### P1-008: Implement generic HTTP scraper core

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-001
**Blocks:** P1-009, P1-010, P1-011, P1-022

#### Description
Build the core HTTP scraping infrastructure that all platform-specific scrapers will use.

#### Acceptance Criteria
- [ ] Async HTTP client with configurable timeouts
- [ ] Respect robots.txt (optional, configurable)
- [ ] Rate limiting (configurable requests/second)
- [ ] Retry logic with exponential backoff
- [ ] Cookie/session handling
- [ ] User-agent configuration
- [ ] Proxy support
- [ ] Progress reporting via events
- [ ] Cancellation support
- [ ] Error classification (network, auth, rate limit, etc.)

#### Technical Notes
```rust
pub struct Scraper {
    client: reqwest::Client,
    rate_limiter: RateLimiter,
    config: ScraperConfig,
}

impl Scraper {
    pub async fn fetch(&self, url: &str) -> Result<Response, ScrapeError> {
        self.rate_limiter.acquire().await;
        // Retry logic, error handling...
    }

    pub async fn crawl(&self, config: CrawlConfig) -> Result<CrawlResult, ScrapeError> {
        // BFS crawl implementation
    }
}
```

#### Success Metrics
- Fetch single page < 2s (network dependent)
- Crawl 100 pages with rate limit = 5/s takes ~20s
- Memory usage < 100MB during crawl

---

### P1-009: Add configurable CSS selector extraction

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-008
**Blocks:** None

#### Description
Implement CSS selector-based content extraction from HTML pages.

#### Acceptance Criteria
- [ ] Parse CSS selectors from configuration
- [ ] Extract content matching selectors
- [ ] Support multiple selectors with fallback chain
- [ ] Extract page title via selector
- [ ] Extract navigation elements via selector
- [ ] Handle missing elements gracefully
- [ ] Performance: parse + extract < 50ms for typical page

#### Technical Notes
Use `scraper` crate:
```rust
use scraper::{Html, Selector};

fn extract_content(html: &str, selectors: &[String]) -> Option<String> {
    let document = Html::parse_document(html);
    for selector_str in selectors {
        let selector = Selector::parse(selector_str).ok()?;
        if let Some(element) = document.select(&selector).next() {
            return Some(element.inner_html());
        }
    }
    None
}
```

#### Success Metrics
- Extract content from 95%+ of common doc sites
- Performance < 50ms per page
- Zero panics on malformed HTML

---

### P1-010: Implement BFS crawl with depth limits

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-008
**Blocks:** None

#### Description
Implement breadth-first crawling with configurable depth limits and link extraction.

#### Acceptance Criteria
- [ ] BFS traversal from entry points
- [ ] Configurable max depth (default: 4)
- [ ] Track visited URLs to avoid duplicates
- [ ] Extract and normalize internal links
- [ ] Respect same-domain constraint
- [ ] Support multiple entry points
- [ ] Progress reporting (pages found, crawled, remaining)
- [ ] Memory-efficient for large sites

#### Technical Notes
```rust
pub struct CrawlConfig {
    pub entry_points: Vec<String>,
    pub max_depth: u32,
    pub include_patterns: Vec<Regex>,
    pub exclude_patterns: Vec<Regex>,
}

pub struct CrawlResult {
    pub pages: Vec<CrawledPage>,
    pub errors: Vec<CrawlError>,
    pub stats: CrawlStats,
}
```

#### Success Metrics
- Crawl 1000 pages in < 5 minutes (rate limited to 5/s)
- Memory usage < 200MB for 1000 page crawl
- 100% link coverage within depth limit

---

### P1-011: Build URL pattern filtering system

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P1-008
**Blocks:** None

#### Description
Implement include/exclude pattern matching for URL filtering during crawl.

#### Acceptance Criteria
- [ ] Support regex patterns for include/exclude
- [ ] Patterns matched against URL path
- [ ] Exclude takes precedence over include
- [ ] Validate patterns at config load time
- [ ] Helpful error messages for invalid regex
- [ ] Pre-compiled regex for performance

#### Technical Notes
```rust
pub struct UrlFilter {
    include: Vec<Regex>,
    exclude: Vec<Regex>,
}

impl UrlFilter {
    pub fn matches(&self, url: &str) -> bool {
        let path = extract_path(url);

        // Check excludes first
        for pattern in &self.exclude {
            if pattern.is_match(&path) {
                return false;
            }
        }

        // If no includes specified, include all
        if self.include.is_empty() {
            return true;
        }

        // Check includes
        self.include.iter().any(|p| p.is_match(&path))
    }
}
```

#### Success Metrics
- Pattern matching < 1μs per URL
- No regex compilation at match time
- Clear errors for invalid patterns

---

### P1-012: Create HTML-to-AST parser

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-001
**Blocks:** P1-013, P1-014

#### Description
Build a parser that converts raw HTML into a structured AST for normalization.

#### Acceptance Criteria
- [ ] Parse HTML into tree structure
- [ ] Handle malformed HTML gracefully (use html5ever)
- [ ] Preserve semantic structure (headings, lists, code blocks, tables)
- [ ] Extract text content
- [ ] Extract links with href resolution
- [ ] Handle embedded code blocks with language hints
- [ ] Support incremental/streaming parsing for large docs

#### Technical Notes
```rust
pub enum Node {
    Document(Vec<Node>),
    Heading { level: u8, id: Option<String>, children: Vec<Node> },
    Paragraph(Vec<Node>),
    CodeBlock { language: Option<String>, content: String },
    InlineCode(String),
    Link { href: String, children: Vec<Node> },
    List { ordered: bool, items: Vec<Vec<Node>> },
    Table { headers: Vec<Node>, rows: Vec<Vec<Node>> },
    Text(String),
    // ...
}

pub fn parse_html(html: &str) -> Result<Node, ParseError> {
    // Implementation using html5ever + custom tree sink
}
```

#### Success Metrics
- Parse 100KB HTML in < 100ms
- Zero panics on any input
- Preserve 100% of semantic content

---

### P1-013: Build AST normalization pipeline

**Priority:** High
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-012
**Blocks:** P1-016, P1-019, P1-021

#### Description
Implement the normalization pipeline that transforms platform-specific HTML into consistent output.

#### Acceptance Criteria
- [ ] Strip non-content elements (nav, footer, ads)
- [ ] Normalize heading levels (ensure h1 is title)
- [ ] Convert relative URLs to absolute
- [ ] Normalize code block language identifiers
- [ ] Clean up whitespace and formatting
- [ ] Extract and structure metadata (title, description)
- [ ] Generate consistent class names for styling
- [ ] Pipeline is configurable per-source

#### Technical Notes
```rust
pub struct NormalizationPipeline {
    steps: Vec<Box<dyn Transform>>,
}

pub trait Transform {
    fn transform(&self, node: Node) -> Node;
}

// Example transforms
struct StripNavigation;
struct NormalizeHeadings;
struct ResolveUrls { base_url: String };
struct NormalizeCodeBlocks;
```

#### Success Metrics
- Normalized output is consistent across platforms
- Processing time < 50ms per page
- No information loss for content elements

---

### P1-014: Implement syntax highlighting engine

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-012, P1-013
**Blocks:** None

#### Description
Add syntax highlighting for code blocks in the rendering pipeline.

#### Acceptance Criteria
- [ ] Support major languages (Rust, Python, JavaScript, TypeScript, Go, Java, C/C++, Shell)
- [ ] Use syntect or tree-sitter for highlighting
- [ ] Theme integration (light/dark modes)
- [ ] Muted, readable color palette
- [ ] Handle unknown languages gracefully (plain text)
- [ ] Line numbers optional

#### Technical Notes
```rust
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_light: Theme,
    theme_dark: Theme,
}

impl Highlighter {
    pub fn highlight(&self, code: &str, language: &str, dark_mode: bool) -> String {
        // Returns HTML with span classes for styling
    }
}
```

#### Success Metrics
- Highlight 1000 lines in < 100ms
- Support 20+ common languages
- Graceful fallback for unknown languages

---

### P1-015: Create typography system

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P1-002
**Blocks:** P1-016

#### Description
Implement the typography system for beautiful, readable documentation.

#### Acceptance Criteria
- [ ] New York serif for body text (with SF Pro fallback)
- [ ] SF Mono for code
- [ ] SF Pro Display for headings
- [ ] Proper sizing: 17px body, 15px code
- [ ] Line height 1.6 for body
- [ ] Maximum measure of 70ch
- [ ] Proper paragraph spacing (1em)
- [ ] Light and dark mode color schemes
- [ ] CSS variables for theming
- [ ] Print stylesheet

#### Technical Notes
```css
:root {
  /* Typography */
  --font-body: 'New York', 'Georgia', serif;
  --font-code: 'SF Mono', 'Menlo', monospace;
  --font-heading: 'SF Pro Display', -apple-system, sans-serif;

  --size-body: 17px;
  --size-code: 15px;
  --line-height: 1.6;
  --measure: 70ch;

  /* Light mode colors */
  --color-bg: #FAFAFA;
  --color-surface: #FFFFFF;
  --color-text: #1D1D1F;
  --color-text-secondary: #6E6E73;
  --color-accent: #5856D6;
  --color-border: #E5E5EA;
}

@media (prefers-color-scheme: dark) {
  :root {
    --color-bg: #1C1C1E;
    --color-surface: #2C2C2E;
    --color-text: #F5F5F7;
    --color-text-secondary: #98989D;
    --color-accent: #5E5CE6;
    --color-border: #38383A;
  }
}
```

#### Success Metrics
- Consistent typography across all rendered content
- Readable at all window widths
- System fonts = no FOUT

---

### P1-016: Build WKWebView rendering bridge

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-003, P1-013, P1-015
**Blocks:** P1-020

#### Description
Create the bridge between normalized content and WKWebView for rendering.

#### Acceptance Criteria
- [ ] Load normalized HTML into WKWebView
- [ ] Inject typography CSS
- [ ] Inject syntax highlighting CSS
- [ ] Handle internal link clicks (intercept and route)
- [ ] Handle external link clicks (open in browser)
- [ ] Expose scroll position to Svelte
- [ ] Support scroll-to-anchor
- [ ] Bidirectional communication (JS ↔ Swift/Rust)

#### Technical Notes
```rust
// Tauri command to render content
#[tauri::command]
async fn render_page(source_id: String, path: String) -> Result<RenderedPage, Error> {
    let content = load_normalized_content(&source_id, &path).await?;
    let html = wrap_with_styling(content);
    Ok(RenderedPage { html, title, toc })
}
```

```svelte
<script>
  import { onMount } from 'svelte';

  let webview: HTMLElement;

  async function loadPage(sourceId: string, path: string) {
    const { html, title, toc } = await invoke('render_page', { sourceId, path });
    // Inject into webview
  }
</script>

<div bind:this={webview} class="reader-content">
  <!-- WKWebView content -->
</div>
```

#### Success Metrics
- Page render < 100ms after content loaded
- Smooth scrolling (60fps)
- No layout shift during load

---

### P1-017: Implement three-panel layout shell

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-002
**Blocks:** P1-018, P1-019

#### Description
Build the main application layout with collapsible sidebars.

#### Acceptance Criteria
- [ ] Left sidebar (sources list) - 240px default
- [ ] Center pane (reader) - flexible
- [ ] Right sidebar (page TOC) - 200px default
- [ ] Sidebars collapsible with animation
- [ ] Keyboard shortcuts: Cmd+1 (left), Cmd+2 (right)
- [ ] Minimum window size: 800x600
- [ ] Responsive: sidebars auto-collapse at narrow widths
- [ ] Sidebar widths persisted
- [ ] Drag-to-resize sidebars

#### Technical Notes
```svelte
<script>
  let leftOpen = true;
  let rightOpen = true;
  let leftWidth = 240;
  let rightWidth = 200;
</script>

<div class="app-layout">
  {#if leftOpen}
    <aside class="sidebar-left" style="width: {leftWidth}px">
      <slot name="left" />
    </aside>
  {/if}

  <main class="reader-pane">
    <slot name="main" />
  </main>

  {#if rightOpen}
    <aside class="sidebar-right" style="width: {rightWidth}px">
      <slot name="right" />
    </aside>
  {/if}
</div>
```

#### Success Metrics
- Collapse animation < 200ms
- No layout jank during resize
- State persisted across sessions

---

### P1-018: Build source library sidebar component

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-017, P1-004
**Blocks:** P1-020

#### Description
Create the left sidebar component showing all documentation sources.

#### Acceptance Criteria
- [ ] List all sources from database
- [ ] Group by category (collapsible)
- [ ] Show source icon, name, and status
- [ ] Show last synced time
- [ ] Click to select and show source in reader
- [ ] Right-click context menu (sync, remove, configure)
- [ ] Search/filter sources
- [ ] Keyboard navigation (up/down arrows)
- [ ] Selected source highlighted

#### Technical Notes
```svelte
<script>
  import { sources } from '$lib/stores/sources';

  let selectedId: string | null = null;
  let filter = '';

  $: grouped = groupByCategory($sources.filter(s =>
    s.name.toLowerCase().includes(filter.toLowerCase())
  ));
</script>

<div class="source-library">
  <input type="search" bind:value={filter} placeholder="Filter sources..." />

  {#each Object.entries(grouped) as [category, items]}
    <div class="category">
      <h3>{category}</h3>
      {#each items as source}
        <SourceItem
          {source}
          selected={source.id === selectedId}
          on:click={() => selectSource(source.id)}
        />
      {/each}
    </div>
  {/each}
</div>
```

#### Success Metrics
- Render 100+ sources in < 50ms
- Filter response < 16ms
- Smooth keyboard navigation

---

### P1-019: Implement TOC sidebar extraction and display

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-017, P1-013
**Blocks:** None

#### Description
Extract table of contents from normalized content and display in right sidebar.

#### Acceptance Criteria
- [ ] Extract headings (h1-h4) from content
- [ ] Generate anchor links
- [ ] Display as nested tree structure
- [ ] Click to scroll to section
- [ ] Highlight current section during scroll
- [ ] Smooth scroll animation
- [ ] Collapse deeply nested items

#### Technical Notes
```rust
pub struct TocEntry {
    pub id: String,
    pub title: String,
    pub level: u8,
    pub children: Vec<TocEntry>,
}

pub fn extract_toc(ast: &Node) -> Vec<TocEntry> {
    // Walk AST, collect headings, build tree
}
```

```svelte
<script>
  export let toc: TocEntry[];
  export let currentSection: string;

  function scrollTo(id: string) {
    // Smooth scroll to anchor
  }
</script>

<nav class="page-toc">
  {#each toc as entry}
    <TocItem
      {entry}
      active={entry.id === currentSection}
      on:click={() => scrollTo(entry.id)}
    />
  {/each}
</nav>
```

#### Success Metrics
- TOC extracted in < 10ms
- Scroll tracking at 60fps
- Works with 50+ headings

---

### P1-020: Create navigation system (back/forward/history)

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-016, P1-018
**Blocks:** None

#### Description
Implement browser-like navigation with history stack.

#### Acceptance Criteria
- [ ] Back navigation (Cmd+[)
- [ ] Forward navigation (Cmd+])
- [ ] History stack maintained per session
- [ ] Visual back/forward buttons in toolbar
- [ ] Buttons disabled when at stack ends
- [ ] Internal links add to history
- [ ] Cross-source navigation supported
- [ ] Scroll position restored on back/forward

#### Technical Notes
```typescript
interface HistoryEntry {
  sourceId: string;
  path: string;
  scrollPosition: number;
  timestamp: number;
}

class NavigationHistory {
  private stack: HistoryEntry[] = [];
  private current: number = -1;

  push(entry: HistoryEntry): void { ... }
  back(): HistoryEntry | null { ... }
  forward(): HistoryEntry | null { ... }
  canGoBack(): boolean { ... }
  canGoForward(): boolean { ... }
}
```

#### Success Metrics
- Navigation latency < 50ms
- Scroll position accurate to within 10px
- History survives 1000+ entries

---

### P1-021: Build page metadata storage and retrieval

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-004, P1-013
**Blocks:** None

#### Description
Store and retrieve page metadata for efficient browsing without re-parsing.

#### Acceptance Criteria
- [ ] Store page metadata after normalization
- [ ] Efficient retrieval by source+path
- [ ] Track content hash for change detection
- [ ] Store modification timestamps
- [ ] Batch insertion for crawl results
- [ ] Update only changed pages on re-sync
- [ ] Delete orphaned pages on re-sync

#### Technical Notes
```rust
impl PageRepository {
    pub async fn upsert_pages(&self, pages: Vec<PageMetadata>) -> Result<UpsertStats> {
        // Batch insert with conflict handling
    }

    pub async fn get_page(&self, source_id: &str, path: &str) -> Result<Option<Page>> {
        // Efficient lookup
    }

    pub async fn list_pages(&self, source_id: &str) -> Result<Vec<PageSummary>> {
        // List all pages for a source
    }

    pub async fn cleanup_orphaned(&self, source_id: &str, current_paths: &[String]) -> Result<u32> {
        // Remove pages no longer in source
    }
}
```

#### Success Metrics
- Batch insert 1000 pages in < 500ms
- Single page lookup < 5ms
- Cleanup accurate (no false deletions)

---

### P1-022: Implement manual source addition workflow

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-005, P1-007, P1-008
**Blocks:** None

#### Description
Enable users to add documentation sources by creating config files.

#### Acceptance Criteria
- [ ] Detect new YAML files in ~/.tome/sources/
- [ ] Validate configuration on detection
- [ ] Show validation errors in UI
- [ ] Trigger initial sync on valid config
- [ ] Show sync progress in UI
- [ ] Add source to library on completion
- [ ] Handle sync failures gracefully
- [ ] Example configs provided in docs

#### Technical Notes
```yaml
# Example: ~/.tome/sources/polars.yaml
name: Polars
source:
  type: generic
  url: https://docs.pola.rs/
  generic:
    entry_points: ["/"]
    max_depth: 4
    content_selector: "main.content"
    title_selector: "h1"
category: Python
sync:
  strategy: weekly
```

User workflow:
1. Create YAML file in ~/.tome/sources/
2. Tome detects new file
3. Validates configuration
4. If valid, starts initial sync
5. Shows progress (pages fetched, total estimated)
6. On completion, source appears in library

#### Success Metrics
- Detection within 1 second of file save
- Clear error messages for invalid configs
- Initial sync progress visible within 2 seconds

---

## Phase 1 Dependency Graph

```
P1-001 (Tauri Init)
  ├── P1-002 (Svelte) ──┬── P1-003 (Integration)
  │                     │        │
  │                     │        └── P1-016 (WebView) ◄──┐
  │                     │                               │
  │                     ├── P1-015 (Typography) ────────┘
  │                     │
  │                     └── P1-017 (Layout) ──┬── P1-018 (Library) ── P1-020
  │                                           │        │
  │                                           │        └── P1-004
  │                                           │
  │                                           └── P1-019 (TOC) ◄── P1-013
  │
  ├── P1-004 (SQLite) ──────────────────────────────── P1-021
  │
  ├── P1-005 (YAML) ──┬── P1-007 (Watcher) ── P1-022
  │                   │                          │
  ├── P1-006 (FS) ────┘                          │
  │                                              │
  ├── P1-008 (Scraper) ──┬── P1-009 (Selectors)  │
  │      │               ├── P1-010 (BFS)        │
  │      │               ├── P1-011 (URL Filter) │
  │      │               └───────────────────────┘
  │      │
  └── P1-012 (Parser) ──┬── P1-013 (Normalize) ── P1-016
                        │
                        └── P1-014 (Syntax HL)
```

---

## Exit Criteria Checklist

- [ ] Tauri + Svelte + Rust project builds and runs
- [ ] Three-panel layout functional with collapsible sidebars
- [ ] Can create YAML config for a ReadTheDocs site
- [ ] Config is detected and validated automatically
- [ ] Generic scraper fetches and crawls the site
- [ ] Content is normalized and styled consistently
- [ ] Typography matches design spec
- [ ] Can navigate between pages
- [ ] Back/forward navigation works
- [ ] TOC sidebar shows page structure
- [ ] Source appears in library sidebar
