# Technical Spikes

**Purpose:** De-risk unknowns through focused research and prototyping before committing to implementation.

> **The spikes were written but never run, and the plan was written as if they had passed.**
> Every P0 spike is "Not Started", yet 90 tickets are specified against the architecture those
> spikes exist to validate — SPIKE-001 asks whether the shell architecture is even feasible while
> 23 Phase-1 tickets assume it is. Spikes that do not gate anything are documentation, not
> risk reduction. They now gate the phases they inform; see the
> [phase gate](./00-project-overview.md#phase-gate-before-any-implementation).
>
> A spike that has no owner and no date does not happen. The tracking table at the bottom has
> owner and due columns for exactly this reason, and they are currently empty — which is itself
> the finding.

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

### SPIKE-004: iCloud Drive container behaviour

**Question:** Does an iCloud Drive ubiquity container behave well enough, from a non-Swift process,
to carry bookmark sync?

**Time Budget:** 3 days

**Background:**
Phase 3 now syncs via a file-based ubiquity container rather than CloudKit (see
[PRD § iCloud Sync Architecture](../PRD.md#icloud-sync-architecture)). That trades one set of
unknowns for another, and the new ones must be measured before P3-010 commits:

- **Eviction.** iCloud can replace a local file with a placeholder. Reading it without requesting
  download yields an empty or missing file — the most likely cause of silent data "loss".
- **Propagation latency**, and whether it is bounded at all.
- **Behaviour from a process that is not the app** — the CLI must reach the same container.
- Whether per-device subdirectories genuinely avoid conflict copies.
- Quota, throttling, and what happens when the user's iCloud is full.

**Investigation Tasks:**
- [ ] Provision a container; write and read from Rust (no Swift) on two Macs
- [ ] Force eviction and confirm the download-then-read path works
- [ ] Measure propagation latency, median and worst case, over a day
- [ ] Concurrently append from two devices; confirm no `.icloud` conflict copies appear
- [ ] Fill the account to quota and observe failure modes
- [ ] Reach the container from a non-sandboxed CLI process
- [ ] Kill the process mid-append; confirm the log reader tolerates a truncated final line

**Success Criteria:**
- Reliable read-after-download from Rust, on both devices
- Propagation typically < 60 s, with a defined behaviour when it is not
- Zero conflict copies across 100 concurrent-write trials
- Every failure mode produces a distinguishable, reportable error

**Outputs:**
- Container behaviour document with measured latencies
- Confirmed or revised sync design for P3-010
- Fault-injection scenarios for Phase 3 tests

**Fallback:**
If the container proves unreliable:
- Reconsider CloudKit, accepting the Swift boundary
- Ship v1.0 with **no sync** (see DEC-004; this is the recommended cut anyway)
- Optional self-hosted sync server, post-v1

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

**Priority raised P2 → P0.** The original plan scheduled this "before P4" and then specified the
MCP server against a **transport that does not exist in the protocol** (a Unix socket). That is
precisely the error a spike prevents, and scheduling the spike after the design was written is why
it did not. Agent access is one of the two genuinely differentiated features; validate it before
building around it.

**Question:** What's the minimal viable MCP server implementation for Tome's use case, and does
Claude Code actually connect to it?

**Time Budget:** 2 days

**Background:**
MCP (Model Context Protocol) is relatively new. We need to understand:
- Protocol specification details
- Required vs optional features
- Client compatibility (Claude Code, other tools)

**Investigation Tasks:**
- [ ] Read the **current** specification revision; record which revision, since it moves
- [ ] Identify the minimal required message set (`initialize`, `notifications/initialized`,
      `tools/list`, `tools/call`)
- [ ] **Build a hello-world stdio server and connect Claude Code to it.** This is the whole point.
- [ ] Confirm the supported transports and discard anything not in the spec
- [ ] Test version negotiation against a client requesting a version we do not list
- [ ] Verify what happens when the server writes to stdout by mistake
- [ ] Measure tool-result size limits in practice, and how a client behaves when a result is huge

**Success Criteria:**
- A trivial server is reachable from Claude Code end-to-end
- Known-correct minimal message set for v1
- Documented protocol revision(s) to support, with negotiation behaviour
- Confirmed stdout discipline requirement

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

### SPIKE-010: Documentation scraping — legal and ToS posture

**Priority:** P0 (before Phase 1)
**Time Budget:** 1 day (plus a lawyer's opinion if the answer is not obviously fine)

**Question:** Under what conditions can Tome fetch, cache, transform, and display third-party
documentation, and what must it *not* do?

**Background:**
The plan had no legal analysis at all — not in the risk register, not in the security document, not
in the PRD. This is the largest non-technical risk in the project and it was invisible. Tome's
entire value proposition is fetching other people's copyrighted content, reformatting it, and
storing it. There is a defensible position here (a local cache, for one user, that they could have
made with a browser), but the boundaries need to be understood *before* they shape the product,
not after someone objects.

Specific questions:
- Personal-use caching of copyrighted docs: fine. Where does that stop?
- Does bundling a **registry of scraper configurations** (not content) create exposure? (Almost
  certainly far less than shipping content — which is exactly why the registry ships configs.)
- Do the major hosts' terms — ReadTheDocs, GitBook, docs.rs — permit automated fetching, and at
  what rate?
- What must be preserved for attribution: origin link, licence, author?
- Is `robots.txt` compliance sufficient, and what about ToS that forbid scraping outright?
- What is the takedown path if a documentation owner objects to a registry entry?

**Investigation Tasks:**
- [ ] Read the ToS and `robots.txt` of the ten most likely sources
- [ ] Check how Dash, DevDocs, and Zeal handle attribution and permission — they have all solved
      this in public and are the cheapest available precedent
- [ ] Draft the attribution rules the reader will enforce
- [ ] Draft a registry takedown policy
- [ ] Decide whether any host needs an explicit opt-out list shipped with the app

**Success Criteria:**
- A written position on what Tome does and does not do, in the README, in plain language
- Attribution requirements specified concretely enough to implement
- A takedown process that exists before it is needed

**Outputs:**
- Legal posture section for the README and the registry contribution guide
- Input to RISK-011

**Fallback:**
If a major source forbids automated access: exclude it from the registry, keep manual
configuration available (the user's own choice, on their own machine), and say so plainly.

---

### SPIKE-011: Sanitizer versus real documentation

**Priority:** P1 (during Phase 1)
**Time Budget:** 1 day

**Question:** Does the HTML sanitizer allowlist preserve everything documentation actually needs?

**Background:**
The allowlist in `12-security-considerations.md` stripped attributes and elements that
documentation depends on — most damagingly the `id` attribute, without which **every heading
anchor, every TOC deep link, and every `#fragment` cross-reference silently stops working**. That
is a security control breaking a headline feature, and it would have been found late, in manual
testing, and misdiagnosed as a TOC bug.

**Investigation Tasks:**
- [ ] Run the proposed allowlist over 50 diverse real pages (Sphinx, rustdoc, mdBook, MkDocs, man)
- [ ] Diff before/after: what was removed, and did it matter?
- [ ] Verify anchors, footnotes, tables, admonitions, definition lists, and math survive
- [ ] Verify an XSS corpus does *not* survive
- [ ] Check the interaction with syntax highlighting (highlighter output is itself HTML that must
      pass the allowlist)

**Success Criteria:**
- Zero anchors broken across the corpus
- Zero XSS payloads surviving
- A documented allowlist justified element by element

**Outputs:**
- Final allowlist for `12-security-considerations.md`
- Regression corpus reused as a permanent test fixture

---

## Spike Tracking

| ID | Title | Priority | Status | Assignee | Due |
|----|-------|----------|--------|----------|-----|
| SPIKE-001 | Tauri + Swift Integration | P0 | Not Started | - | Before P1 |
| SPIKE-002 | WKWebView Bridge Perf | P0 | Not Started | - | Before P1 |
| SPIKE-003 | Tantivy Memory at Scale | P0 | Not Started | - | Before P1 |
| SPIKE-004 | iCloud Drive container | P1 | Not Started | - | Before P3 |
| SPIKE-005 | mandoc Output | P1 | Not Started | - | During P1 |
| SPIKE-006 | Sphinx searchindex.js | P1 | Not Started | - | During P1 |
| SPIKE-007 | rustdoc search-index.js | P2 | Not Started | - | Before P2 |
| SPIKE-008 | MCP Protocol | **P0** | Not Started | - | Before P1 |
| SPIKE-009 | Apple Silicon Perf | P2 | Not Started | - | Before P5 |

> **Every row has an empty owner.** That is the most important thing in this table. Assign owners
> and dates, or accept that the spikes will not run and that the plan is being built on
> assumptions. SPIKE-009 was previously marked P2 ("before the relevant phase") but due "During
> P1", which contradicts its own priority definition — it informs Phase 5 performance work, so it
> is due before Phase 5.
| SPIKE-010 | Doc scraping: legal & ToS posture | **P0** | Not Started | - | Before P1 |
| SPIKE-011 | Sanitizer vs. real documentation | P1 | Not Started | - | During P1 |

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
