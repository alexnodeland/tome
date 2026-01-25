# Tome Project: Comprehensive Dependency Map

This document provides a complete visualization of dependencies between all 87 tickets across the 5 phases of the Tome project.

---

## Dependency Matrix

### Legend

- **→** = "depends on" (must be completed first)
- **Critical Path** = Tickets that directly impact project completion timeline
- **Parallel Candidates** = Tickets that can be worked on simultaneously

---

## Phase-Level Dependencies

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  ┌─────────────────────┐                                                    │
│  │     PHASE 1         │                                                    │
│  │    Foundation       │                                                    │
│  │   (22 tickets)      │                                                    │
│  └──────────┬──────────┘                                                    │
│             │                                                               │
│             ▼                                                               │
│  ┌─────────────────────┐                                                    │
│  │     PHASE 2         │                                                    │
│  │  Search & Platforms │                                                    │
│  │   (18 tickets)      │                                                    │
│  └──────────┬──────────┘                                                    │
│             │                                                               │
│     ┌───────┴───────┐                                                       │
│     ▼               ▼                                                       │
│  ┌──────────┐    ┌──────────────────┐                                       │
│  │ PHASE 3  │    │     PHASE 4      │  ◄── Can run in parallel              │
│  │ Bookmarks│    │   Automation &   │                                       │
│  │ & Sync   │    │   Integration    │                                       │
│  │  (15)    │    │     (18)         │                                       │
│  └────┬─────┘    └────────┬─────────┘                                       │
│       │                   │                                                 │
│       └─────────┬─────────┘                                                 │
│                 ▼                                                           │
│       ┌─────────────────────┐                                               │
│       │      PHASE 5        │                                               │
│       │   Polish & Launch   │                                               │
│       │     (14 tickets)    │                                               │
│       └─────────────────────┘                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Critical Path Analysis

The critical path represents the longest sequence of dependent tickets. Any delay in these tickets directly delays the entire project.

### Primary Critical Path

```
P1-001 → P1-002 → P1-003 → P1-016 → P1-020 → P2-001 → P2-002 → P2-004 → P2-005
                                                                    ↓
                                    ┌───────────────────────────────┘
                                    ↓
                              P4-001 → P4-002 → P4-005 → P4-017
                                                    ↓
                              P5-001 → P5-010 → P5-011 → P5-012
```

### Critical Path Tickets (23 total)

| Phase | Ticket | Title | Why Critical |
|-------|--------|-------|--------------|
| P1 | P1-001 | Initialize Tauri + Rust project | Foundation for everything |
| P1 | P1-002 | Setup Svelte frontend scaffold | Required for UI |
| P1 | P1-003 | Configure Tauri-Svelte integration | Bridges backend and frontend |
| P1 | P1-004 | Design SQLite schema | Data foundation |
| P1 | P1-008 | Implement generic HTTP scraper core | Content ingestion |
| P1 | P1-012 | Create HTML-to-AST parser | Content processing |
| P1 | P1-013 | Build AST normalization pipeline | Content normalization |
| P1 | P1-016 | Build WKWebView rendering bridge | Content display |
| P1 | P1-018 | Build source library sidebar | Core UI |
| P2 | P2-001 | Integrate Tantivy search engine | Search foundation |
| P2 | P2-002 | Design search index schema | Search structure |
| P2 | P2-004 | Implement global search UI | User-facing search |
| P2 | P2-005 | Create search results component | Search display |
| P2 | P2-010 | Implement ReadTheDocs scraper | First platform |
| P3 | P3-001 | Design bookmark data model | Bookmark foundation |
| P3 | P3-010 | Design CloudKit sync architecture | Sync foundation |
| P3 | P3-012 | Build sync engine core | Sync implementation |
| P4 | P4-001 | Design CLI architecture | CLI foundation |
| P4 | P4-002 | Implement CLI scaffolding | CLI implementation |
| P4 | P4-009 | Implement HTTP server with Axum | API server |
| P4 | P4-014 | Implement MCP protocol handler | AI integration |
| P5 | P5-010 | macOS notarization setup | Distribution requirement |
| P5 | P5-012 | Create Homebrew cask | Primary distribution |

---

## Full Dependency Graph

### Phase 1: Foundation

```
P1-001 (Tauri Init)
   │
   ├──────────────────────────────────────────────────────┐
   │                                                      │
   ├── P1-002 (Svelte) ──┬── P1-003 (Integration)         │
   │        │            │         │                      │
   │        │            │         └── P1-016 (WebView)◄──┼──┐
   │        │            │                 │              │  │
   │        │            ├── P1-015 (Typography)──────────┘  │
   │        │            │                                   │
   │        │            └── P1-017 (Layout)──┬── P1-018 (Library)
   │        │                                 │       │
   │        │                                 │       └─► P1-020 (Navigation)
   │        │                                 │               ▲
   │        │                                 └── P1-019 (TOC)│
   │        │                                       │         │
   │        │                                       └─────────┤
   │        │                                                 │
   │        └─────────────────────────────────────────────────┤
   │                                                          │
   ├── P1-004 (SQLite) ───────────────────┬── P1-018         │
   │                                      │                   │
   │                                      └── P1-021 (Metadata)
   │
   ├── P1-005 (YAML) ──┬── P1-007 (Watcher)
   │                   │         │
   │                   │         └── P1-022 (Add Workflow)◄── P1-008
   │                   │                                         │
   ├── P1-006 (FS) ────┘                                         │
   │                                                             │
   ├── P1-008 (Scraper) ──┬── P1-009 (Selectors)                 │
   │        │             │                                      │
   │        │             ├── P1-010 (BFS)                       │
   │        │             │                                      │
   │        │             └── P1-011 (URL Filter)                │
   │        │                                                    │
   │        └────────────────────────────────────────────────────┘
   │
   └── P1-012 (Parser) ──┬── P1-013 (Normalize) ──┬── P1-016
                         │                        │
                         │                        ├── P1-019
                         │                        │
                         │                        └── P1-021
                         │
                         └── P1-014 (Syntax HL)
```

### Phase 2: Search & Platforms

```
                    ┌──────────────────────────────────────┐
                    │         FROM PHASE 1                 │
                    │                                      │
                    │  P1-004 (SQLite)   P1-021 (Metadata) │
                    │       │                 │            │
                    └───────┼─────────────────┼────────────┘
                            │                 │
                            ▼                 ▼
                         P2-001 (Tantivy) ◄───┘
                            │
            ┌───────────────┼───────────────────────────────────┐
            │               │                                   │
            ▼               ▼                                   ▼
      P2-009 (Fuzzy)   P2-002 (Schema) ──────────────┐    P2-018 (Bench)
                            │                        │
               ┌────────────┼────────────┐           │
               │            │            │           │
               ▼            ▼            ▼           ▼
         P2-003       P2-006        P2-015      (P2-001)
       (Indexing)   (Ranking)     (Symbols)
                        │              │
                        └──────┬───────┘
                               │
                               ▼
                         P2-004 (Search UI)
                               │
            ┌──────────────────┼──────────────────┐
            │                  │                  │
            ▼                  ▼                  ▼
      P2-016 (History)   P2-005 (Results)   P2-008 (Scoping)
                               │
                               ▼
                         P2-017 (Keyboard)


                    ┌──────────────────────────────────────┐
                    │         FROM PHASE 1                 │
                    │                                      │
                    │  P1-008 (Scraper)   P1-016 (WebView) │
                    │       │                 │            │
                    └───────┼─────────────────┼────────────┘
                            │                 │
       ┌────────────────────┼─────────────────┼───────────┐
       │                    │                 │           │
       ▼                    ▼                 ▼           │
  P2-010 (RTD)        P2-011 (rustdoc)   P2-007 (In-page) │
       │                    │                             │
       │                    │                             │
       ▼                    ▼                             │
  P2-012 (mdBook)──────────►────────────────────┐         │
       │                                        │         │
       │                                        ▼         │
       └────────────────────────────────► P2-014 (Detect) │
                                                          │
  P2-013 (Man pages)◄─────────────────────────────────────┘
```

### Phase 3: Bookmarks & Sync

```
                    ┌──────────────────────────────────────┐
                    │         FROM PHASES 1 & 2            │
                    │                                      │
                    │  P1-004 (SQLite)   P1-016 (WebView)  │
                    │       │                 │            │
                    └───────┼─────────────────┼────────────┘
                            │                 │
            ┌───────────────┘                 │
            │                                 │
            ▼                                 │
      P3-001 (Data Model)                     │
            │                                 │
      ┌─────┴─────┬───────────────────┐       │
      │           │                   │       │
      ▼           ▼                   ▼       │
P3-002 (CRUD)   P3-005 (Collections)  P3-010 (CloudKit Design)
      │           │                       │
      ├───────────┤                       │
      │           │                       │
      ▼           ▼                       ▼
P3-003 (UI)   P3-006 (Collection UI)  P3-011 (CKRecord)
      │                                   │
      ▼                                   ▼
P3-004 (Cmd+D)                        P3-012 (Sync Engine)
                                          │
                            ┌─────────────┼─────────────┐
                            │             │             │
                            ▼             ▼             ▼
                      P3-013         P3-014         P3-015
                    (Conflicts)    (Status UI)   (Offline Queue)


                            P1-016 (WebView)
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
              ▼                  ▼                  ▼
        P3-007 (Highlight)  P3-009 (Position)      │
              │                  │                  │
              ▼                  │                  │
        P3-008 (Notes)           │                  │
                                 │                  │
                                 └──────► P1-004 ◄──┘
```

### Phase 4: Automation & Integration

```
                    ┌──────────────────────────────────────┐
                    │         FROM PHASES 1 & 2            │
                    │                                      │
                    │  P2-001 (Search)    P1-008 (Scraper) │
                    │       │                 │            │
                    │       │    P2-014 (Detect)           │
                    │       │        │                     │
                    └───────┼────────┼────────┼────────────┘
                            │        │        │
                            ▼        │        │
                      P4-001 (CLI Design)     │
                            │        │        │
                            ▼        │        │
                      P4-002 (CLI Scaffold)   │
                            │        │        │
      ┌──────────┬──────────┼────────┼────────┼─────────┬────────┐
      │          │          │        │        │         │        │
      ▼          ▼          ▼        ▼        ▼         ▼        ▼
  P4-003     P4-004     P4-005   P4-006    P4-007   P4-018     │
  (add)      (pull)    (search) (list/rm)  (json)   (sync)     │
      │          │          │                                   │
      ▼          │          │                                   │
  P2-014 ◄───────┘          │                                   │
  (Detect)                  │                                   │
                            │                                   │
                            └───────────────────────────────┐   │
                                                            │   │
                            ┌──────── P4-008 (API Design)   │   │
                            │              │                │   │
                            │              ▼                │   │
                            │       P4-009 (Axum Server)    │   │
                            │              │                │   │
                            │   ┌──────────┼──────────┐     │   │
                            │   │          │          │     │   │
                            │   ▼          ▼          ▼     │   │
                            │ P4-010   P4-011     P4-012    │   │
                            │ (API1)   (API2)     (Auth)    │   │
                            │                               │   │
                            │                               │   │
                            └──────── P4-013 (MCP Design)   │   │
                                           │                │   │
                                           ▼                │   │
                                     P4-014 (MCP Handler)   │   │
                                           │                │   │
                               ┌───────────┼───────────┐    │   │
                               │           │           │    │   │
                               ▼           ▼           ▼    │   │
                           P4-015      P4-016              │   │
                          (Tools1)    (Tools2)             │   │
                               │           │               │   │
                               └─────┬─────┘               │   │
                                     │                     │   │
                                     └─────────────────────┘   │
                                                │              │
                                                ▼              │
                                          P4-017 (Claude Code) │
                                                               │
                               P4-004 ─────────────────────────┘
```

### Phase 5: Polish & Launch

```
                    ┌──────────────────────────────────────┐
                    │         ALL PREVIOUS PHASES          │
                    │                                      │
                    └───────────────────┬──────────────────┘
                                        │
            ┌───────────────────────────┼───────────────────────────┐
            │                           │                           │
            ▼                           ▼                           ▼
      P5-001 (Perf)              P5-004 (Errors)              P5-008 (Menu)
            │                           │                           │
      ┌─────┴─────┐                     ▼                           ▼
      │           │               P5-005 (Recovery)           P5-009 (Global)
      ▼           ▼
P5-002 (Lazy)  P5-003 (Incr)


                                        │
            ┌───────────────────────────┼───────────────────────────┐
            │                           │                           │
            ▼                           ▼                           ▼
      P5-006 (Onboard)           P5-007 (Prefs)              P5-010 (Notarize)
            │                           │                           │
      ┌─────┘                     ┌─────┘                           ▼
      │                           │                           P5-011 (DMG)
      ▼                           ▼                                 │
  P1-022 (Add)               P1-017 (Layout)                        ▼
  P2-014 (Detect)                                             P5-012 (Homebrew)


      P5-013 (Docs)                                           P5-014 (Landing)
```

---

## Parallel Work Opportunities

### Phase 1 Parallel Tracks

These workstreams can proceed independently within Phase 1:

| Track | Tickets | Focus |
|-------|---------|-------|
| **A: Core Infra** | P1-001 → P1-002 → P1-003 → P1-016 | Tauri/Svelte setup |
| **B: Data Layer** | P1-001 → P1-004 → P1-021 | SQLite and storage |
| **C: Ingestion** | P1-001 → P1-008 → P1-009, P1-010, P1-011 | Scraping system |
| **D: Config** | P1-001 → P1-005, P1-006 → P1-007 | Configuration |
| **E: Parsing** | P1-001 → P1-012 → P1-013, P1-014 | Content processing |

**Recommended Team Allocation:**
- Developer 1: Track A + Track B (frontend + data)
- Developer 2: Track C + Track D (backend infra)
- Developer 3: Track E (parsing/rendering)

### Phase 2 Parallel Tracks

| Track | Tickets | Focus |
|-------|---------|-------|
| **A: Search Core** | P2-001 → P2-002 → P2-003, P2-006, P2-015 | Tantivy integration |
| **B: Search UI** | P2-004 → P2-005 → P2-008, P2-016, P2-017 | Search interface |
| **C: Scrapers** | P2-010, P2-011, P2-012, P2-013 → P2-014 | Platform scrapers |

### Phase 3 & 4 Parallel Execution

Phases 3 and 4 can be developed in parallel after Phase 2:

```
        Phase 2 Complete
              │
    ┌─────────┴─────────┐
    │                   │
    ▼                   ▼
Phase 3 Team       Phase 4 Team
(Bookmarks/Sync)   (CLI/API/MCP)
    │                   │
    └─────────┬─────────┘
              │
              ▼
        Phase 5
```

---

## Cross-Phase Dependencies

These tickets have dependencies that cross phase boundaries:

| Ticket | Phase | Depends On | From Phase |
|--------|-------|------------|------------|
| P2-001 | 2 | P1-004, P1-021 | 1 |
| P2-007 | 2 | P1-016 | 1 |
| P2-010-13 | 2 | P1-008 | 1 |
| P3-001 | 3 | P1-004 | 1 |
| P3-007, P3-009 | 3 | P1-016 | 1 |
| P4-001 | 4 | P2-001 | 2 |
| P4-003 | 4 | P2-014 | 2 |
| P4-018 | 4 | P1-008 | 1 |
| P5-003 | 5 | P2-003 | 2 |
| P5-006 | 5 | P1-022, P2-014 | 1, 2 |
| P5-007 | 5 | P1-017 | 1 |

---

## Blocking Dependencies Summary

### Most Critical Blockers

These tickets block the most other work:

| Ticket | Blocks | Count |
|--------|--------|-------|
| **P1-001** | P1-002, P1-003, P1-004, P1-005, P1-006, P1-008, P1-012 | 7 |
| **P1-008** | P1-009, P1-010, P1-011, P1-022, P2-010, P2-011, P2-012, P2-013, P4-018 | 9 |
| **P2-001** | P2-002, P2-003, P2-004, P2-006, P2-009, P2-018, P4-001 | 7 |
| **P1-016** | P1-020, P2-007, P3-007, P3-009 | 4 |
| **P4-002** | P4-003, P4-004, P4-005, P4-006, P4-007 | 5 |
| **P4-009** | P4-010, P4-011, P4-012 | 3 |
| **P4-014** | P4-015, P4-016 | 2 |

### Tickets with Most Dependencies

These tickets require the most prerequisites:

| Ticket | Depends On | Count |
|--------|------------|-------|
| **P1-022** | P1-005, P1-007, P1-008 | 3 |
| **P1-020** | P1-016, P1-018 | 2 |
| **P1-016** | P1-003, P1-013, P1-015 | 3 |
| **P4-017** | P4-005, P4-015 | 2 |
| **P5-006** | P1-022, P2-014 | 2 |

---

## Dependency Validation Rules

When working on tickets, verify:

1. **All blockers complete**: Check that all dependencies are marked done
2. **No circular deps**: Ensure no ticket depends on itself transitively
3. **Phase order respected**: Earlier phase tickets complete first
4. **Critical path protected**: Don't delay critical path tickets for non-critical work

---

## Quick Reference: What Can I Work On?

### If Phase 1 is in progress:

After completing P1-001, you can parallelize:
- P1-002, P1-003 (frontend)
- P1-004 (database)
- P1-005, P1-006 (config)
- P1-008 (scraping)
- P1-012 (parsing)

### If Phase 2 is in progress:

After completing P2-001, you can parallelize:
- P2-002, P2-003 (indexing)
- P2-004, P2-005 (search UI)
- P2-010, P2-011, P2-012, P2-013 (scrapers)

### If Phases 3 & 4 are in progress:

Work entirely in parallel:
- Phase 3: P3-001 → P3-002 → ... → P3-015
- Phase 4: P4-001 → P4-002 → ... → P4-018

### If Phase 5 is in progress:

Most tickets can parallelize:
- P5-001, P5-002, P5-003 (performance)
- P5-004, P5-005 (errors)
- P5-006 (onboarding)
- P5-007 (preferences)
- P5-008, P5-009 (menu bar)
- P5-010 → P5-011 → P5-012 (distribution - sequential)
- P5-013, P5-014 (documentation)
