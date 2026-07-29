# Phase 2: Search & Platforms (v0.2)

**Goal:** Intelligent search and platform-specific scrapers
**Tickets:** 20
**Effort:** ~85 person-days (≈ 2.9 FTE against the 6-week calendar target)
**Prerequisites:** Phase 1 complete
**Exit Criteria:** Fast search across multiple doc sources, platform scrapers work reliably, **and
both claims are backed by a measurable eval set rather than an impression**

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
| P2-019 | Build search relevance eval set and harness | M | Critical | P2-001 |
| P2-020 | Build platform detection corpus and harness | S | High | P2-014 |

---

> **Two tickets added.** Phase 2 previously asserted "relevant result in top 3 for 90 %+ of
> queries" (P2-006) and "correct detection 95 %+" (P2-014) as success metrics with nothing capable
> of measuring either. A quality bar with no measurement is an opinion. P2-019 and P2-020 build the
> measurement, and they must land *before* the tickets whose targets depend on them, because
> tuning ranking without an eval set is how search quality silently regresses.

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
- [ ] Index directory created at `~/Library/Caches/Tome/index/` (path via P1-006) — the index is
      rebuildable, so it belongs in the cache, not alongside irreplaceable state
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
- [x] Opens with Cmd+K anywhere in app — and toggles, so the same key dismisses it
- [x] Centered modal with backdrop
- [x] Large search input field
- [x] Live results as you type (debounced 150 ms), with stale responses discarded — a debounce does not serialise requests, and a slow early query resolving late would overwrite the results of the query the user actually finished typing
- [x] Loading indicator during search
- [x] Close with Escape or click outside
- [x] Focus returns to previous element on close
- [x] Scope indicator showing search context

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
- [x] Show source name, page title, and snippet — the source's *display* name, resolved in Rust so the list does not need a round trip per result
- [x] Highlight matched terms in snippet — including terms the query was *corrected* to, or a typo-corrected result looks unrelated to what was typed
- [x] Show result count and search time. The count is the returned count with a `+` when truncated, **not a total**: a total needs a second uncapped collector pass, and an invented number is worse than none
- [x] Click result to navigate, across sources
- [~] Score is carried on every hit and deliberately not drawn. A BM25 score is not a percentage and has no scale a reader can interpret; showing it invites comparing numbers between queries, where they mean nothing. The `[type]`/`[function]` symbol badge is the useful per-result signal and is shown instead
- [~] Not built. Grouping fights ranking: the whole point of S2-4/5/6 is a single ordering across sources, and grouping re-sorts by something the eval set does not measure. Scoping (P2-008) answers the same need without discarding the ranking
- [x] No results state, with the "did you mean?" corrections and a one-click escape from a scope that is hiding everything
- [x] Error state — reported rather than shown as an empty list, which is indistinguishable from "your library does not contain this"

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
- [x] Scope to single source
- [~] Not built. Every source in a category shares its category string, so this is a filter over the same `source_id` list with no new mechanism; it is UI work that belongs with a category-aware source list
- [x] Scope indicator in search UI
- [x] Clear scope with '×' button
- [~] Not built. Cmd+K already opens search; a second shortcut to change scope inside it needs a chord that Appendix C does not allocate, and allocating one here is how the shortcut table drifted before
- [x] Remember last used scope — **validated on load**, because a source can be removed between launches and a scope naming one that is gone would silently return nothing for ever
- [~] In-page search is P2-007/S2-8 and does not exist yet

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
- [x] Edit distance 0 for terms ≤ 3 chars, 1 for 4–6, **2 for longer — 2 is the maximum**
- [x] Prefix matching for partial words — candidate search is prefix-anchored; see the limitation
- [x] Fuzzy applied only to terms that produce no exact match, not to every term
- [x] Fuzzy results ranked strictly below exact matches
- [x] Configurable fuzzy threshold — `Ranking::fuzzy` (weight) and `Ranking::fuzzy_max_distance`
- [x] "Did you mean?" suggestions — `SearchEngine::suggest`, surfaced by `tome search`
- [x] Multi-term queries: fuzzy is applied per-term and recombined, not to the whole query string

> The original criterion "edit distance of 2-3 for longer words" is not implementable: Tantivy's
> Levenshtein automaton supports a maximum distance of 2. Distance 3 on a large index is also a
> latency trap — the candidate set explodes.

#### Technical Notes

**Built without `FuzzyTermQuery`, deliberately** (S2-5, `tome-core/src/search/fuzzy.rs`). The
sketch below was the plan; it does not survive contact with how Tantivy scores.

`FuzzyTermQuery` is built on an `AutomatonWeight`, and an `AutomatonWeight` produces a
**`ConstScorer`** — every document it matches gets an identical score. Dropping one into the query
would hand every fuzzy hit the same score and discard BM25 for that term: the page whose *subject*
is environment variables would rank level with one that mentions them in a footnote, and none of
S2-4's ranking would reach it. It also cannot answer "did you mean?", because it never reveals
which terms it matched.

So the implementation corrects the **query** rather than relaxing the match. A term found nowhere
in the index is looked up in the term dictionary, the nearest real term wins (ties broken by
document frequency — the commoner word is the likelier intent), and the correction is searched as
an ordinary term, scored and boosted like anything else. Three consequences:

- "Did you mean?" is a by-product rather than a separate feature.
- "Fuzzy ranks strictly below exact" is *structural*: corrections are only generated for terms that
  match nothing, so no document exists that matched the typo exactly and could be displaced.
- A wrong correction shows up in the relevance eval as a ranking change, instead of a flat bonus
  smeared across hundreds of documents where nothing stands out.

**The limitation:** candidates are found by scanning the term dictionary from a three-character
prefix, so a typo *inside* that prefix is not corrected — `teh` will not find `the`. A Levenshtein
automaton has no such blind spot, but `DfaWrapper` is `pub(crate)`, so reaching one means taking
`levenshtein-automata` and `tantivy-fst` as direct dependencies pinned to whatever Tantivy
resolves. The prefix is what makes this affordable: without it, correcting one term means reading
the whole dictionary.

**Two eval-corpus typos are out of reach of the specified schedule**, and are recorded rather than
worked around: `modual` → `module` is a transposition *and* a substitution (2 edits on a
6-character term, which allows 1), and `pth` → `path` is 1 edit on a 3-character term, which allows
0. Widening the schedule is a change to this specification, not a tuning decision, and it buys
false positives everywhere — at distance 1 on three-character terms, `Vec` reaches `Vex`, `Vev`,
`sec` and `hex`.

#### Success Metrics
- ✅ "functoin" finds "function" — measured on the eval corpus's real misspellings: `misspelling`
  recall@3 went **0.4167 → 0.7500** and MRR **0.3500 → 0.6333**, with nothing outside that category
  moved by turning it on
- ✅ Fuzzy overhead < 20 ms — measured at **~2 µs** for a correctly spelled query (no term misses,
  so no scan happens) and **~65 µs per misspelled term**, on the 339-document corpus. Scan cost
  tracks *vocabulary*, not page count (SPIKE-003 finding 2), so this needs re-measuring at 100k
  pages — S2-12's. `fuzzy_cost` in `tests/relevance.rs` is the measurement
- ✅ False positives < 5% — one query of 207 regressed when tolerance was enabled
  (`ms-kubernets`, rank 1 → 3)

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

> **`searchindex.js` is a JavaScript file, not JSON, and its format is not a public API.** It is
> emitted as `Search.setIndex({...})` (older Sphinx) or a bare object, may be minified, and its
> internal shape changed across Sphinx 4/5/6/7. Parsing it requires either a tolerant JS-object
> parser or per-version handling — budget for that, and treat SPIKE-006 as a genuine gate on this
> ticket rather than a formality. **Fall back to HTML crawling whenever parsing fails**; the index
> is an optimization that yields structure, not a replacement for fetching pages.

#### Success Metrics
- Scrapes docs.python.org structure correctly
- Preserves API reference hierarchy
- Version switching detected
- Parses ≥ 95 % of the Sphinx corpus collected in SPIKE-006, and degrades to the generic crawler
  (not an error) on the remainder

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

> **rustdoc's `search-index.js` is explicitly an unstable internal format.** It changes without
> notice between Rust releases and has done so repeatedly. Pin the ticket to "current stable plus
> the two previous formats", detect the format version defensively, and make failure fall back to
> HTML extraction rather than erroring. Assume this scraper will need maintenance every few months —
> that ongoing cost is RISK-003, and the registry CI check (PRD § Source Registry) is what turns it
> from silent rot into a tracked signal.

#### Success Metrics
- Correctly parses std library structure
- Type signatures extracted accurately
- Cross-references between types work
- Detects an unrecognised index format and falls back cleanly instead of producing partial garbage

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
- [x] Recognize patterns: fn, struct, class, def, function — and `func`, `type`, `trait`, `mod`,
  `const`, `interface`, `macro`; see `SymbolKind::from_keyword`
- [x] Symbol queries prioritize symbol matches — the page's **primary** symbol is a boosted field
- [x] Syntax: `@symbol` forces symbol search
- [x] Extract symbols during indexing
- [x] Language-specific symbol patterns — rustdoc paths and titles, source-style signatures, Sphinx
  definition terms, and Node's bare `os.cpus()` headings
- [x] Symbol type in results (function, type, module) — `Hit::symbol_kind`, shown by `tome search`

#### Technical Notes

**The declarations are not in the code blocks.** The sketch that used to sit here regexed
`fn\s+(\w+)` out of code blocks, per language. Run over the 339-page relevance corpus, that finds
2 821 declarations whose most common names are:

```text
main, buf, server, Foo, foo, __init__, char, import, myURL, req, options
```

Those are the *examples'* scaffolding. The symbols users actually search for barely appear as
declarations at all: `Vec` is declared once and mentioned 321 times, `with_capacity` is declared
**never** and mentioned 30, `os.cpus` never. A symbol field filled from code blocks would fail this
ticket's own "no false positives for prose" criterion.

The patterns were right and the *place* was wrong. Documentation generators put signatures in
**headings**:

```text
path : std/vec/struct.Vec.html
title: Struct Vec
h4   : pub fn with_capacity(capacity: usize) -> Vec<T>
```

So `search::symbols` reads three things, in descending order of trust: the **page path** (rustdoc
encodes the kind in the filename — `struct.Vec.html`), the **title** (`Struct Vec`), and
**headings** (signatures, Sphinx definition terms, and Node's bare `os.cpus()` form). This is also
why S2-4 measured the `code` field as barely load-bearing — `headers` was already carrying the
method names.

**Two fields, because two different questions are being asked.**

| Field | Holds | Used by |
|---|---|---|
| `symbol` | the page's **primary** symbol — one term | ordinary ranking, boosted |
| `declarations` | **every** symbol the page declares | `@symbol` only |

Blending *all* declarations was measured and is bad: at boost 3.0 it cost 0.08 MRR and made 39
queries worse, because a rustdoc page declares `from`, `into`, `borrow`, `fmt` and `try_from` as
trait boilerplate and a short field makes each a strong BM25 signal. Coordinate descent then drove
that boost to **zero** — the eval set saying the field was worth nothing blended. Restricted to the
primary symbol it earns 1.5 and lifts `symbol` MRR from 0.8815 to 0.8919. As an *explicit* filter
the full set is exactly right: someone typing `@borrow` wants the pages that declare `borrow`.

**Adding these fields changes the index schema**, so every existing library's index is unreadable
until rebuilt. `Error::IndexSchemaOutdated` says so and names the remedy; a read command must not
silently delete an index to recover.

#### Success Metrics
- ✅ "@Vec" finds struct Vec immediately — and, unlike a boost, returns *only* pages that declare
  it. Both a reference page and a prose page contain the word; `@` separates them
- ✅ Symbol extraction covers 80%+ of common patterns — measured by `symbol_extraction_report`:
  2 632 symbols across 84 rust-std pages, 1 765 across 82 Python pages, 918 across 42 Node pages.
  Go's 21 pages yield 0 and Kubernetes' 55 yield 1, correctly — they are tutorials and concept
  prose, and declare nothing
- ✅ No false positives for prose — the report caught the one real case (`Trait Implementations`
  became the symbol `Implementations` on 35 of 84 rust-std pages) and it is fixed and pinned by a
  test. Prose headings — `Examples`, `Guarantees`, `Capacity and reallocation` — yield nothing
- ✅ Relevance: `symbol` recall@1 0.8108 → **0.8243**, MRR 0.8817 → **0.8919**

---

### P2-016: Implement search history

**Priority:** Medium
**Complexity:** S (1-2 days)
**Dependencies:** P2-004
**Blocks:** None

#### Description
Track and display recent search queries.

#### Acceptance Criteria
- [x] Store last 50 searches
- [x] Show recent searches when search opens with empty query
- [x] Click recent to re-execute
- [x] Clear individual or all history — and `clear` removes the bytes rather than setting a flag. This is reading history
- [x] Persist across sessions, in `localStorage` — every read defensive, because the store can hold what another version wrote or what someone typed into the inspector
- [x] Deduplicated against the **whole** list, not just the previous entry: a search re-run from the recents list has whatever was searched in between sitting between the two, so a consecutive-only check would record it twice

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
- [x] ↑/↓ to navigate results, plus Home/End
- [x] Enter to select/open result
- [~] Cmd+Enter to open in new window — the ticket already marks this future; Tome is single-window
- [~] There are no result groups; see P2-005 on why grouping is not built
- [~] Optional, and not built: the snippet is the preview
- [x] Visual selection indicator, mirrored into `aria-selected` so it is not colour-only
- [x] Wrap around at both ends. Without it, holding ↓ stops silently at the last result and the user cannot tell whether the key stopped working or the list ended

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

### P2-019: Build search relevance eval set and harness

**Priority:** Critical
**Complexity:** M (3-5 days)
**Dependencies:** P2-001
**Blocks:** P2-006, P2-009, P2-015 (they cannot be verified without it)

#### Description
Create a labelled query set and an offline harness that scores ranking quality, so that changes to
boosts, tokenizers, and fuzzy behaviour are measured rather than guessed.

#### Acceptance Criteria
- [ ] ≥ 200 queries across ≥ 5 fixture sources, covering: exact symbol lookups (`Vec::new`),
      natural-language questions ("how do I read a file"), misspellings, multi-word phrases,
      acronyms, and queries with a correct answer in *another* source than the obvious one
- [ ] Each query labelled with one or more acceptable target pages
- [ ] Harness reports MRR, recall@1, recall@3, recall@10, and per-query deltas against the last run
- [ ] Runs offline against a fixed index built from committed fixtures — no network, deterministic
- [ ] CI fails on a regression greater than an agreed margin, and prints which queries moved
- [ ] Adding a query is a one-line change to a YAML file

#### Technical Notes
Queries should be drawn from real usage, not invented: mine the maintainers' own shell history and
browser history for what they actually search for. A synthetic eval set measures agreement with
your assumptions.

Grade on *acceptable targets*, not a single gold answer — documentation frequently has several
correct destinations, and a strict single-answer set punishes correct behaviour.

#### Success Metrics
- Full eval runs in < 60 s in CI
- Baseline recorded before any ranking work begins in P2-006

---

### P2-020: Build platform detection corpus and harness

**Priority:** High
**Complexity:** S (1-2 days)
**Dependencies:** P2-014
**Blocks:** None

#### Description
Save real documentation homepages and assert the detector classifies them correctly.

#### Acceptance Criteria
- [ ] ≥ 100 saved homepages (HTML + response headers) spanning Sphinx/ReadTheDocs, rustdoc,
      mdBook, GitBook, Docusaurus, MkDocs, and sites that are none of these
- [ ] Includes deliberate hard cases: custom domains, heavily themed Sphinx, Docusaurus that looks
      like MkDocs, and a plain marketing site that must classify as Generic
- [ ] Harness asserts predicted platform and reports a confusion matrix
- [ ] Runs offline from committed fixtures
- [ ] Fixtures record the date captured, so staleness is visible

#### Technical Notes
Store fixtures compressed and strip anything user-identifying. Keep them small — the homepage plus
whatever manifest file the detector actually reads, not a full crawl.

**Detection must be allowed to say "I don't know."** The original detector returned
`(Generic, 1.0)` — full confidence in the fallback — which makes low-confidence detection
indistinguishable from a confident answer. Return a confidence below the auto-accept threshold and
let the UI ask.

#### Success Metrics
- ≥ 95 % correct on the corpus, with no confident-but-wrong classification of a non-doc site

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
- [ ] Search latency < 100ms (P95) — **measured by the P2-018 benchmark, not by feel**
- [ ] Index build < 30s for 1000 pages
- [ ] **Relevance eval (P2-019) passes: correct page in top 3 for ≥ 90 % of the labelled query set**
- [ ] **Detection eval (P2-020) passes: ≥ 95 % on the saved corpus**
- [ ] Every scraper degrades to the generic crawler on parse failure; none returns partial content
      silently
