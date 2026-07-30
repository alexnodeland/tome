# Tome: Product Requirements Document

**Version:** 1.1  
**Date:** 2026-07-28  
**Author:** Alex  
**Status:** Draft — not yet started. No code exists; nothing here has been validated by a spike.

> **Read this first.** This PRD describes an intended product, not a built one. Several
> foundational choices are still open — see [Open Decisions](./decisions/README.md) — and the
> effort estimates require a team this project does not currently have. See
> [the plan review](./reviews/2026-07-28-plan-review.md) for a full assessment.

-----

## Executive Summary

Tome is a native macOS application for developers who want unified, offline access to technical documentation. It aggregates documentation from diverse sources—ReadTheDocs, rustdoc, man pages, GitBook, and more—into a single, beautifully typeset reading experience with powerful search, bookmarks, and sync via iCloud.

Tome's intended differentiation is the combination of: a local-first architecture, typography-forward reading experience, an ingestion pipeline the user can point at *any* documentation site, and first-class programmatic access for AI agents via an MCP server and a Claude Code plugin. Each of these exists individually elsewhere; the bet is that the combination is worth switching for.

**This bet is unvalidated.** No user research has been done. Before Phase 1 begins, run the
validation step in [Product Validation](#product-validation).

-----

## Problem Statement

Developers constantly context-switch between documentation sources:

- Browser tabs for ReadTheDocs, GitBook, package docs
- Terminal for `man` pages
- IDE hover docs (incomplete)
- PDF references
- Local project wikis

Each source has different styling, search behavior, and navigation. Documentation is ephemeral—close the tab, lose your place. No unified bookmarks, no annotations, no offline guarantee.

**Current alternatives — honest assessment:**

|Solution         |What it does well                                                        |Where Tome could win                                                      |
|-----------------|-------------------------------------------------------------------------|---------------------------------------------------------------------------|
|Dash             |200+ curated, ready-to-install docsets; mature; fast; native macOS; one-time purchase|Arbitrary-site ingestion, typography, open source, MCP/agent access|
|DevDocs          |Open source; 500+ maintained docsets; works offline via service worker; free|Native app, man pages, private/internal docs, agent access                |
|Zeal             |Open source; cross-platform (Linux/Windows/macOS); reuses Dash docsets     |Reading experience, arbitrary-site ingestion, agent access                 |
|Browser bookmarks|Zero setup; always current                                                |Offline, unified search, no tab chaos                                      |

> **Correcting the original framing.** Earlier drafts of this table claimed DevDocs has "no
> offline" (it does, via service worker), that Dash is "subscription" (it is a one-time purchase
> with an optional subscription), and that Zeal has "limited platform support" (it supports more
> platforms than Tome will). Building against inaccurate competitor claims produces a product
> that loses on the axes that actually matter. The real gaps are: **nobody lets you point a tool
> at an arbitrary docs site and get a first-class reading experience**, and **nobody exposes a
> local docs library to coding agents**. Those are the two things worth building.

**The hardest competitive problem is cold start.** Dash's and DevDocs' actual moat is the curated
catalog: a new user installs and immediately has the docs they need. Tome as specified asks every
user to hand-write YAML before the app does anything. A curated source registry is therefore a
**v1.0 requirement**, not a nice-to-have — see [Source Registry](#source-registry).

-----

## Target Users

**Primary:** Professional developers who work across multiple languages and frameworks, value offline capability (travel, focus time), and appreciate thoughtful design.

**Secondary:** Technical writers maintaining documentation, students learning new technologies, open-source maintainers tracking ecosystem docs.

**Anti-persona:** Casual users who need documentation once a month and are fine with Google.

> **Unvalidated.** These personas are assumptions. No interviews, surveys, or usage data support
> them. See [Product Validation](#product-validation).

### Product Validation

Before committing 70+ person-weeks (see [Effort and Schedule](#effort-and-schedule)), spend one
week testing the premise:

1. **Ten interviews** with the primary persona. Ask what they do today, what they've tried, and
   what made them stop. Specifically probe: do Dash/DevDocs users feel the gaps above?
2. **Concierge test.** Manually build a docset for three willing developers from a site Dash
   doesn't cover. Do they use it a week later?
3. **Agent-access smoke test.** Wire an MCP server over a hand-built index for one project and
   see whether docs-in-context measurably helps.

**Kill/continue criteria:** if fewer than 3 of 10 interviewees can name a specific doc source
they cannot get into Dash or DevDocs today, the core premise is weak — narrow the product to the
agent-access angle rather than building a full reader.

-----

## Product Vision

> Tome is the personal library for the working programmer—a quiet, beautiful space where all technical knowledge lives, always available, deeply searchable, and entirely yours.

### Design Principles

1. **Local-first**: Your docs, your machine, your control. Cloud is optional sync, not a dependency.
2. **Typography matters**: Reading documentation should feel like reading a well-set book, not a raw HTML dump.
3. **Invisible complexity**: Platform-specific scrapers, format normalization, index building—all hidden. Users see unified docs.
4. **Extensible by default**: First-party scrapers for common platforms, but anyone can add sources via config or the Claude Code plugin.
5. **Programmable**: Local API and MCP server enable integration with editors, scripts, and AI agents.
6. **Good citizen**: Tome fetches other people's documentation. It obeys `robots.txt`, rate-limits
   itself, identifies itself honestly, caches for the user only, and never republishes.

### Non-Goals (v1.0)

Stating these explicitly so they can be pointed at when scope pressure arrives (see RISK-009):

- **Not a docs hosting or publishing tool.** Content is fetched for the local user and never
  redistributed.
- **Not a general web clipper / read-later app.** The ingestion pipeline is tuned for structured
  technical documentation, not arbitrary articles.
- **Not cross-platform in v1.** macOS on Apple Silicon only. This is a deliberate narrowing, and
  it is a competitive *disadvantage* against Zeal and DevDocs — accepted to keep the surface small.
- **Not multi-user, not team-shared, not enterprise.** One person, their machines, their Apple ID.
- **No AI features inside the app.** Tome exposes docs *to* agents; it does not summarize,
  generate, or chat. (`enrich.tldr_summaries` in Appendix A is post-v1 and unfunded.)
- **No semantic/vector search in v1.** Full-text only. (Roadmap v1.3.)

-----

## Core Features

### 1. Documentation Library

The primary interface is a library view showing all ingested documentation sources organized by category, with search, filtering, and customizable organization.

**Capabilities:**

- Grid or list view of doc sources
- Categories: Language (Rust, Python, JS…), Framework, Tool, Project, Custom
- Source metadata: version, last synced, size, entry count
- Quick actions: sync, remove, open, configure

### 2. Platform-Aware Ingestion

Tome ships with scrapers for common documentation platforms that understand their structure:

|Platform       |Detection                                   |Extraction Strategy                                          |Priority|
|---------------|--------------------------------------------|-------------------------------------------------------------|--------|
|**ReadTheDocs**|`readthedocs.io` subdomain, Sphinx markers  |Parse `searchindex.js` for doc tree, semantic HTML extraction|v1.0    |
|**rustdoc**    |`doc.rust-lang.org` style, `search-index.js`|Parse search index, module/type/function structure           |v1.0    |
|**mdBook**     |`book.toml`, SUMMARY.md structure           |Parse SUMMARY.md hierarchy, chapter extraction               |v1.0    |
|**Man pages**  |Local `man -w` paths                        |`mandoc -T html` rendering, section indexing                 |v1.0    |
|**Generic**    |Fallback for unknown sites                  |Configurable BFS crawl with selectors                        |v1.0    |
|**GitBook**    |`gitbook.io` or theme markers               |Navigation JSON parsing                                      |v1.1    |
|**Docusaurus** |Meta tags, sidebars structure               |Parse sidebars.js manifest                                   |v1.1    |
|**MkDocs**     |`mkdocs.yml`, Material theme                |Index parsing similar to Sphinx                              |v1.1    |
|**Docsets**    |`.docset` bundle format                     |Dash-compatible import                                       |v1.2    |

**Generic Scraper Configuration:**

```yaml
source:
  type: generic
  url: "https://example.com/docs/"
  generic:
    entry_points: ["/docs/", "/api/"]
    max_depth: 4
    include_patterns: ["^/docs/", "^/api/"]
    exclude_patterns: ["/changelog", "/_static/", "/search"]
    content_selector: "main.content, article, .documentation"
    title_selector: "h1, .page-title"
    nav_selector: "nav.sidebar, .toc"
```

**Crawl etiquette (non-negotiable defaults).** Tome fetches documentation it does not own. The
following are defaults that a user may loosen only for hosts they control:

| Rule | Default | Override |
|------|---------|----------|
| `robots.txt` | Obeyed, including `Crawl-delay` | Only for `localhost` and user-declared owned hosts |
| Rate limit | 2 req/s per host, single connection | Configurable down; capped at 4 |
| User-Agent | `Tome/<version> (+https://<project-url>)` — identifies the tool | Not overridable |
| Conditional fetch | `If-None-Match` / `If-Modified-Since` on every re-sync | Not overridable |
| Backoff | Honour `Retry-After`; exponential backoff on 429/5xx | Not overridable |
| Scope | Same registrable domain as the entry point | Explicit opt-in per host |

Earlier drafts listed `robots.txt` compliance as "optional, configurable" (P1-008). It is not
optional: it is the difference between a tool people can recommend and one that gets IP-banned
from ReadTheDocs. See RISK-011 in the risk register.

**Asset handling (required for the offline guarantee).** Documentation contains images, diagrams,
and SVGs. The pipeline as originally specified fetched only HTML, which means "works offline" was
false for any page with a figure, and the reader would silently reach out to the network — leaking
the user's reading activity to third-party hosts and contradicting the content-security policy.

Ingestion therefore includes an asset pass:

1. After normalization, collect every `src`/`srcset`/`poster` and CSS `url()` reference.
2. Fetch each asset under the same rate limit and etiquette rules; cap per-source asset bytes
   (default 250 MB) and per-asset size (default 10 MB).
3. Content-address assets on disk (`data/<source>/assets/<sha256>.<ext>`) so they deduplicate
   across pages and across sources.
4. Rewrite references to the local path. Assets that fail to fetch are replaced with an inline
   placeholder that records the original URL, not left as a live remote reference.
5. Assets are never fetched at render time. The reader's CSP forbids it.

### 3. Unified Reader

All documentation renders through a normalization pipeline producing consistent, beautiful output.

**Reader Features:**

- Consistent typography (see Design section)
- Syntax highlighting for code blocks (language-aware)
- Internal link resolution (cross-reference within doc set)
- External link handling (open in browser)
- Table of contents sidebar (extracted from page structure)
- Breadcrumb navigation
- Keyboard navigation (see [Appendix C](#appendix-c-keyboard-shortcut-reference))
- Reading position memory (resume where you left off)

**Rendering Pipeline:**

```
Ingest time (once per sync):
  Raw HTML/Markdown → Parse to AST → Normalize structure → Sanitize →
  Localize assets → Syntax highlight → Store normalized HTML + TOC

Read time (every page view):
  Load normalized HTML → Wrap in typography shell + CSP → Render to WebView
```

Splitting these matters: sanitizing and highlighting at ingest time means page views are a file
read plus a template, which is what makes the <100 ms render target reachable. It also means
untrusted HTML is neutralised once, on the way in, rather than trusted on every view.

**Sanitization is part of the contract, not an afterthought.** Fetched documentation is untrusted
input. It is sanitized to an allowlist at ingest (see `12-security-considerations.md`), which must
*preserve* heading `id` attributes, anchors, `img`, `figure`, `sup`/`sub`, and definition lists —
otherwise in-page anchors, the TOC sidebar, and footnotes all break.

### 4. Search

Full-text search across all documentation with scoping and filtering.

**Search Capabilities:**

- Global search (all sources) or scoped (single source/category)
- Fuzzy matching with typo tolerance
- Symbol-aware search (recognizes `fn`, `class`, `def` patterns)
- Recent searches history
- Search within current page (Cmd+F)
- Keyboard-first (Cmd+K opens global search)

**Implementation:** Tantivy (Rust) for full-text indexing, with optional future extension to vector/semantic search.

**Index Structure:**

- Title (boosted weight)
- Headers (boosted weight)
- Body text
- Code blocks (separate field, symbol extraction)
- Source metadata (for filtering)

### 5. Bookmarks & Annotations

Personal layer on top of documentation.

**Features:**

- Bookmark any page (keyboard: Cmd+D)
- Organize bookmarks into collections
- Highlight text passages (see anchoring below)
- Add notes to highlights
- Export annotations (Markdown, JSON)

**Sync:** Bookmarks and annotations sync via iCloud, not the doc content itself.

**Annotation anchoring.** Highlights cannot be stored as bare character offsets. Documentation is
re-fetched on a schedule; a single upstream edit above a highlight shifts every offset below it,
so highlights silently drift onto the wrong text — worse than losing them, because the user does
not notice. Tome uses the W3C Web Annotation selector model:

| Selector | Role |
|----------|------|
| `TextQuoteSelector` | The exact highlighted text plus ~32 chars of prefix and suffix. Primary anchor. |
| `TextPositionSelector` | Start/end offsets. A *hint* that makes re-anchoring fast, never the source of truth. |
| `RangeSelector` | Normalized-DOM path. Tiebreaker when the same quote appears more than once. |

On sync, if a page's `content_hash` changes, every annotation on that page is re-anchored:

1. Try the position hint; if the text there still matches the quote, done.
2. Otherwise search the new content for the quote plus its prefix/suffix.
3. If still not found, fall back to fuzzy match above a similarity threshold, and mark the
   annotation `approximate` so the UI can show it as such.
4. If nothing matches, mark it `orphaned` — never delete it. Orphaned annotations keep their note
   and their quoted text, and are listed in a "needs attention" view so the user can re-place or
   discard them.

The same rule applies to reading positions: store a scroll *percentage* plus the id of the nearest
preceding heading, and restore by heading first, percentage second.

### 6. Sync Strategy Configuration

Each documentation source has independent sync settings.

**Strategies:**

|Strategy   |Behavior                                                     |
|-----------|-------------------------------------------------------------|
|`manual`   |Only sync when explicitly triggered                          |
|`on_launch`|Check for updates when Tome launches                         |
|`scheduled`|Sync on schedule (daily, weekly, monthly)                    |
|`watch`    |Monitor package registry for new versions, sync when detected|

**Version Pinning:** Option to lock a doc source to a specific version, ignoring updates. Useful for projects locked to older library versions.

**Configuration:**

```yaml
sync:
  strategy: scheduled
  schedule: weekly
  pin_version: false
  watch_source: "crates:serde"  # for watch strategy
```

**`watch` polls third-party registries, so it needs the same etiquette as scraping.** The original
design ran a 60-second scheduler tick that checked every watched package's registry — for a user
with 30 watched crates that is 43 000 requests a day to crates.io for information that changes
weekly. It also contradicts the non-functional requirement "no background network activity without
user action".

Rules:

- Registry checks are **at most daily per source**, jittered, and never on a fixed-interval tick.
- Use each registry's cheap endpoint and conditional requests (`If-None-Match`); crates.io in
  particular requires a `User-Agent` identifying the client with contact information.
- The scheduler tick decides *what is due*; it does not itself make network calls.
- `watch` detects a new version and, per DEC-006, **notifies rather than fetching** by default.
  "Detects updates < 1 hour after publish" was a target that only aggressive polling could hit,
  and it is not a user-visible benefit worth the traffic.
- If the app has been closed, missed schedules are coalesced into one check at next launch, not
  replayed.

### 7. Man Page Integration

First-class support for Unix manual pages.

**Features:**

- Index all installed man pages (configurable paths)
- Render via `mandoc -T html` with Tome styling
- Section-aware browsing (1-8)
- Cross-reference linking (when man page references another)
- Optional: tldr-style summary display alongside full man page

**Configuration:**

```yaml
source:
  type: man
  paths:
    - /usr/share/man
    - /usr/local/share/man
    - /opt/homebrew/share/man
  sections: [1, 2, 3, 5, 7, 8]  # which sections to index
```

**Rendering man pages shells out to `mandoc`.** Two consequences the plan did not record:

- `mandoc` is invoked as a subprocess on paths the user controls. Arguments must be passed as an
  argument vector (never a shell string), paths canonicalized, and the process run with a timeout
  and output size cap — a pathological roff file can make a formatter loop.
- macOS ships `mandoc`; Homebrew may shadow it with a different version whose HTML output differs.
  Resolve the binary explicitly rather than trusting `$PATH`, and record which one produced a page
  so output changes are explainable. SPIKE-005 must test both.

### 8. Local API

HTTP API for programmatic access, enabling editor integrations and scripts.

**Endpoints:**

```
GET  /api/search?q={query}&scope={source}&limit={n}
     Search documentation, returns ranked results

GET  /api/sources
     List all documentation sources with metadata

GET  /api/sources/{id}
     Get specific source details

POST /api/sources
     Add new documentation source (accepts config YAML/JSON)

DELETE /api/sources/{id}
     Remove documentation source

GET  /api/sources/{id}/pages
     List all pages in a source

GET  /api/sources/{id}/pages/{path}
     Get rendered page content

POST /api/sources/{id}/sync
     Trigger sync for a source

GET  /api/bookmarks
     List all bookmarks

POST /api/bookmarks
     Create bookmark

GET  /api/status
     Health check, sync status, index stats
```

**Server is off by default.** It starts only when the user enables it in Preferences or runs
`tome serve`.

**Authentication: a bearer token is always required — including on localhost.**

This is the single most important security decision in the product, and the original design got
it backwards. "Bound to 127.0.0.1, therefore trusted" is false on a desktop machine:

- **Any web page the user visits can reach it.** A browser tab can `fetch()` `http://localhost:7431`.
  With permissive CORS it can also *read* the response. That means any site could enumerate the
  user's library, read their bookmarks, and — via `POST /api/sources` — make Tome fetch arbitrary
  URLs (SSRF into the user's private network) or index arbitrary local paths.
- **Every other process on the machine can reach it**, including anything the user pip-installed.

Controls:

| Control | Behaviour |
|---------|-----------|
| Bind address | `127.0.0.1` only. Binding elsewhere requires an explicit config flag and prints a warning. |
| Token | Generated on first run, stored in the macOS Keychain, sent as `Authorization: Bearer …`. Required for every request including loopback. Retrievable via `tome status --show-token`. |
| CORS | **No CORS headers by default.** Browsers cannot read responses cross-origin. An explicit origin allowlist exists for users who want a browser extension; `*` is not an accepted value. |
| Origin/Host checks | Requests carrying an `Origin` not on the allowlist are rejected. `Host` must be `localhost`/`127.0.0.1` — this blocks DNS-rebinding attacks that would otherwise defeat the bind address. |
| Mutating endpoints | `POST`/`DELETE` additionally require the token to be sent, and `POST /sources` validates URLs through the SSRF filter before any fetch. |
| Rate limit | Per-token, to bound damage from a leaked token. |

`GET /api/status` is the only unauthenticated endpoint, and it returns nothing but
`{"status":"ok","version":"…"}` so that clients can detect the server without holding a token.

### 9. MCP Server

Model Context Protocol server enabling AI agents to query documentation.

**Tools Exposed:**

```typescript
// Search across all documentation
tome_search(query: string, scope?: string, limit?: number): SearchResult[]

// Get specific page content
tome_get_page(source_id: string, page_path: string): PageContent

// List available sources
tome_list_sources(): Source[]

// Get source table of contents
tome_get_toc(source_id: string): TableOfContents

// Add bookmark (with optional note)
tome_bookmark(source_id: string, page_path: string, note?: string): Bookmark

// Lookup symbol (function, type, module)
tome_lookup_symbol(symbol: string, language?: string): SymbolResult[]
```

**Use Cases:**

- Claude Code queries relevant docs while helping with code
- Agentic workflows that need API references
- Custom tooling that surfaces docs contextually

**Transport: stdio by default.**

The original design specified a Unix domain socket at `~/.tome/mcp.sock`. **A raw Unix socket is
not an MCP transport.** MCP defines stdio and Streamable HTTP; no MCP client — including Claude
Code, the headline integration — can connect to a bare socket. As specified, the flagship feature
would not have worked with the flagship client.

| Transport | Status | Use |
|-----------|--------|-----|
| **stdio** | Default | The client launches `tome mcp`; JSON-RPC over stdin/stdout. This is how Claude Code and most clients connect. |
| **Streamable HTTP** | Opt-in | `tome mcp --http --port 7432`, for clients that cannot spawn processes. Uses the same bearer token and the same Origin/Host validation as the local API — an MCP HTTP endpoint on localhost has exactly the same browser-reachability problem. |
| Unix socket | Removed | Not part of the protocol. |

Because `tome mcp` is spawned per-client over stdio, it must not assume exclusive ownership of the
index: the search index is opened read-only, and writes (e.g. `tome_bookmark`) go through the
database with normal locking.

**Client configuration** (what a user actually pastes into their MCP client):

```json
{
  "mcpServers": {
    "tome": { "command": "tome", "args": ["mcp"] }
  }
}
```

**Protocol version.** Negotiated at `initialize`. Tome accepts the client's requested version if
supported and otherwise responds with its own latest; the supported list is a build-time constant
kept current with the specification, not a hardcoded pair of dates. See
`14-api-versioning-strategy.md`.

**Agent-facing tool design.** `tome_get_page` can return a very large page. Tool results are
truncated to a configurable budget (default ~8k tokens' worth of text) with an explicit
`truncated: true` and a `section` parameter so an agent can request a subtree of the TOC instead of
the whole document. Returning a 200 KB page into an agent's context is a bug, not a feature.

### 10. Claude Code Plugin

Slash command integration for managing Tome from the terminal.

**Commands:**

```bash
/tome add <url>
# Detects platform, proposes config, adds source
# Example: /tome add https://docs.pola.rs/

/tome add <local-path>
# Configures local documentation directory
# Example: /tome add ~/projects/mylib/docs/

/tome search <query>
# Search and display results inline
# Example: /tome search "async iterator rust"

/tome pull [source]
# Fetch/update documentation, shows progress
# Example: /tome pull polars
# (named `pull` to match the CLI; `sync` refers to bookmark sync, which is automatic)

/tome list
# Show all configured sources with status

/tome remove <source>
# Remove a documentation source

/tome config <source>
# Open/edit source configuration
```

**Workflow Example:**

```
User: /tome add https://docs.pola.rs/

Claude Code:
  → Fetches URL, analyzes structure
  → Detects: MkDocs with Material theme
  → "This looks like Polars documentation (Python DataFrame library).
     I'll configure it with:
     - Weekly sync schedule
     - Category: Python
     Want me to proceed, or would you like to customize?"
  
User: proceed

Claude Code:
  → Writes ~/Library/Application Support/Tome/sources/polars.yaml
  → Invokes: tome pull polars
  → "Done! Polars docs are now available in Tome.
     Found 847 pages across 12 sections."
```

### 11. Source Registry

**New in v1.1 of this document, and a v1.0 requirement.** Without it, a new user's first experience
is a text editor and a YAML schema, and they will not have that experience twice.

A registry is a versioned index of ready-made source configurations — the same job Dash's docset
catalog and DevDocs' documentation list do, minus the hosting of content.

```yaml
# registry/index.yaml — served from the project's GitHub Pages, cached locally
version: 1
updated: 2026-07-28
sources:
  - id: rust-std
    name: Rust Standard Library
    category: Rust
    homepage: https://doc.rust-lang.org/std/
    licence: MIT OR Apache-2.0        # of the documentation, for attribution
    config: sources/rust-std.yaml     # the tested scraper config
    verified: 2026-07-20              # last date CI confirmed this config still works
```

**Properties that make this worth building:**

- **It contains configuration, never content.** Tome never hosts or redistributes anyone's
  documentation; the user's machine fetches it directly from the origin. This keeps the legal
  posture clean (see RISK-011).
- **It is CI-verified.** A scheduled job re-runs each registry config against the live site and
  opens an issue when one breaks. This turns RISK-003 (scraper maintenance burden) from an
  invisible slow decay into a tracked, actionable signal — the single highest-leverage mitigation
  available, because scraper rot is otherwise discovered by users.
- **It is community-extensible.** Adding a source is a PR containing one YAML file, which is the
  lowest-friction contribution path this project can offer and directly mitigates RISK-010
  (single maintainer).
- **It degrades gracefully.** The registry is a convenience; every source can still be added by
  hand, and Tome works fully offline once fetched.

**Built by S3-8**, in [`registry/`](../registry/README.md): four sources so far, all verified
2026-07-30. Its own README owns the contribution and verification procedure; the offline checks
(`cargo test -p tome-core --test registry`) run in the gate, and the live check
(`scripts/verify-registry.sh`) deliberately does not — a gate that fails when someone else's
website is down teaches everyone to ignore the gate.

The verification job earned its place on its first run: `nodejs.org/docs/` is `Disallow`ed by
that site's `robots.txt` while `nodejs.org/api/` is explicitly `Allow`ed, so the obvious URL was
the forbidden one. No review would have caught that; a fetch did.

**Target for v1.0: 30 verified sources** covering the languages and frameworks in the top of the
Stack Overflow survey. That is a realistic weekend of work per ten sources once the generic scraper
is solid, and it converts the onboarding screen from "here's a YAML schema" to "pick the docs you
want."

-----

## Technical Architecture

### Stack

> **Provisional until SPIKE-001 and SPIKE-002 complete.** 87 tickets are currently written against
> this stack, none of which has been validated. Do not begin Phase 1 implementation until the
> P0 spikes pass — see the gate in [Effort and Schedule](#effort-and-schedule).

The original table listed "Native Shell: Swift + AppKit", "UI: Svelte", "Doc Rendering: WKWebView",
and "IPC: Tauri" as four peer layers. They are not peers, and reading them as such produces an
incoherent design. **Tauri already is the native shell on macOS**: it owns the process, creates the
`NSWindow`, and hosts a `WKWebView`. There is no second shell to write, and the "Svelte UI" and
"WKWebView doc rendering" rows describe *the same* web context unless a second webview is
deliberately created.

|Layer|Technology|What it actually is|
|-----|----------|-------------------|
|**Application shell**|Tauri (Rust)|Owns the process, window, menus, lifecycle. This *is* the native shell.|
|**UI**|Svelte + TypeScript|Runs in Tauri's primary WKWebView. Library sidebar, TOC, search, settings.|
|**Reader surface**|Nested `<iframe>` inside the primary webview, `sandbox="allow-same-origin"`, own CSP|Isolates untrusted documentation HTML from the app UI. **Not** a second `WKWebView`.|
|**Core engine**|Rust|Scraping, parsing, normalization, indexing, sync. In-process with Tauri.|
|**IPC**|Tauri commands + events|The only Rust ↔ JS boundary.|
|**Search index**|Tantivy|Rust-native full-text search.|
|**Metadata store**|SQLite (via `sqlx`)|Sources, pages, bookmarks, sync state.|
|**Content store**|Filesystem|Normalized HTML, raw fetches, content-addressed assets.|
|**Native extras**|Swift/Objective-C via a small Tauri plugin — *only if SPIKE-001 justifies it*|`NSStatusItem` popover, global hotkey. Everything else stays in Rust.|
|**Sync**|iCloud Drive ubiquity container (file-based)|See [Sync Architecture](#icloud-sync-architecture). **Not** CloudKit in v1.|

**Why the reader is an iframe, not a second WKWebView.** Documentation is untrusted HTML. Rendering
it in the same document as the app UI means one sanitizer bypass compromises the whole app,
including the Tauri IPC bridge. An iframe with its own restrictive CSP and no `allow-scripts` gives
isolation without the cost of managing a second native webview and a second bridge. It also means
`Cmd+F`, scroll tracking, and highlight anchoring all work through one well-defined `postMessage`
channel rather than two IPC paths.

**Swift is a contingency, not a foundation.** Only two features need AppKit that Tauri does not
already cover: the menu-bar popover and the system-wide hotkey. Both are Phase 5, both are
non-critical, and both have pure-Tauri fallbacks. Do not let them shape the Phase 1 architecture.
If SPIKE-001 shows the Swift plugin is awkward, cut the popover to a plain menu-bar menu and use
Tauri's global-shortcut plugin — the cost is small and it is paid in Phase 5, not Phase 1.

### System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│              Tome.app  —  Tauri process (Rust)                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │              Primary WKWebView (owned by Tauri)            │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                   Svelte UI Layer                    │  │  │
│  │  │  Library • Search • Settings • Bookmarks • TOC       │  │  │
│  │  │  ┌───────────────────────────────────────────────┐  │  │  │
│  │  │  │  Reader <iframe>  — strict CSP, no scripts     │  │  │  │
│  │  │  │  untrusted normalized documentation HTML       │  │  │  │
│  │  │  └───────────────────────────────────────────────┘  │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │   Optional Swift plugin (Phase 5, only if SPIKE-001 OKs)  │  │
│  │        NSStatusItem popover  •  Global hotkey             │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    Rust Core Engine                        │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐  │  │
│  │  │  Ingestion  │ │   Search    │ │  Render Pipeline    │  │  │
│  │  │  ─────────  │ │  ────────   │ │  ───────────────    │  │  │
│  │  │  Scrapers   │ │  Tantivy    │ │  HTML → AST →       │  │  │
│  │  │  Parsers    │ │  Indexing   │ │  Normalize →        │  │  │
│  │  │  Extractors │ │  Query      │ │  Style → Render     │  │  │
│  │  └─────────────┘ └─────────────┘ └─────────────────────┘  │  │
│  │  ┌─────────────┐ ┌─────────────────────────────────────┐ │  │
│  │  │  Local API  │ │           Sync Manager              │ │  │
│  │  │  ─────────  │ │  ──────────────────────────────     │ │  │
│  │  │  HTTP/REST  │ │  iCloud Drive container (bookmarks) │ │  │
│  │  │  127.0.0.1  │ │  + per-source fetch scheduling      │ │  │
│  │  │  token auth │ │                                     │ │  │
│  │  │  off by dflt│ │                                     │ │  │
│  │  └─────────────┘ └─────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                      Storage Layer                         │  │
│  │  SQLite (metadata, bookmarks)  •  Filesystem (pages,      │  │
│  │  content-addressed assets)  •  Tantivy index              │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
        │                                          │
        │ shares ~/Library/Application Support/Tome │
        ▼                                          ▼
┌──────────────────┐                    ┌────────────────────────┐
│  tome  (CLI)     │                    │  iCloud Drive container │
│  ──────────────  │                    │  bookmarks / positions  │
│  add pull search │                    │  (not doc content)      │
│  serve │ mcp     │                    └────────────────────────┘
└────────┬─────────┘
         │ `tome mcp` — stdio, spawned per client
         ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ Claude Code  │  │   Scripts    │  │   Editors    │
│ (MCP + CLI)  │  │  (via API)   │  │  (via MCP)   │
└──────────────┘  └──────────────┘  └──────────────┘
```

**Note the process boundary.** The MCP server and the CLI are *not* inside the app process — they
are the `tome` binary, spawned independently, reading the same on-disk state. This is why the data
directory must be reachable by both a GUI app and a Homebrew-installed CLI, and why App Sandbox is
incompatible with the design as drawn (see [File System Layout](#file-system-layout)).

### Data Model

**Source:**

```rust
// As frozen by S1-1 (crates/tome-core/src/model/), with one correction to
// this sketch: `id` is a validated slug (`SourceId`), NOT a Uuid. The slug
// is the config file name, the on-disk directory name, what the CLI takes,
// and what the registry ships — and ADR-0001's bookmark sync needs the SAME
// source added on two devices to be the SAME identity, which per-device
// UUIDs would break. tome-core's `model` module is authoritative for the
// exact shapes; this sketch shows intent.
struct Source {
    id: SourceId,       // validated slug, e.g. "python" — see correction above
    name: String,
    source_type: SourceType,  // ReadTheDocs, Rustdoc, Man, Generic...
    url: Option<String>,
    local_path: Option<PathBuf>,
    version: Option<String>,
    category: String,
    icon: Icon,
    accent_color: Option<String>,
    sync_config: SyncConfig,
    created_at: DateTime,
    last_synced: Option<DateTime>,
    page_count: u32,
    index_size_bytes: u64,
}
```

**Page:**

```rust
// Corrected by S1-1: a page's identity is (source, path) — the natural key
// this document's own Bookmark and Annotation types already reference — and
// there is no surrogate Uuid. `path` and `content_hash` are validated
// newtypes; the fetch validators (ETag / Last-Modified) live here so re-sync
// can be conditional.
struct Page {
    source: SourceId,
    path: PagePath,             // relative path within source, validated
    title: String,
    content_hash: ContentHash,  // SHA-256 of normalized content
    fetched_at: DateTime<Utc>,
    etag: Option<String>,
    last_modified: Option<String>,
}
```

**Bookmark:**

```rust
struct Bookmark {
    id: Uuid,
    source_id: Uuid,
    page_path: String,
    title: String,
    note: Option<String>,
    /// Many-to-many. A bookmark can live in several collections.
    collection_ids: Vec<Uuid>,
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
    /// Which device last wrote this. Metadata for conflict resolution —
    /// deliberately NOT part of the record's identity.
    last_writer: DeviceId,
}

/// Separate from Bookmark: you can annotate a page without bookmarking it,
/// and a bookmarked page can carry many annotations.
struct Annotation {
    id: Uuid,
    source_id: Uuid,
    page_path: String,
    /// Robust anchoring — see "Annotation anchoring" above.
    quote: String,          // exact highlighted text
    prefix: String,         // ~32 chars before
    suffix: String,         // ~32 chars after
    position_hint: Range<u32>,
    anchor_state: AnchorState, // Exact | Approximate | Orphaned
    color: HighlightColor,
    note: Option<String>,
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
    last_writer: DeviceId,
}

/// Scroll position is per (source, page) and is not a property of a bookmark.
struct ReadingPosition {
    source_id: Uuid,
    page_path: String,
    /// Restore by heading first, percentage second.
    nearest_heading_id: Option<String>,
    scroll_fraction: f32,
    updated_at: DateTime<Utc>,
}
```

Three changes from the original shape, each of which was a real defect:

- **`highlight_ranges: Vec<(u32,u32)>` is gone.** Bare offsets do not survive re-sync.
- **`collection_id: Option<Uuid>` became `collection_ids: Vec<Uuid>`.** The features list promises
  "add bookmark to multiple collections"; a single nullable id cannot express that, and the
  Phase 3 schema already contradicted it with a join table.
- **`synced: bool` is gone.** A boolean cannot distinguish "never synced", "pending", "in flight",
  "conflicted", and "sync disabled". Sync state lives in a dedicated table keyed by entity.

`device_id` must **not** appear in any uniqueness constraint. The Phase 3 schema originally had
`UNIQUE(source_id, page_path, device_id)`, which means each device creates its own row for the same
page and sync produces duplicates instead of converging — the exact opposite of what a sync key is
for.

### File System Layout

**Two constraints drive this, and the original `~/.tome` layout satisfied neither.**

1. **App Sandbox is incompatible with a shared `~/.tome`.** A sandboxed app cannot write `~/.tome`;
   the path is redirected into its container. The `tome` CLI installed by Homebrew is *not*
   sandboxed, so it would read a different directory than the app. The CLI, the MCP server, and the
   GUI would each see a different library — which breaks the entire integration story.
   **Resolution: Tome ships Developer ID–signed and notarized, hardened runtime on, App Sandbox
   off.** Sandboxing is only mandatory for Mac App Store distribution, and the plan distributes via
   DMG and Homebrew. `09-non-functional-requirements.md` and `12-security-considerations.md` have
   been corrected to match; the entitlements in Phase 5 are the authoritative set.

2. **macOS has conventions, and `~/.tome` is not one of them.** State belongs in Application
   Support; re-fetchable content belongs in Caches, so that macOS can evict it under disk pressure
   and so `brew uninstall --zap` has a correct list to remove.

```
~/Library/Application Support/Tome/      # state — back this up
├── config.yaml                          # global configuration
├── sources/                             # source configurations (YAML)
│   ├── rust-std.yaml
│   └── polars.yaml
├── tome.db                              # SQLite: sources, pages, bookmarks, annotations, sync state
└── logs/                                # rotated, 7-day retention

~/Library/Caches/Tome/                   # re-fetchable — safe to delete
├── data/
│   └── <source-id>/
│       ├── pages/                       # normalized, sanitized, highlighted HTML
│       ├── raw/                         # original fetched bytes (for re-normalization)
│       └── assets/                      # content-addressed: <sha256>.<ext>
└── index/                               # Tantivy index

~/Library/Mobile Documents/iCloud~<bundle-id>/Documents/   # synced, if enabled
└── devices/<device-id>/…                # see Sync Architecture
```

`$TOME_HOME` overrides the Application Support root for users who want everything in one place;
`~/.tome` is no longer a default but is accepted if `$TOME_HOME` points at it. There is no
`mcp.sock` — MCP is stdio (see [MCP Server](#9-mcp-server)).

Every path in the plan must come from one path-resolution module. The original docs referenced
`~/.tome/`, `~/Library/Application Support/Tome`, `~/Library/Caches/com.example.tome`, and
`dirs::data_dir()/tome/` in different places, and several code samples passed a literal `~` to
APIs that do not expand it — meaning they would have created a directory named `~` in the
working directory.

### iCloud Sync Architecture

**What syncs:**

- Source configurations (which docs you have, not the content)
- Bookmarks and annotations
- Collections/organization
- Reading positions
- User preferences

**What doesn't sync (too large, easily re-fetched):**

- Actual documentation content
- Search index
- Cached assets

**Sync mechanism: iCloud Drive ubiquity container, not CloudKit.**

The plan previously described *three* incompatible mechanisms — CloudKit `CKRecord`s in a custom
zone (Phase 3), a symlinked `~/.tome/icloud/*.json` (this document), and
`~/Library/Mobile Documents/…/*.json` (rollback plan). Only one can be built. v1 uses the
file-based container:

| | CloudKit | iCloud Drive container ✅ |
|---|---|---|
|Language|Swift/Obj-C only — the core is Rust|Plain file I/O, works from Rust|
|Requires paid Apple Developer Program|Yes|Yes (for notarization anyway)|
|Requires iCloud entitlements + container config|Yes|Yes, but simpler|
|Conflict model|Server-authoritative, change tokens, `CKError` taxonomy|Whatever we implement|
|Failure modes|Large, partly undocumented (RISK-002 scores this 16/Critical)|Small, ours|
|Works when the CLI runs outside the app|No|Yes|

This is not a new idea — it is the contingency the risk register already recorded for RISK-002
("Simpler sync: iCloud Drive file-based sync instead of CloudKit"). Given that the core is Rust and
that the CLI must sync too, the contingency is the better v1 primary. CloudKit moves to post-v1
and only if file-based sync proves inadequate.

**Design — append-only per-device logs, converge on read:**

```
Documents/
├── devices/
│   ├── <device-a-uuid>/
│   │   ├── manifest.json      # device name, last write, schema version
│   │   └── ops-<seq>.jsonl    # append-only operation log, one op per line
│   └── <device-b-uuid>/…
└── schema-version
```

- Each device only ever **writes its own directory**, so two devices can never write the same file.
  This eliminates the entire class of file-level iCloud conflicts, which is the main reason
  naive file sync fails.
- Each op is `{op, entity_type, entity_id, fields, lamport, wall_clock, device}`.
- On read, every device replays all logs into local SQLite. Convergence rules:
  - **Scalar fields** (title, note, colour): last-writer-wins by Lamport clock, wall clock as
    tiebreak, device id as final tiebreak so all devices pick the same winner deterministically.
  - **Set fields** (collection membership): add-wins set — an add always beats a concurrent
    remove, because losing a bookmark is worse than an extra one.
  - **Deletes**: tombstones, retained 90 days, never physical deletion during that window.
- Logs compact when they exceed a size threshold: the writing device replaces its own log with a
  snapshot of its current state. Only the owning device may compact its own log.
- **Offline is the default state, not a special case.** Ops are written locally and the container
  syncs whenever iCloud does; there is no separate offline queue to get out of step.

**Explicitly out of scope for v1:** real-time sync, sync of documentation content, sync between
different Apple IDs, and conflict *resolution UI*. Conflicts resolve automatically by the rules
above; the user sees a log, not a dialog.

-----

## Design Specification

### Visual Identity

**Philosophy:** A quiet, focused reading environment. The documentation is the star, not the chrome.

**Color Palette:**

|Element       |Light Mode            |Dark Mode              |
|--------------|----------------------|-----------------------|
|Background    |`#FAFAFA` (warm white)|`#1C1C1E` (system dark)|
|Surface       |`#FFFFFF`             |`#2C2C2E`              |
|Text Primary  |`#1D1D1F`             |`#F5F5F7`              |
|Text Secondary|`#6E6E73`             |`#98989D`              |
|Accent        |`#5856D6` (indigo)    |`#5E5CE6`              |
|Border        |`#E5E5EA`             |`#38383A`              |

### Typography

**Body Text:**

- Font: New York (system serif) with SF Pro fallback
- Size: 17px (matches Apple’s reading-optimized size)
- Line height: 1.6
- Measure: 70ch maximum
- Paragraph spacing: 1em

**Code:**

- Font: SF Mono or user-configured monospace
- Size: 15px (slightly smaller than body)
- Background: subtle contrast (`#F5F5F7` / `#2C2C2E`)
- Syntax highlighting: Subtle, muted palette (not rainbow)

**Headings:**

- Font: SF Pro Display (system sans)
- Weight: Semibold
- Scale: 1.25 ratio (h1: 2em, h2: 1.6em, h3: 1.25em)

### Layout

**Three-panel design:**

```
┌─────────┬──────────────────────────────────────┬─────────┐
│         │                                      │         │
│ Sources │           Reader Pane                │  Page   │
│  List   │                                      │   TOC   │
│         │                                      │         │
│  ────   │                                      │  ────   │
│         │                                      │         │
│ Library │     Beautifully rendered docs        │ Quick   │
│ Browser │                                      │  nav    │
│         │                                      │         │
└─────────┴──────────────────────────────────────┴─────────┘
     240px              flexible                    200px
```

**Responsive behavior:**

- Sidebars collapsible (keyboard: Cmd+1, Cmd+2)
- Reader expands to fill space
- Minimum window: 800x600

### Components

**Search (Cmd+K):**

- Centered modal overlay
- Large input field
- Live results as you type
- Keyboard navigation (↑↓ to select, Enter to open)
- Scope indicator (searching: All / Rust / Python…)

**Bookmark indicator:**

- Subtle filled bookmark icon in toolbar when page is bookmarked
- Click to add/remove
- Long-press to add note

**Sync status:**

- Subtle indicator in toolbar
- Animated during sync
- Click for detailed status

### Keyboard Shortcuts

**Canonical list: [Appendix C](#appendix-c-keyboard-shortcut-reference).** It is the single source
of truth; the design system, the NFR document, and the user documentation all reference it rather
than restating it. Four partially-contradictory copies previously existed, and they had already
drifted.

-----

## CLI Specification

The CLI enables scripting and serves as the backend for the Claude Code plugin.

```bash
tome [command] [options]

Commands:
  add <url>            Add documentation source (interactive; --yes, --name, --category, --insecure)
  pull [source]        Fetch/update documentation content   (--all, --all --due, --max-pages)
  list                 List all sources                     (--category)
  search <query>       Search documentation
  remove <source>      Remove a source                      (--yes)
  config [source]      View/edit configuration      (rotate-token, forget-token)
  registry             Browse/install from the source registry
  serve                Start local API server       (--port, --bind, --allow-origin; off unless run)
  mcp                  Start MCP server (stdio by default; --http for Streamable HTTP)
  status               Show sync and index status               (--show-token)
  export               Export bookmarks/annotations
  import <path>        Import previously exported bookmarks/annotations
  debug <subcommand>   Diagnostics and recovery (hidden from top-level --help)
                         check          report problems; never repairs
                         rebuild-index  rebuild the index from local content, no network
                         report         a redacted bundle to paste into a bug report

Global options:
  --json               Output as JSON (for scripting)
  --quiet              Suppress non-essential output
  --config <path>      Use alternate config file
  --version            Show version
  --help               Show help

Examples:
  tome add https://docs.python.org/3/
  tome pull --all
  tome search "async iterator" --scope rust-std
  tome list --json | jq '.sources[] | select(.category == "Rust")'
```

`tome add <url>` fetches the site's homepage through the ordinary fetch path (robots.txt, rate
limit, SSRF guard — detection is the first thing Tome does to a host a user names), classifies the
platform (P2-014), and proposes a config. A **confident** detection (≥ the auto-accept threshold)
selects that platform's scraper; anything less falls back to the generic scraper, which is correct
for every platform. Confirmation is interactive by default; `--yes` skips it and is **required**
with `--json` or when stdin is not a terminal — and that check runs before any network traffic.
`--insecure` permits http and private hosts (an intranet mirror you own) and is written into the
config as `fetch.allow_insecure`. The written YAML is round-tripped through the real config parser
before the initial pull, so `add` cannot write a file `pull` would reject. Local paths are refused
until local/docset ingestion exists.

`tome config rotate-token` replaces the API bearer token; `tome config forget-token` deletes it.
The second exists because of uninstall: `brew uninstall --zap` removes files, and the Keychain is
not a file, so without it the one secret Tome creates outlives the uninstall. The cask's caveats
name it.

**`tome debug` is diagnostics and recovery**, hidden from the top-level help because nothing there
belongs to an ordinary day.

- **`check`** reports and never repairs — a diagnostic that fixes things cannot be run twice to see
  whether the fix worked. It verifies that the library is writable, the database opens, every source
  configuration parses, the index opens, and **that the index and the database agree on every
  source's page count**. That last one is the failure with no symptom: an interrupted pull leaves the
  database ahead of the index and search quietly misses pages that are on disk. An empty library is
  reported as healthy, not as a fault, and `check` creates nothing. Exits non-zero when something is
  wrong, so `tome debug check && …` works.
- **`rebuild-index`** discards the index and rebuilds it from pages already on disk. **No network**:
  the index is derived and lives under the cache root, and SPIKE-003 measured a rebuild at 5–21 s per
  100 000 pages against about seven hours to re-crawl them. The database, the configurations and the
  cached content are untouched.
- **`report`** prints a redacted bundle: version, OS, library locations with `$HOME` rewritten to
  `~`, source ids with page counts and last-pull dates, the `check` findings, and the tail of the
  log. **No page paths, no search queries, no note text, no username.** There is no telemetry and
  there will not be, so this is the only path from a broken machine to something a maintainer can
  read — which means it has to be worth reading *and* safe to publish.

**Logs.** Every command writes to `~/Library/Application Support/Tome/logs/tome-<date>.log` as well
as to stderr, rotated daily with 7-day retention. The file is created on the first event, never at
startup, so a read-only command on a machine that has pulled nothing still creates nothing. Log
lines carry no reading history, by the same rule that governs error messages.

`tome remove <source>` deletes the source's search-index entries, database rows, cached content,
and — deliberately last, so a failed removal can be re-run — its config file. Confirmation is
interactive with a **default of No** (the opposite of `add`); `--yes` skips it, same rules as
above.

**The `--json` contract.** Success output is a single JSON document on stdout, one stable shape
per command (`{"sources": […]}`, `{"results": […], "suggestions": […]}`, `{"pulled": […]}`), with
every key always present even when empty. **Errors are `{"error": {"message": …}}` on stderr**,
stdout stays empty, and the exit code is non-zero — a script piping stdout never receives half a
result and then an error. Progress and prompts go to stderr always.

`tome search` corrects terms that appear nowhere in the index (P2-009) and **says so**, because a
search that quietly answers a different question than the one asked is worse than one that reports
what it did:

```
$ tome search "enviroment variables"
Did you mean: enviroment → environment

cargo                    Environment variables
                         cargo/reference/environment-variables.html
```

Prefixing a term with `@` searches **declared symbols only** (P2-015) — the pages that declare it,
rather than every page that mentions it:

```
$ tome search "@with_capacity"
rust-std                 Struct Vec  [type]
                         std/vec/struct.Vec.html
```

The `[type]` suffix is the kind of symbol the page documents — `function`, `type`, `trait`,
`module`, `constant`, `macro` — and is absent for a page that documents no single symbol.

Under `--json`, `results` entries carry `symbol_kind` (`null` for prose pages) and corrections are a
`suggestions` array of `{typed, meant}` objects. Both keys are **always present, even when empty**,
so `tome search --json | jq` needs no special case — the same rule `tome list --json` follows.

`--max-pages` caps a crawl at runtime, overriding whatever the config says (it only ever
tightens). It is for health checks — `scripts/verify-registry.sh` asks "does this scraper still
find anything", not "fetch the site" — and it is a *runtime* override so the config file a check
reads stays byte-identical to the one users get.

`tome pull --all` fetches every configured source, because someone typing that has asked for
exactly that. `tome pull --all --due` fetches only the sources their `sync.strategy` says are
due (P4-018) and **names what it skipped and why** — `manual`, `pinned`, not-yet-elapsed — so
"did nothing" is distinguishable from "is broken". A `watch` source is never fetched
automatically while DEC-006 is open.

**Naming: `pull` fetches documentation; there is no `tome sync`.**

`sync` is reserved for the *bookmark* sync engine and is not user-invoked. Earlier documents used
`tome sync`, `/tome sync`, `tome rebuild-index`, `tome import`, and `tome debug …` without ever
adding them to this specification — a CLI whose surface is defined by whoever wrote the most recent
example. The list above is now the complete surface; anything not on it does not exist.

`tome debug` subcommands (`check-integrity`, `rebuild-index`, `reset-sync`, `rollback-migration`,
`reset`) are documented in `17-rollback-recovery.md` and hidden from the top-level help, visible
via `tome debug --help`.

-----

## Milestones & Roadmap

### Effort and Schedule

**The phase durations below are calendar weeks, and they only work with a team.** Summing the
per-ticket complexity estimates in the phase plans (S ≈ 1.5 d, M ≈ 4 d, L ≈ 7.5 d) gives:

|Phase|Tickets|Effort (person-days)|Calendar target|Implied FTE|
|-----|-------|--------------------|---------------|-----------|
|P1 Foundation|22|94.5|8 weeks|**2.4**|
|P2 Search & Platforms|18|79.5|6 weeks|**2.7**|
|P3 Bookmarks & Sync|15|68|6 weeks (∥ P4)|**4.7 combined**|
|P4 Automation & Integration|18|71.5|6 weeks (∥ P3)|↑|
|P5 Polish & Launch|14|54.5|4 weeks|**2.7**|
|**Total**|**87**|**368 person-days ≈ 74 person-weeks**|**30 weeks**|**~2.5 average**|

Add the 16 days of technical spikes, and nothing for code review, design iteration, dependency
breakage, or the ~30 % overhead every real project carries.

**What this means:**

- **The 30-week roadmap is not wrong, but it is not a solo schedule.** It silently assumes about
  two and a half full-time engineers. `06-dependency-map.md` quietly confirms this — it allocates
  work across "Developer 1 / 2 / 3" — while `11-risk-register.md` RISK-010 describes a
  single-maintainer bus factor. Both cannot be true.
- **Solo, at full time, this is ~77 weeks (≈ 18 months).** As a side project at two days a week,
  it is four-plus years — by which time three of the five platform scrapers will have broken.
- **The critical path is not the constraint; capacity is.** The longest chain of strictly dependent
  tickets is ~88 working days (~18 weeks). Everything above that 18-week floor is a
  people problem, not a sequencing problem.

**Recommendation: cut scope, don't extend the calendar.** A defensible solo v1.0 is Phases 1, 2 and
the MCP half of Phase 4 — a beautiful, searchable, agent-accessible local docs reader with no sync
— which is ~55 % of the effort and keeps the two genuinely differentiated features. Bookmarks sync
(Phase 3, 68 person-days, and the highest-risk item in the register) is the first thing to cut, and
losing it costs the least: bookmarks still work locally.

This is DEC-004. **Resolve it before writing code**, because the answer changes what gets built.

### Phase Gate: do not start Phase 1 until these pass

Phase 1 currently commits 22 tickets to an architecture no spike has validated. The P0 spikes exist
precisely to prevent that, and they are all "Not Started".

- [ ] **SPIKE-001** (Tauri + Swift) — or the decision to defer native extras to Phase 5 and skip it
- [ ] **SPIKE-002** (WebView bridge performance at 60 fps)
- [ ] **SPIKE-003** (Tantivy memory at 100k pages)
- [ ] **DEC-001** licence chosen, `LICENSE` file committed
- [ ] **DEC-004** team size settled and the roadmap re-cut to match

### Phase 1: Foundation (v0.1) — 8 weeks *(94.5 person-days)*

**Goal:** Core reading experience with manual doc addition.

**Deliverables:**

- [ ] Tauri + Svelte + Rust project scaffolding
- [ ] Basic three-panel UI layout
- [ ] Generic web scraper with configurable selectors
- [ ] Content normalization and rendering pipeline
- [ ] Typography system implementation
- [ ] SQLite storage for sources and pages
- [ ] Local file-based source configs
- [ ] Basic navigation (back/forward, TOC)
- [ ] Manual source addition via config file

**Exit Criteria:** Can add a ReadTheDocs site via config and read it with consistent styling.

-----

### Phase 2: Search & Platforms (v0.2) — 6 weeks *(79.5 person-days)*

**Goal:** Intelligent search and platform-specific scrapers.

**Deliverables:**

- [ ] Tantivy integration for full-text search
- [ ] Global search UI (Cmd+K)
- [ ] Search within page (Cmd+F)
- [ ] ReadTheDocs scraper (Sphinx structure-aware)
- [ ] rustdoc scraper (search-index.js parsing)
- [ ] mdBook scraper
- [ ] Man page integration (`mandoc` rendering)
- [ ] Platform auto-detection from URL

**Exit Criteria:** Fast search across multiple doc sources, platform scrapers work reliably.

-----

### Phase 3: Bookmarks & Sync (v0.3) — 6 weeks *(68 person-days — first candidate to cut, see DEC-004)*

**Goal:** Personal layer and cross-device sync.

**Deliverables:**

- [ ] Bookmark system (add, organize, collections)
- [ ] Text highlighting with notes
- [ ] Reading position memory
- [ ] iCloud sync for bookmarks/annotations
- [ ] Source list sync (not content)
- [ ] Sync status UI
- [ ] Conflict resolution handling

**Exit Criteria:** Bookmarks sync reliably between two Macs via iCloud.

-----

### Phase 4: Automation & Integration (v0.4) — 6 weeks *(71.5 person-days)*

**Goal:** Programmable access and developer tool integration.

**Deliverables:**

- [ ] CLI tool (`tome` command)
- [ ] Local HTTP API (localhost)
- [ ] MCP server implementation
- [ ] Claude Code plugin (`/tome` commands)
- [ ] Sync strategy system (manual, scheduled, watch)
- [ ] Package registry watching (crates.io, npm, PyPI)

**Exit Criteria:** Claude Code can add and search docs; MCP tools work with AI agents.

-----

### Phase 5: Polish & Launch (v1.0) — 4 weeks *(54.5 person-days)*

**Goal:** Production-ready release.

**Deliverables:**

- [ ] Performance optimization (lazy loading, incremental indexing)
- [ ] Error handling and recovery
- [ ] Onboarding experience (first-run, sample docs)
- [ ] Preferences UI (typography, sync, keyboard)
- [ ] Menu bar integration
- [ ] Notarization and distribution (DMG, Homebrew cask)
- [ ] Documentation and landing page

**Exit Criteria:** App is stable, performant, and ready for public use.

-----

### Future (v1.x)

- **v1.1:** GitBook, Docusaurus, MkDocs scrapers
- **v1.2:** Dash docset import
- **v1.3:** Semantic/vector search option
- **v1.4:** Annotation export to Obsidian/Notion
- **v1.5:** iOS companion app (read-only)

-----

## Success Metrics

**Tome collects no telemetry. That is a deliberate product decision, and it means most
"success metrics" in the original plan could never be observed.** Targets like "80% of users
complete onboarding", "sync reliability > 99.5%", "crash-free sessions > 99.9%", and "cache hit
rate > 80%" have no measurement path in a zero-telemetry product. Keeping unmeasurable numbers in
a plan is worse than having none: they create false confidence and get quietly dropped.

Metrics are therefore split by *how they are actually observed*.

### Tier 1 — Lab metrics (measured in CI, on every commit)

These are the real quality gates. Each one requires an owned test asset, listed in the last column.

|Metric|Target|Measured by|Asset required|
|------|------|-----------|--------------|
|Search latency|P95 < 100 ms @ 10k pages|Criterion benchmark|Synthetic 10k-page corpus|
|Search latency at scale|P95 < 250 ms @ 100k pages|Criterion benchmark, nightly|Synthetic 100k-page corpus|
|Index build|< 30 s / 1 000 pages|Criterion benchmark|Same corpus|
|Index size|< 50 MB / 1 000 pages|Benchmark assertion|Same corpus|
|Idle memory|< 200 MB, 10 sources|Instrumented launch test|Fixture library|
|Cold start|< 500 ms to library visible|Instrumented launch test|Fixture library|
|**Search relevance**|Correct page in top 3 for ≥ 90 % of queries|Offline eval harness|**Labelled query→page eval set, ~200 queries.** Does not exist yet.|
|**Platform detection**|≥ 95 % correct|Offline eval harness|**Corpus of ~100 saved doc-site homepages.** Does not exist yet.|
|Sync convergence|100 % convergence, 0 lost writes|Deterministic multi-device simulation|Op-log replay harness|
|Crash-free|0 panics across the corpus|Fuzz + property tests on the parser/sanitizer|Malformed-HTML corpus|

The two bolded rows are the ones that matter most and the ones the plan never budgeted for.
"Relevant result in top 3 for 90 % of queries" was stated as a Phase 2 success metric with no eval
set behind it, which makes it an aspiration, not a metric. **Building the eval set is a ticket**
(added as P2-019 and P2-020).

### Tier 2 — Public signals (observed, not collected)

|Signal|Source|What it tells us|
|------|------|----------------|
|Issue volume and type|GitHub|Where the product hurts|
|Time-to-first-response|GitHub|Whether maintenance is sustainable|
|Download counts|GitHub Releases|Adoption trend|
|Homebrew installs|Homebrew's own analytics|Adoption trend|
|Recurring "how do I add X?" issues|GitHub|Ingestion gaps → registry candidates|

### Tier 3 — Deliberately not measured

Retention, DAU, feature usage, funnel completion, session length. These would require telemetry.
Not collecting them is the cost of the privacy stance, and it is worth paying — but it must be
stated, not glossed. **In practice this means Tome cannot be run as a metrics-driven product.**
Direction comes from user conversations and issues, not dashboards.

-----

## Open Decisions

Tracked in [`docs/decisions/`](./decisions/README.md). These are genuinely open and are **not**
resolved by this document; several block Phase 1.

|ID|Decision|Blocks|Status|
|--|--------|------|------|
|DEC-001|Licence|—|✅ Dual MIT OR Apache-2.0 ([ADR-0003](./decisions/0003-dual-mit-apache-licence.md))|
|DEC-002|Bundle identifier|—|✅ `com.alexnodeland.tome` ([ADR-0004](./decisions/0004-bundle-identifier.md))|
|DEC-003|Apple Developer Program|—|✅ **Deferred.** Unsigned distribution via own Homebrew tap ([ADR-0006](./decisions/0006-unsigned-distribution.md)). Note this also gates iCloud sync, reinforcing its deferral.|
|DEC-004|Capacity and scope|—|✅ Solo + agent workflows; sync deferred ([ADR-0005](./decisions/0005-agent-driven-build.md))|
|DEC-005|Docset import priority|v1.0 vs v1.2|Open — importing Dash docsets is the cheapest possible answer to cold start and may deserve promotion|
|DEC-006|`watch` strategy fetch behaviour|Eager background fetch vs on-open|Open — leaning on-open, to honour "no background network activity without user action"|
|DEC-007|Annotation note format|Plain text vs Markdown|Open — leaning Markdown, rendered read-only|
|DEC-008|Export destinations|Obsidian / Notion / plain files|Open — plain Markdown files first; they satisfy most of both|

-----

## Appendix A: Source Configuration Schema

Full YAML schema for documentation sources:

```yaml
# ~/Library/Application Support/Tome/sources/{name}.yaml

# Required
schema_version: 1               # REQUIRED. Enables forward migration; see 14-api-versioning.
name: string                    # Human-readable name
source:
  type: enum                    # readthedocs | rustdoc | mdbook | man | generic | local | docset

  # For remote sources
  url: string                   # Entry URL. https only unless `allow_insecure: true`.

  # For local sources
  path: string                  # Local filesystem path

  # For generic scraper
  generic:
    entry_points: [string]      # Starting paths
    max_depth: integer          # Crawl depth limit (default: 4)
    max_pages: integer          # Hard cap (default: 5000) — bounds a runaway crawl
    include_patterns: [regex]   # URL patterns to include
    exclude_patterns: [regex]   # URL patterns to exclude
    content_selector: string    # CSS selector for main content
    title_selector: string      # CSS selector for page title
    nav_selector: string        # CSS selector for navigation

  # For man pages
  man:
    paths: [string]             # Man page directories
    sections: [integer]         # Sections to index (1-8)

# Fetch etiquette — see "Crawl etiquette" above. Defaults are the safe values.
fetch:
  respect_robots: boolean       # default: true. false only for hosts you own.
  rate_limit_rps: number        # default: 2, capped at 4. Read the Docs publishes "< 4 req/s"
                                # as its crawler ceiling (SPIKE-010) — the default stays well
                                # under the strictest host's published limit, and the cap means
                                # no configuration can cross it.
  timeout_seconds: integer      # default: 30
  allow_insecure: boolean       # default: false. Permits http:// (e.g. an intranet mirror).
  max_asset_bytes: integer      # default: 262144000 (250 MB)

# Optional
version: string                 # Pin to specific version
category: string                # Organization category
display:
  icon: string                  # URL, local path, or emoji
  accent_color: string          # Hex color for UI accents

# Attribution — captured at ingest, shown in the reader footer and in exports.
attribution:
  homepage: string              # Canonical upstream URL
  licence: string               # SPDX identifier if determinable
  # Every rendered page also links back to its origin URL. This is not optional:
  # it is how a local cache stays a local cache rather than a republication.

# Sync configuration
sync:
  strategy: enum                # manual | on_launch | scheduled | watch
  schedule: enum                # For scheduled ONLY: daily | weekly | monthly
  watch_source: string          # For watch: "crates:serde", "npm:react", "pypi:polars"
  pin_version: boolean          # If true, never auto-update

# Enrichment (post-v1, unfunded — see Non-Goals)
enrich:
  link_to_source: boolean       # Link symbols to source code
  tldr_summaries: boolean       # Generate brief summaries
```

**Note on `sync.strategy` vs `sync.schedule`.** `strategy` accepts only
`manual | on_launch | scheduled | watch`. `weekly` is a `schedule` value, not a `strategy` value —
several examples elsewhere in the plan had `strategy: weekly`, which this schema rejects. The
parser must fail loudly on that mistake with a message naming the correct field, because it is the
error users will make most often.

-----

## Appendix B: API Reference

### Local HTTP API

Base URL: `http://127.0.0.1:7431/api/v1`

**Every request requires `Authorization: Bearer <token>`** except `GET /api/v1/status`. See
[Local API](#8-local-api) for why loopback is not a trust boundary. The version segment is
mandatory — earlier drafts used both `/api/…` and `/api/v1/…` in different documents.

#### Search

```http
GET /api/v1/search?q={query}&scope={source_id}&limit={n}
Authorization: Bearer <token>

Response:
{
  "results": [
    {
      "source_id": "uuid",
      "source_name": "Rust std",
      "page_path": "/std/vec/struct.Vec.html",
      "title": "Vec in std::vec",
      "snippet": "A contiguous growable array type...",
      "score": 0.95
    }
  ],
  "total_hits": 42,      // total matching documents, NOT results.len()
  "total_capped": false,  // true when total_hits saturated its 1000-document counting ceiling
  "returned": 10,
  "query_time_ms": 12
}
```

**Error shape** — one format for every endpoint, so clients can handle failures generically:

```json
{
  "error": {
    "code": "source_not_found",
    "message": "No source with id 'abc123'.",
    "details": null
  }
}
```

| Status | When |
|--------|------|
| `400` | Malformed request or failed validation (includes SSRF-rejected URLs) |
| `401` | Missing or invalid bearer token |
| `403` | Origin/Host validation failed |
| `404` | Unknown source, page, or bookmark |
| `409` | Source already exists |
| `429` | Rate limited |
| `503` | Index unavailable (rebuilding) — includes `Retry-After` |

#### Sources

```http
GET    /api/v1/sources
GET    /api/v1/sources/{id}
POST   /api/v1/sources?id={id}  # Body: YAML config (JSON parses too — YAML is a superset).
                                # The id is a query parameter because the config file's name IS
                                # the source's identity. URL passes the SSRF filter first: a
                                # literal-IP URL is judged immediately; a hostname is judged at
                                # fetch time by the pinned resolver, the only judgement that
                                # survives DNS rebinding. 201, or 409 if the id exists.
DELETE /api/v1/sources/{id}
POST   /api/v1/sources/{id}/sync   # 202; the pull runs in the background. 409 while running.
```

#### Pages

```http
GET /api/v1/sources/{id}/pages          # paginated: ?cursor=&limit=
GET /api/v1/sources/{id}/pages/{path}   # normalized, sanitized HTML — never raw upstream HTML
```

#### Bookmarks

```http
GET    /api/v1/bookmarks
POST   /api/v1/bookmarks        # Body: { source_id, page_path, note? }
DELETE /api/v1/bookmarks/{id}
```

#### Status

```http
GET /api/v1/status              # unauthenticated; returns only {status, version}
GET /api/v1/status/detail       # authenticated; index stats, sync state, health checks
```

### MCP Tools

```typescript
// Tool definitions for MCP server

tool tome_search {
  description: "Search documentation across all sources or within a specific source"
  parameters: {
    query: string (required)
    scope: string (optional) - source ID to limit search
    limit: number (optional, default 10)
  }
  returns: SearchResult[]
}

tool tome_get_page {
  description: "Retrieve the content of a documentation page"
  parameters: {
    source_id: string (required)
    page_path: string (required)
  }
  returns: { title: string, content: string, toc: Section[] }
}

tool tome_list_sources {
  description: "List all available documentation sources"
  parameters: {}
  returns: Source[]
}

tool tome_lookup_symbol {
  description: "Look up a symbol (function, type, module) across documentation"
  parameters: {
    symbol: string (required)
    language: string (optional) - filter by language
  }
  returns: SymbolResult[]
}

tool tome_bookmark {
  description: "Bookmark a documentation page with optional note"
  parameters: {
    source_id: string (required)
    page_path: string (required)  
    note: string (optional)
  }
  returns: Bookmark
}
```

-----

## Appendix C: Keyboard Shortcut Reference

**This table is the single source of truth.** All other documents link here.

|Category      |Action               |Shortcut             |Notes|
|--------------|---------------------|---------------------|-----|
|**Navigation**|Back                 |`Cmd + [`            ||
|              |Forward              |`Cmd + ]`            ||
|              |Go to source         |`Cmd + O`            ||
|              |Go to page (by title)|`Cmd + Shift + O`    |Was `Cmd+P`, which is Print system-wide|
|**Search**    |Global search        |`Cmd + K`            ||
|              |Search in page       |`Cmd + F`            ||
|              |Next match           |`Cmd + G`            ||
|              |Previous match       |`Cmd + Shift + G`    ||
|**Reading**   |Scroll down          |`J` / `Space`        |Reader-scoped — see below|
|              |Scroll up            |`K` / `Shift + Space`|Reader-scoped|
|              |Page down            |`Ctrl + D`           |Reader-scoped|
|              |Page up              |`Ctrl + U`           |Reader-scoped|
|              |Top of page          |`G G`                |Reader-scoped|
|              |Bottom of page       |`Shift + G`          |Reader-scoped|
|              |Next section         |`]` `]`              |Was `N`; `n`/`N` is find-next by vim convention|
|              |Previous section     |`[` `[`              |Was `P`|
|**Annotate**  |Highlight selection  |`Cmd + Shift + H`    |Was `Cmd+H`, which is Hide Application|
|              |Add note to highlight|`Cmd + Shift + N`    ||
|**Bookmarks** |Bookmark page        |`Cmd + D`            ||
|              |Show bookmarks       |`Cmd + B`            ||
|**Sources**   |Add source           |`Cmd + N`            |Referenced by onboarding/E2E; was undocumented|
|**View**      |Toggle source sidebar|`Cmd + 1`            ||
|              |Toggle TOC sidebar   |`Cmd + 2`            ||
|              |Toggle both sidebars |`Cmd + \`            ||
|              |Increase font size   |`Cmd + =`            ||
|              |Decrease font size   |`Cmd + -`            ||
|              |Reset font size      |`Cmd + 0`            ||
|**Sync**      |Sync current source  |`Cmd + R`            ||
|              |Sync all sources     |`Cmd + Shift + R`    ||
|**App**       |Preferences          |`Cmd + ,`            ||
|              |Quit                 |`Cmd + Q`            ||
|**Global**    |Activate Tome        |`Cmd + Shift + D`    |System-wide; user-configurable; off by default|

**Rules that make this safe:**

1. **Reader-scoped keys are single letters and must never fire while a text input has focus.**
   `J`, `K`, `G`, `[`, `]` are bound on the reader surface only, and every handler bails if
   `document.activeElement` is an input, textarea, or `contenteditable`. Without this, typing "j"
   in the bookmark-filter box scrolls the page.
2. **Nothing shadows a macOS system shortcut.** `Cmd+H` (Hide), `Cmd+M` (Minimize), `Cmd+P`
   (Print), `Cmd+W`, `Cmd+Q`, `Cmd+Tab`, `Cmd+Space` are reserved and unused. Two shortcuts in
   earlier drafts violated this and are corrected above.
3. **Every shortcut is discoverable.** Each appears in the menu bar next to its command, so
   `Cmd+/` cheat sheets are unnecessary and VoiceOver announces them.
4. **The global shortcut is opt-in.** Registering a system-wide hotkey by default is hostile and
   collides unpredictably with other apps; conflict detection is required before it is enabled.

-----

*End of PRD*