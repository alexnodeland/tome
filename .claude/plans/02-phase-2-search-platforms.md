# Phase 2: Search & Platforms (v0.2)

**Goal:** Intelligent search and platform-specific scrapers
**Tickets:** 18
**Prerequisites:** Phase 1 complete
**Exit Criteria:** Fast search across multiple doc sources, platform scrapers work reliably

---

## Ticket Summary

| ID | Title | Complexity | Priority | Dependencies |
|----|-------|------------|----------|--------------|
| P2-001 | Integrate Tantivy search engine | L | Critical | P1-004, P1-021 |
| P2-002 | Design search index schema | M | Critical | P2-001 |
| P2-003 | Build incremental indexing pipeline | L | High | P2-001, P2-002 |
| P2-004 | Implement global search UI (Cmd+K) | M | Critical | P2-001 |
| P2-005 | Create search results component | M | Critical | P2-004 |
| P2-006 | Add search result ranking and scoring | M | High | P2-001, P2-002 |
| P2-007 | Implement in-page search (Cmd+F) | M | High | P1-016 |
| P2-008 | Build search scoping system | S | High | P2-004, P2-005 |
| P2-009 | Add fuzzy matching and typo tolerance | M | Medium | P2-001 |
| P2-010 | Implement ReadTheDocs scraper | L | Critical | P1-008 |
| P2-011 | Build rustdoc scraper | L | High | P1-008 |
| P2-012 | Create mdBook scraper | M | High | P1-008 |
| P2-013 | Implement man page integration | L | High | P1-008 |
| P2-014 | Build platform auto-detection | M | High | P2-010, P2-011, P2-012 |
| P2-015 | Create symbol-aware search | M | Medium | P2-002, P2-006 |
| P2-016 | Implement search history | S | Medium | P2-004 |
| P2-017 | Add search keyboard navigation | S | High | P2-004, P2-005 |
| P2-018 | Build search performance benchmarks | S | Medium | P2-001 |

---

## Detailed Tickets

### P2-001: Integrate Tantivy search engine

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-004 (SQLite), P1-021 (Page metadata)
**Blocks:** P2-002, P2-003, P2-004, P2-006, P2-009

#### Description
Add Tantivy as the full-text search engine for documentation.

#### Acceptance Criteria
- [ ] Tantivy dependency added and compiling
- [ ] Index directory created at ~/.tome/index/
- [ ] Index writer configured with appropriate settings
- [ ] Index reader with reload capability
- [ ] Basic document indexing working
- [ ] Basic query execution working
- [ ] Index persistence across restarts
- [ ] Concurrent read/write support

#### Technical Notes
```rust
use tantivy::{Index, IndexWriter, IndexReader};
use tantivy::schema::{Schema, TEXT, STORED, STRING};

pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    writer_mutex: Mutex<IndexWriter>,
}

impl SearchEngine {
    pub fn new(index_path: &Path) -> Result<Self> {
        let schema = Self::build_schema();
        let index = Index::create_in_dir(index_path, schema.clone())?;
        // ...
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        // ...
    }

    pub fn index_page(&self, page: &IndexedPage) -> Result<()> {
        // ...
    }
}
```

#### Success Metrics
- Index 1000 pages in < 30 seconds
- Search latency < 50ms for simple queries
- Memory usage < 100MB for 10,000 pages

---

### P2-002: Design search index schema

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P2-001
**Blocks:** P2-003, P2-006, P2-015

#### Description
Define the Tantivy schema optimized for documentation search.

#### Acceptance Criteria
- [ ] Fields: title, headers, body, code, source_id, path
- [ ] Appropriate tokenizers per field
- [ ] Field boost configuration
- [ ] Multi-value fields for headers
- [ ] Stored vs indexed field decisions
- [ ] Code-specific tokenizer (camelCase, snake_case aware)
- [ ] Faceting support for source/category filtering

#### Technical Notes
```rust
fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // Stored and searchable
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("path", STRING | STORED);
    schema_builder.add_text_field("source_id", STRING | STORED);

    // Searchable with different weights
    schema_builder.add_text_field("headers", TEXT);  // Boost: 2x
    schema_builder.add_text_field("body", TEXT);     // Boost: 1x

    // Code with special tokenizer
    let code_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("code")
        );
    schema_builder.add_text_field("code", code_options);

    // Facets
    schema_builder.add_facet_field("category", INDEXED);

    schema_builder.build()
}
```

#### Success Metrics
- Schema supports all search use cases
- Relevant results for code queries
- Facet filtering < 10ms overhead

---

### P2-003: Build incremental indexing pipeline

**Priority:** High
**Complexity:** L (1-2 weeks)
**Dependencies:** P2-001, P2-002
**Blocks:** None

#### Description
Create a pipeline that efficiently indexes new and changed content.

#### Acceptance Criteria
- [ ] Index only changed pages (use content_hash)
- [ ] Remove deleted pages from index
- [ ] Batch indexing for efficiency
- [ ] Progress reporting during index builds
- [ ] Commit strategy (batch size, time-based)
- [ ] Index optimization/merge scheduling
- [ ] Handle index corruption gracefully

#### Technical Notes
```rust
pub struct IndexingPipeline {
    search_engine: Arc<SearchEngine>,
    page_repo: PageRepository,
}

impl IndexingPipeline {
    pub async fn sync_source(&self, source_id: &str) -> Result<IndexStats> {
        let pages = self.page_repo.list_pages(source_id).await?;
        let indexed = self.search_engine.get_indexed_hashes(source_id)?;

        let mut stats = IndexStats::default();

        for page in pages {
            let hash_key = format!("{}:{}", page.path, page.content_hash);
            if !indexed.contains(&hash_key) {
                let content = self.load_normalized_content(&page).await?;
                self.search_engine.index_page(&page, &content)?;
                stats.added += 1;
            }
        }

        // Remove deleted pages
        stats.removed = self.search_engine.cleanup_missing(source_id, &pages)?;

        self.search_engine.commit()?;
        Ok(stats)
    }
}
```

#### Success Metrics
- Incremental index update < 5s for 100 changed pages
- No duplicate documents in index
- Zero data loss during indexing

---

### P2-004: Implement global search UI (Cmd+K)

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P2-001
**Blocks:** P2-005, P2-008, P2-016, P2-017

#### Description
Create the global search modal interface.

#### Acceptance Criteria
- [ ] Opens with Cmd+K anywhere in app
- [ ] Centered modal with backdrop
- [ ] Large search input field
- [ ] Live results as you type (debounced 150ms)
- [ ] Loading indicator during search
- [ ] Close with Escape or click outside
- [ ] Focus returns to previous element on close
- [ ] Scope indicator showing search context

#### Technical Notes
```svelte
<script>
  import { searchStore } from '$lib/stores/search';
  import { onMount } from 'svelte';

  let visible = false;
  let query = '';
  let results = [];
  let loading = false;

  onMount(() => {
    const handleKeydown = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === 'k') {
        e.preventDefault();
        visible = true;
      }
    };
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });

  $: if (query) {
    debounceSearch(query);
  }
</script>

{#if visible}
  <div class="search-modal" on:click|self={() => visible = false}>
    <div class="search-container">
      <input
        type="search"
        bind:value={query}
        placeholder="Search documentation..."
        autofocus
      />
      <SearchResults {results} {loading} />
    </div>
  </div>
{/if}
```

#### Success Metrics
- Modal opens in < 50ms
- First results appear < 200ms after typing stops
- Smooth animation (no jank)

---

### P2-005: Create search results component

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P2-004
**Blocks:** P2-008, P2-017

#### Description
Build the component that displays search results.

#### Acceptance Criteria
- [ ] Show source name, page title, and snippet
- [ ] Highlight matched terms in snippet
- [ ] Show result count and search time
- [ ] Click result to navigate
- [ ] Score/relevance indicator (subtle)
- [ ] Group by source option
- [ ] No results state with suggestions
- [ ] Error state handling

#### Technical Notes
```svelte
<script>
  export let results: SearchResult[];
  export let loading: boolean;
  export let query: string;
  export let selectedIndex: number = 0;
</script>

<div class="search-results">
  {#if loading}
    <div class="loading">Searching...</div>
  {:else if results.length === 0}
    <div class="no-results">
      No results for "{query}"
      <p>Try different keywords or check spelling</p>
    </div>
  {:else}
    <div class="result-count">
      {results.length} results ({searchTime}ms)
    </div>
    {#each results as result, i}
      <div
        class="result-item"
        class:selected={i === selectedIndex}
        on:click={() => navigate(result)}
      >
        <div class="result-source">{result.source_name}</div>
        <div class="result-title">{@html highlightMatches(result.title)}</div>
        <div class="result-snippet">{@html highlightMatches(result.snippet)}</div>
      </div>
    {/each}
  {/if}
</div>
```

#### Success Metrics
- Render 50 results in < 16ms
- Snippet generation < 5ms per result
- Highlight accuracy 100%

---

### P2-006: Add search result ranking and scoring

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P2-001, P2-002
**Blocks:** None

#### Description
Implement intelligent ranking for search results.

#### Acceptance Criteria
- [ ] Title matches ranked highest
- [ ] Header matches ranked second
- [ ] Exact phrase matches boosted
- [ ] Recent pages slightly boosted (freshness)
- [ ] Source popularity factor (configurable)
- [ ] Code block matches weighted appropriately
- [ ] Configurable boost factors

#### Technical Notes
```rust
pub struct RankingConfig {
    pub title_boost: f32,      // default: 3.0
    pub header_boost: f32,     // default: 2.0
    pub body_boost: f32,       // default: 1.0
    pub code_boost: f32,       // default: 1.5
    pub freshness_factor: f32, // default: 0.1
}

impl SearchEngine {
    pub fn search_ranked(
        &self,
        query: &str,
        config: &RankingConfig,
    ) -> Result<Vec<ScoredResult>> {
        // Build multi-field query with boosts
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                (self.schema.get_field("title").unwrap(), config.title_boost),
                (self.schema.get_field("headers").unwrap(), config.header_boost),
                (self.schema.get_field("body").unwrap(), config.body_boost),
                (self.schema.get_field("code").unwrap(), config.code_boost),
            ],
        );
        // ...
    }
}
```

#### Success Metrics
- Relevant result in top 3 for 90%+ of queries
- Ranking consistent and explainable
- Configuration changes apply immediately

---

### P2-007: Implement in-page search (Cmd+F)

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-016 (WebView)
**Blocks:** None

#### Description
Add search within the current page view.

#### Acceptance Criteria
- [ ] Opens with Cmd+F
- [ ] Search bar appears at top of reader pane
- [ ] Live highlighting as you type
- [ ] Navigate matches with Enter/Shift+Enter
- [ ] Show match count (X of Y)
- [ ] Current match highlighted differently
- [ ] Close with Escape
- [ ] Works with WKWebView content

#### Technical Notes
```typescript
// Bridge to WebView for highlighting
function highlightMatches(query: string): number {
  // Use window.find() or custom highlight implementation
  // Return match count
}

function nextMatch(): void {
  // Navigate to next match
}

function previousMatch(): void {
  // Navigate to previous match
}

function clearHighlights(): void {
  // Remove all highlights
}
```

```svelte
<script>
  let visible = false;
  let query = '';
  let currentMatch = 0;
  let totalMatches = 0;

  $: if (query) {
    totalMatches = highlightMatches(query);
    currentMatch = totalMatches > 0 ? 1 : 0;
  }
</script>

{#if visible}
  <div class="in-page-search">
    <input bind:value={query} placeholder="Find in page..." />
    <span>{currentMatch} of {totalMatches}</span>
    <button on:click={previousMatch}>↑</button>
    <button on:click={nextMatch}>↓</button>
    <button on:click={() => visible = false}>×</button>
  </div>
{/if}
```

#### Success Metrics
- Highlight all matches < 100ms for typical page
- Navigation instant (< 16ms)
- Works with 1000+ matches

---

### P2-008: Build search scoping system

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P2-004, P2-005
**Blocks:** None

#### Description
Allow limiting search to specific sources or categories.

#### Acceptance Criteria
- [ ] Scope to single source
- [ ] Scope to category
- [ ] Scope indicator in search UI
- [ ] Clear scope with 'x' button
- [ ] Keyboard shortcut to change scope
- [ ] Remember last used scope
- [ ] Scope applies to both global and local search

#### Technical Notes
```typescript
type SearchScope =
  | { type: 'all' }
  | { type: 'source'; sourceId: string }
  | { type: 'category'; category: string };

interface SearchState {
  query: string;
  scope: SearchScope;
  results: SearchResult[];
}
```

#### Success Metrics
- Scope switch < 50ms
- Scoped search faster than global
- UI clearly shows current scope

---

### P2-009: Add fuzzy matching and typo tolerance

**Priority:** Medium
**Complexity:** M (3-5 days)
**Dependencies:** P2-001
**Blocks:** None

#### Description
Enable fuzzy matching to handle typos and approximate queries.

#### Acceptance Criteria
- [ ] Edit distance of 1-2 for short words
- [ ] Edit distance of 2-3 for longer words
- [ ] Prefix matching for partial words
- [ ] Fuzzy results ranked lower than exact
- [ ] Configurable fuzzy threshold
- [ ] "Did you mean?" suggestions

#### Technical Notes
```rust
use tantivy::query::FuzzyTermQuery;

pub fn build_fuzzy_query(term: &str) -> Box<dyn Query> {
    let distance = match term.len() {
        0..=3 => 0,  // No fuzzy for very short
        4..=6 => 1,
        _ => 2,
    };

    FuzzyTermQuery::new_prefix(
        Term::from_field_text(field, term),
        distance,
        true,  // transpositions
    )
}
```

#### Success Metrics
- "functoin" finds "function"
- Fuzzy overhead < 20ms
- False positives < 5%

---

### P2-010: Implement ReadTheDocs scraper

**Priority:** Critical
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-008 (Generic scraper)
**Blocks:** P2-014

#### Description
Build a specialized scraper for ReadTheDocs/Sphinx documentation.

#### Acceptance Criteria
- [ ] Detect ReadTheDocs by domain or HTML markers
- [ ] Parse searchindex.js for doc structure
- [ ] Extract semantic HTML (not raw Sphinx output)
- [ ] Handle versioned documentation
- [ ] Extract breadcrumb navigation
- [ ] Handle inter-doc references
- [ ] Support custom domains (not just *.readthedocs.io)
- [ ] Extract API reference structure

#### Technical Notes
```rust
pub struct ReadTheDocsScraper {
    base_scraper: GenericScraper,
}

impl ReadTheDocsScraper {
    pub async fn scrape(&self, url: &str) -> Result<DocSet> {
        // 1. Fetch searchindex.js
        let search_index = self.fetch_search_index(url).await?;

        // 2. Parse doc tree structure
        let doc_tree = parse_sphinx_index(&search_index)?;

        // 3. Fetch each page with structure awareness
        let pages = self.fetch_pages(&doc_tree).await?;

        // 4. Normalize with Sphinx-specific rules
        let normalized = pages.iter()
            .map(|p| self.normalize_sphinx_html(p))
            .collect();

        Ok(DocSet { pages: normalized, tree: doc_tree })
    }
}
```

#### Success Metrics
- Scrapes docs.python.org structure correctly
- Preserves API reference hierarchy
- Version switching detected

---

### P2-011: Build rustdoc scraper

**Priority:** High
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-008 (Generic scraper)
**Blocks:** P2-014

#### Description
Create a scraper that understands rustdoc output structure.

#### Acceptance Criteria
- [ ] Parse search-index.js for crate structure
- [ ] Extract module/type/function hierarchy
- [ ] Handle trait implementations
- [ ] Extract doc comments as content
- [ ] Link source code references
- [ ] Support workspace/multi-crate docs
- [ ] Handle doc.rust-lang.org and docs.rs
- [ ] Extract deprecation notices

#### Technical Notes
```rust
pub struct RustdocScraper;

impl RustdocScraper {
    pub async fn scrape(&self, url: &str) -> Result<DocSet> {
        // 1. Detect docs.rs vs doc.rust-lang.org vs local
        let doc_type = detect_rustdoc_type(url)?;

        // 2. Fetch and parse search-index.js
        let search_index = self.fetch_search_index(url, &doc_type).await?;

        // 3. Build module tree
        let modules = parse_rustdoc_index(&search_index)?;

        // 4. Fetch pages with rustdoc-specific extraction
        // Focus on: module doc, struct/enum, fn signatures, trait impls
    }
}

// search-index.js structure:
// searchIndex["crate_name"] = {"doc":"...","t":[...],"n":[...],...}
```

#### Success Metrics
- Correctly parses std library structure
- Type signatures extracted accurately
- Cross-references between types work

---

### P2-012: Create mdBook scraper

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P1-008 (Generic scraper)
**Blocks:** P2-014

#### Description
Build a scraper for mdBook-generated documentation.

#### Acceptance Criteria
- [ ] Detect mdBook by book.toml or structure
- [ ] Parse SUMMARY.md for chapter hierarchy
- [ ] Handle nested chapters
- [ ] Extract frontmatter metadata
- [ ] Process markdown with syntax highlighting
- [ ] Handle internal links correctly
- [ ] Support draft chapters (optional indexing)

#### Technical Notes
```rust
pub struct MdBookScraper;

impl MdBookScraper {
    pub async fn scrape(&self, url: &str) -> Result<DocSet> {
        // 1. Fetch and parse SUMMARY.md or book.toml
        let summary = self.fetch_summary(url).await?;

        // 2. Build chapter tree
        let chapters = parse_summary(&summary)?;

        // 3. Fetch each chapter
        let pages = self.fetch_chapters(&chapters).await?;

        Ok(DocSet { pages, tree: chapters })
    }
}

// SUMMARY.md structure:
// # Summary
// - [Chapter 1](./chapter1.md)
//   - [Section 1.1](./chapter1/section1.md)
```

#### Success Metrics
- Rust Book scrapes correctly
- Chapter ordering preserved
- Nested chapters up to 5 levels

---

### P2-013: Implement man page integration

**Priority:** High
**Complexity:** L (1-2 weeks)
**Dependencies:** P1-008 (Generic scraper)
**Blocks:** None

#### Description
Add first-class support for Unix manual pages.

#### Acceptance Criteria
- [ ] Discover man pages from configured paths
- [ ] Parse with mandoc -T html
- [ ] Apply Tome typography styling
- [ ] Section-aware organization (1-8)
- [ ] Cross-reference linking (see also)
- [ ] Index NAME section for search
- [ ] Handle compressed man pages (.gz)
- [ ] Support macOS and Linux paths

#### Technical Notes
```rust
pub struct ManPageScraper {
    paths: Vec<PathBuf>,
    sections: Vec<u8>,
}

impl ManPageScraper {
    pub fn discover(&self) -> Result<Vec<ManPage>> {
        let mut pages = Vec::new();
        for path in &self.paths {
            for section in &self.sections {
                let section_path = path.join(format!("man{}", section));
                if section_path.exists() {
                    for entry in fs::read_dir(section_path)? {
                        // Parse man page metadata
                    }
                }
            }
        }
        Ok(pages)
    }

    pub fn render(&self, page: &ManPage) -> Result<String> {
        let output = Command::new("mandoc")
            .args(["-T", "html", "-O", "fragment"])
            .arg(&page.path)
            .output()?;

        let html = String::from_utf8(output.stdout)?;
        self.apply_styling(html)
    }
}
```

#### Success Metrics
- Index 5000+ man pages in < 2 minutes
- mandoc rendering accurate
- Cross-references resolved 95%+

---

### P2-014: Build platform auto-detection

**Priority:** High
**Complexity:** M (3-5 days)
**Dependencies:** P2-010, P2-011, P2-012
**Blocks:** None

#### Description
Automatically detect documentation platform from URL.

#### Acceptance Criteria
- [ ] Detect ReadTheDocs (domain + HTML markers)
- [ ] Detect rustdoc (structure + search-index)
- [ ] Detect mdBook (book.toml, SUMMARY.md)
- [ ] Detect GitBook (for future)
- [ ] Fall back to generic scraper
- [ ] Confidence score for detection
- [ ] User override capability
- [ ] Fast detection (single request preferred)

#### Technical Notes
```rust
pub enum DetectedPlatform {
    ReadTheDocs { version: Option<String> },
    Rustdoc { crate_name: String },
    MdBook,
    GitBook,
    Docusaurus,
    MkDocs,
    Generic,
}

pub async fn detect_platform(url: &str) -> Result<(DetectedPlatform, f32)> {
    let response = fetch_with_headers(url).await?;

    // Check URL patterns
    if url.contains("readthedocs") {
        return Ok((DetectedPlatform::ReadTheDocs { version: None }, 0.95));
    }

    // Check HTML markers
    let html = response.text().await?;
    if html.contains("sphinx") || html.contains("_static/documentation_options.js") {
        return Ok((DetectedPlatform::ReadTheDocs { version: None }, 0.8));
    }

    if html.contains("search-index.js") && html.contains("rustdoc") {
        return Ok((DetectedPlatform::Rustdoc { crate_name: extract_crate(url) }, 0.9));
    }

    // More checks...

    Ok((DetectedPlatform::Generic, 1.0))
}
```

#### Success Metrics
- Correct detection 95%+ for known platforms
- Detection < 2 seconds
- Clear fallback behavior

---

### P2-015: Create symbol-aware search

**Priority:** Medium
**Complexity:** M (3-5 days)
**Dependencies:** P2-002, P2-006
**Blocks:** None

#### Description
Enhance search to recognize and prioritize code symbols.

#### Acceptance Criteria
- [ ] Recognize patterns: fn, struct, class, def, function
- [ ] Symbol queries prioritize symbol matches
- [ ] Syntax: `@symbol` forces symbol search
- [ ] Extract symbols during indexing
- [ ] Language-specific symbol patterns
- [ ] Symbol type in results (function, type, module)

#### Technical Notes
```rust
// During indexing
fn extract_symbols(code: &str, language: &str) -> Vec<Symbol> {
    match language {
        "rust" => extract_rust_symbols(code),
        "python" => extract_python_symbols(code),
        "javascript" | "typescript" => extract_js_symbols(code),
        _ => vec![],
    }
}

// Rust patterns
fn extract_rust_symbols(code: &str) -> Vec<Symbol> {
    let patterns = [
        (r"fn\s+(\w+)", SymbolType::Function),
        (r"struct\s+(\w+)", SymbolType::Type),
        (r"enum\s+(\w+)", SymbolType::Type),
        (r"trait\s+(\w+)", SymbolType::Trait),
        (r"mod\s+(\w+)", SymbolType::Module),
    ];
    // Extract matches
}
```

#### Success Metrics
- "@Vec" finds struct Vec immediately
- Symbol extraction covers 80%+ of common patterns
- No false positives for prose

---

### P2-016: Implement search history

**Priority:** Medium
**Complexity:** S (1-2 days)
**Dependencies:** P2-004
**Blocks:** None

#### Description
Track and display recent search queries.

#### Acceptance Criteria
- [ ] Store last 50 searches
- [ ] Show recent searches when search opens with empty query
- [ ] Click recent to re-execute
- [ ] Clear individual or all history
- [ ] Persist across sessions
- [ ] Deduplicate consecutive identical searches

#### Technical Notes
```typescript
interface SearchHistory {
  queries: Array<{
    query: string;
    scope: SearchScope;
    timestamp: number;
    resultCount: number;
  }>;
}

class SearchHistoryManager {
  private readonly maxItems = 50;

  add(entry: SearchHistoryEntry): void {
    // Dedupe, add to front, trim to max
  }

  getRecent(limit: number = 10): SearchHistoryEntry[] {
    return this.history.slice(0, limit);
  }

  clear(): void {
    this.history = [];
    this.persist();
  }
}
```

#### Success Metrics
- History loads in < 10ms
- Persistence reliable
- UI updates immediately

---

### P2-017: Add search keyboard navigation

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P2-004, P2-005
**Blocks:** None

#### Description
Enable full keyboard control of search interface.

#### Acceptance Criteria
- [ ] ↑/↓ to navigate results
- [ ] Enter to select/open result
- [ ] Cmd+Enter to open in new window (future)
- [ ] Tab to move between result groups
- [ ] Preview selected result (optional)
- [ ] Visual selection indicator
- [ ] Wrap around at list ends

#### Technical Notes
```svelte
<script>
  let selectedIndex = 0;

  function handleKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, results.length - 1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        break;
      case 'Enter':
        e.preventDefault();
        if (results[selectedIndex]) {
          navigate(results[selectedIndex]);
        }
        break;
    }
  }
</script>

<div on:keydown={handleKeydown}>
  <!-- Search UI -->
</div>
```

#### Success Metrics
- Selection updates in < 16ms
- No missed keystrokes
- Scroll follows selection

---

### P2-018: Build search performance benchmarks

**Priority:** Medium
**Complexity:** S (1-2 days)
**Dependencies:** P2-001
**Blocks:** None

#### Description
Create automated benchmarks to track search performance.

#### Acceptance Criteria
- [ ] Benchmark index build time
- [ ] Benchmark query latency (simple, complex, fuzzy)
- [ ] Benchmark with varying index sizes
- [ ] Memory usage during indexing
- [ ] Regression detection in CI
- [ ] Performance report generation

#### Technical Notes
```rust
#[bench]
fn bench_simple_query(b: &mut Bencher) {
    let engine = setup_test_engine(1000); // 1000 pages
    b.iter(|| {
        engine.search("iterator", 10).unwrap();
    });
}

#[bench]
fn bench_index_build(b: &mut Bencher) {
    let pages = generate_test_pages(1000);
    b.iter(|| {
        let engine = SearchEngine::new_temp().unwrap();
        for page in &pages {
            engine.index_page(page).unwrap();
        }
        engine.commit().unwrap();
    });
}
```

#### Success Metrics
- Benchmarks complete in CI < 5 minutes
- Regression alerts for > 20% slowdown
- Coverage of critical paths

---

## Phase 2 Dependency Graph

```
                    P1-004 (SQLite)
                         │
                         ▼
P1-021 (Page metadata) ─── P2-001 (Tantivy) ─────────────────┐
                              │                               │
                              ▼                               │
                         P2-002 (Schema) ──────┬──────────────┤
                              │                │              │
                              ▼                ▼              │
                         P2-003 (Indexing)  P2-015 (Symbols)  │
                                               │              │
                                               ▼              │
                                          P2-006 (Ranking) ◄──┤
                                                              │
         ┌────────────────────────────────────────────────────┘
         │
         ▼
    P2-004 (Search UI) ────┬──── P2-005 (Results) ───── P2-017 (Keyboard)
         │                 │            │
         ▼                 ▼            ▼
    P2-016 (History)  P2-008 (Scoping)  │
                                        │
    P1-016 (WebView) ──── P2-007 (In-page search)
                                        │
                         P2-009 (Fuzzy) ◄────── P2-001

    P1-008 (Generic Scraper)
         │
         ├──── P2-010 (ReadTheDocs) ──┐
         │                            │
         ├──── P2-011 (Rustdoc) ──────┼──── P2-014 (Auto-detect)
         │                            │
         ├──── P2-012 (mdBook) ───────┘
         │
         └──── P2-013 (Man pages)

    P2-001 ──── P2-018 (Benchmarks)
```

---

## Exit Criteria Checklist

- [ ] Tantivy integrated and indexing documents
- [ ] Global search (Cmd+K) functional with live results
- [ ] In-page search (Cmd+F) working
- [ ] Search results ranked by relevance
- [ ] Fuzzy matching handles typos
- [ ] ReadTheDocs scraper working (test: docs.python.org)
- [ ] rustdoc scraper working (test: doc.rust-lang.org/std)
- [ ] mdBook scraper working (test: Rust Book)
- [ ] Man pages indexed and searchable
- [ ] Platform auto-detection working
- [ ] Search latency < 100ms (P95)
- [ ] Index build < 30s for 1000 pages
