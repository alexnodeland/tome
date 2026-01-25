# Technical Spikes

**Purpose:** De-risk unknowns through focused research and prototyping before committing to implementation.

---

## Overview

Technical spikes are time-boxed investigations to reduce uncertainty. Each spike has a clear question to answer, a maximum time budget, and defined outputs.

### Spike Priority Levels

| Priority | Timing | Rationale |
|----------|--------|-----------|
| **P0** | Before Phase 1 starts | Blocks architecture decisions |
| **P1** | During Phase 1 | Blocks dependent tickets |
| **P2** | Before relevant phase | Can inform but not block |

---

## P0 Spikes (Pre-Phase 1)

### SPIKE-001: Tauri + Swift AppKit Integration

**Question:** Can we achieve native macOS menu bar integration with Tauri while using Swift for the AppKit shell?

**Time Budget:** 3 days

**Background:**
Tome requires a native macOS menu bar experience (status item, popover, global shortcuts). Tauri provides some macOS integration, but we need to verify we can extend it with custom Swift code for features like:
- NSStatusItem with custom popover
- Global keyboard shortcuts (Carbon/CGEvent)
- Native notifications with actions

**Investigation Tasks:**
- [ ] Create minimal Tauri app with Swift plugin
- [ ] Implement NSStatusItem with click handling
- [ ] Test bidirectional communication (Swift ↔ Rust ↔ JS)
- [ ] Measure IPC latency overhead
- [ ] Document integration pattern

**Success Criteria:**
- Menu bar icon renders and responds to clicks
- Can invoke Rust commands from Swift
- Can trigger Swift functions from Rust
- IPC latency < 10ms

**Outputs:**
- Working prototype in `/spikes/tauri-swift/`
- Architecture decision record (ADR)
- Integration guide for team

**Fallback:**
If integration proves too complex, consider:
- Pure Tauri menu bar (limited features)
- Separate Swift helper process
- Alternative framework (Electron with native module)

---

### SPIKE-002: WKWebView JavaScript Bridge Performance

**Question:** Can we achieve smooth bidirectional communication between WKWebView and Rust with acceptable latency?

**Time Budget:** 2 days

**Background:**
The reader pane uses WKWebView for rendering documentation. We need:
- Rust → JS: Push content updates, scroll commands
- JS → Rust: Link clicks, scroll position, highlight creation
- High frequency updates (scroll tracking at 60fps)

**Investigation Tasks:**
- [ ] Benchmark `WKScriptMessageHandler` latency
- [ ] Benchmark `evaluateJavaScript` latency
- [ ] Test high-frequency message passing (60/sec)
- [ ] Measure memory overhead of message queue
- [ ] Compare with Tauri's built-in webview bridge

**Success Criteria:**
- Single message round-trip < 5ms
- 60 messages/sec sustained without frame drops
- Memory stable under load

**Outputs:**
- Benchmark results document
- Recommended communication pattern
- Code samples for common operations

**Fallback:**
If performance insufficient:
- Batch messages to reduce frequency
- Use shared memory for scroll position
- Consider native text rendering for critical paths

---

### SPIKE-003: Tantivy Memory Usage at Scale

**Question:** What is Tantivy's memory footprint when indexing and searching 100,000+ documentation pages?

**Time Budget:** 2 days

**Background:**
A power user might have 50+ documentation sources, potentially 100,000+ pages. We need to understand:
- Memory usage during indexing
- Memory usage during search (cold vs warm)
- Index file size on disk
- Search latency at scale

**Investigation Tasks:**
- [ ] Generate synthetic doc set (100K pages, varied sizes)
- [ ] Measure memory during bulk indexing
- [ ] Measure index size on disk
- [ ] Benchmark search latency at various index sizes
- [ ] Test incremental indexing memory usage
- [ ] Profile with Instruments

**Success Criteria:**
- Indexing 100K pages uses < 1GB RAM peak
- Search latency < 100ms at 100K pages
- Index size < 500MB for 100K pages
- Idle memory < 50MB (index not loaded)

**Outputs:**
- Scaling characteristics document
- Memory optimization recommendations
- Configuration recommendations for large indexes

**Fallback:**
If memory usage too high:
- Implement index sharding by source
- Lazy-load index segments
- Consider SQLite FTS5 for metadata, Tantivy for content only

---

## P1 Spikes (During Phase 1)

### SPIKE-004: CloudKit Sync Complexity

**Question:** What are the edge cases and limitations of CloudKit for syncing bookmarks and reading state?

**Time Budget:** 3 days

**Background:**
iCloud sync via CloudKit is core to Phase 3. We need to understand:
- Rate limits and quotas
- Conflict resolution mechanics
- Offline behavior
- Initial sync performance
- Record size limits

**Investigation Tasks:**
- [ ] Create test CloudKit container
- [ ] Implement basic CKRecord CRUD
- [ ] Test conflict scenarios (simultaneous edits)
- [ ] Measure sync latency (local → cloud → other device)
- [ ] Test offline queue behavior
- [ ] Document quota limits hit during testing

**Success Criteria:**
- Understand all CloudKit quotas relevant to Tome
- Conflict resolution strategy validated
- Sync latency < 5s for single record change
- Offline changes sync correctly when online

**Outputs:**
- CloudKit limitations document
- Sync architecture recommendation
- Test scenarios for Phase 3

**Fallback:**
If CloudKit proves problematic:
- Consider CouchDB/PouchDB for sync
- Simple file-based sync via iCloud Drive
- Optional self-hosted sync server

---

### SPIKE-005: mandoc HTML Output Quality

**Question:** Does `mandoc -T html` produce sufficiently clean HTML for Tome's rendering pipeline?

**Time Budget:** 1 day

**Background:**
Man page rendering relies on mandoc's HTML output. We need to verify:
- HTML structure is parseable
- Cross-references are extractable
- Output works with our typography system
- Edge cases (tables, Unicode, code blocks)

**Investigation Tasks:**
- [ ] Run mandoc on 100 diverse man pages
- [ ] Analyze HTML structure consistency
- [ ] Test with our normalization pipeline
- [ ] Identify problematic patterns
- [ ] Test on both macOS system mandoc and Homebrew version

**Success Criteria:**
- 95%+ of man pages render acceptably
- Cross-references extractable programmatically
- No crashes on malformed man pages

**Outputs:**
- mandoc compatibility report
- List of required normalizations
- Recommendation: use system mandoc vs bundle our own

**Fallback:**
If mandoc output insufficient:
- Use groff with custom macros
- Implement custom man page parser
- Render as plain text with basic formatting

---

### SPIKE-006: Sphinx searchindex.js Parsing

**Question:** Can we reliably parse Sphinx's searchindex.js across different Sphinx versions?

**Time Budget:** 2 days

**Background:**
ReadTheDocs/Sphinx documentation uses searchindex.js for client-side search. We want to parse this for:
- Document hierarchy
- Page metadata
- Search terms (to avoid re-scraping)

**Investigation Tasks:**
- [ ] Collect searchindex.js from 20+ Sphinx sites
- [ ] Identify format variations across Sphinx versions
- [ ] Build parser that handles all variations
- [ ] Test with compressed vs uncompressed formats
- [ ] Handle missing or malformed indexes

**Success Criteria:**
- Parse 95%+ of collected searchindex.js files
- Extract document tree structure
- Handle graceful fallback for unparseable files

**Outputs:**
- searchindex.js format documentation
- Parser implementation (reusable in Phase 2)
- List of known edge cases

**Fallback:**
If parsing unreliable:
- Fall back to HTML crawling for structure
- Use searchindex.js only as optimization hint

---

## P2 Spikes (Before Relevant Phase)

### SPIKE-007: rustdoc search-index.js Format

**Question:** How stable is rustdoc's search-index.js format, and can we parse it reliably?

**Time Budget:** 2 days

**Background:**
Similar to Sphinx, rustdoc generates search-index.js with crate structure. Format has changed between Rust versions.

**Investigation Tasks:**
- [ ] Collect search-index.js from docs.rs (multiple Rust versions)
- [ ] Document format evolution
- [ ] Build version-aware parser
- [ ] Test with workspace (multi-crate) docs

**Success Criteria:**
- Parse current stable rustdoc format
- Handle at least 2 previous format versions
- Extract module/type/function hierarchy

**Outputs:**
- rustdoc format documentation
- Parser implementation
- Version detection heuristics

---

### SPIKE-008: MCP Protocol Implementation

**Question:** What's the minimal viable MCP server implementation for Tome's use case?

**Time Budget:** 2 days

**Background:**
MCP (Model Context Protocol) is relatively new. We need to understand:
- Protocol specification details
- Required vs optional features
- Client compatibility (Claude Code, other tools)

**Investigation Tasks:**
- [ ] Study MCP specification thoroughly
- [ ] Identify minimal required messages
- [ ] Test with Claude Code as client
- [ ] Understand transport options (stdio, socket, HTTP)

**Success Criteria:**
- Clear understanding of protocol requirements
- Know which features to implement for v1
- Validated with at least one MCP client

**Outputs:**
- MCP implementation guide for Tome
- Minimal message set documentation
- Test client for development

---

### SPIKE-009: Apple Silicon Performance Characteristics

**Question:** Are there Apple Silicon-specific optimizations we should leverage?

**Time Budget:** 1 day

**Background:**
Targeting M1+ only. Potential optimizations:
- Unified memory architecture
- Hardware-accelerated text rendering
- Efficient core scheduling

**Investigation Tasks:**
- [ ] Profile Tauri app on M1/M2/M3
- [ ] Identify any ARM64-specific Rust optimizations
- [ ] Test memory access patterns
- [ ] Review Apple's optimization guides

**Success Criteria:**
- Understand M-chip specific opportunities
- No performance regressions vs Intel baseline
- Document any required build flags

**Outputs:**
- Apple Silicon optimization checklist
- Build configuration recommendations

---

## Spike Tracking

| ID | Title | Priority | Status | Assignee | Due |
|----|-------|----------|--------|----------|-----|
| SPIKE-001 | Tauri + Swift Integration | P0 | Not Started | - | Before P1 |
| SPIKE-002 | WKWebView Bridge Perf | P0 | Not Started | - | Before P1 |
| SPIKE-003 | Tantivy Memory at Scale | P0 | Not Started | - | Before P1 |
| SPIKE-004 | CloudKit Sync | P1 | Not Started | - | During P1 |
| SPIKE-005 | mandoc Output | P1 | Not Started | - | During P1 |
| SPIKE-006 | Sphinx searchindex.js | P1 | Not Started | - | During P1 |
| SPIKE-007 | rustdoc search-index.js | P2 | Not Started | - | Before P2 |
| SPIKE-008 | MCP Protocol | P2 | Not Started | - | Before P4 |
| SPIKE-009 | Apple Silicon Perf | P2 | Not Started | - | During P1 |

---

## Spike Template

When adding new spikes, use this template:

```markdown
### SPIKE-XXX: [Title]

**Question:** [Single clear question to answer]

**Time Budget:** [N days]

**Background:**
[Why this is uncertain and why it matters]

**Investigation Tasks:**
- [ ] Task 1
- [ ] Task 2

**Success Criteria:**
- Criterion 1
- Criterion 2

**Outputs:**
- Output 1
- Output 2

**Fallback:**
[What to do if spike reveals problems]
```
