# Tome Project: Comprehensive Dependency Map

This document provides a complete visualization of dependencies between all 90 tickets across the 5 phases of the Tome project.

> **Owner note.** `00-project-overview.md` owns the critical path and the effort figures. This
> document owns the full graph and the parallelization analysis. Where they previously disagreed —
> and they did, in three places — the overview wins.

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

**Definition:** the longest chain of strictly dependent tickets, weighted by estimated effort. This
document previously contained *two* versions of the critical path that disagreed with each other
and with `00-project-overview.md` — a "Primary Critical Path" diagram and a 23-row table naming a
different set of tickets. All three now agree; `00-project-overview.md` is the owner.

### The critical path (15 tickets, ≈ 88 working days)

```
P1-001 → P1-012 → P1-013 → P1-016 → P1-020
       → P2-001 → P2-002 → P2-003
       → P3-001 → P3-010 → P3-011 → P3-012 → P3-013
       → P5-001 → P5-002
```

| # | Ticket | Effort | Why it is on the path |
|---|--------|--------|-----------------------|
| 1 | P1-001 Tauri init | 4 d | Nothing exists before it |
| 2 | P1-012 HTML→AST parser | 7.5 d | Longest branch out of P1-001 |
| 3 | P1-013 Normalization | 7.5 d | Everything rendered or indexed passes through it |
| 4 | P1-016 Reader bridge | 7.5 d | Also needs P1-003 and P1-015, both shorter branches |
| 5 | P1-020 Navigation | 4 d | Completes the Phase 1 exit criteria |
| 6 | P2-001 Tantivy | 7.5 d | Also needs P1-004 + P1-021 (12 d branch, not critical) |
| 7 | P2-002 Index schema | 4 d | Gates ranking, symbols, incremental indexing |
| 8 | P2-003 Incremental indexing | 7.5 d | Longest branch in Phase 2 |
| 9 | P3-001 Bookmark model | 4 d | Gates all of Phase 3 |
| 10 | P3-010 Sync design | 7.5 d | |
| 11 | P3-011 Op log + codec | 4 d | |
| 12 | P3-012 Sync engine | 7.5 d | |
| 13 | P3-013 Conflict resolution | 4 d | Phase 3's longest chain (27 d) beats Phase 4's (23.5 d) |
| 14 | P5-001 Performance | 7.5 d | Cannot start before the system exists |
| 15 | P5-002 Lazy loading | 4 d | |

### Release-gate chain (mandatory, not longest)

`P5-010 notarize (4 d) → P5-011 DMG (4 d) → P5-012 distribution (1.5 d)` — 9.5 d. Cannot be
compressed and cannot start until DEC-002 and DEC-003 are resolved, so **start the Apple Developer
Program enrolment early**; it is a lead-time item, not a task.

### Near-critical: the agent-integration chain

`P4-008 → P4-013 → P4-014 → P4-015 → P4-017` — 23.5 d against Phase 3's 27 d. Only 3.5 days of
slack. **If Phase 3 is cut (see DEC-004), this becomes the critical path** — which is convenient,
because it is also the more differentiated half of the product.

### What the critical path does *not* tell you

The path is ~18 calendar weeks with unlimited people. The plan is ~381 person-days. **Capacity, not
sequencing, is the binding constraint.** Optimising the dependency graph further buys nothing until
DEC-004 is answered.

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
                                                        P2-019 (Relevance eval)
                                                        P2-020 (Detection corpus)
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
| **C: Ingestion** | P1-001 → P1-008 → P1-009, P1-010, P1-011, P1-023 | Scraping system + assets |
| **D: Config** | P1-001 → P1-005, P1-006 → P1-007 | Configuration |
| **E: Parsing** | P1-001 → P1-012 → P1-013, P1-014 | Content processing |

**Recommended Team Allocation:**
- Developer 1: Track A + Track B (frontend + data)
- Developer 2: Track C + Track D (backend infra)
- Developer 3: Track E (parsing/rendering)

> ⚠️ **This allocation is the plan's implicit staffing assumption, and it is unresolved (DEC-004).**
> Three developers on Phase 1 is roughly what the arithmetic requires: 102 person-days over 8 weeks
> is 2.6 FTE. But `11-risk-register.md` RISK-010 describes a **single maintainer** bus factor. The
> plan cannot simultaneously assume three developers and one. Until DEC-004 is answered, treat
> every calendar date in this plan set as conditional.
>
> **Solo sequencing, if that is the answer.** Parallel tracks are worthless to one person; what
> matters instead is ordering to get feedback early and to avoid rework:
>
> 1. P1-001 → P1-008 → P1-012 → P1-013 → a crude renderer. Get *one real docs site* readable
>    end-to-end before building any UI polish. This validates the riskiest assumption — that
>    normalization across arbitrary sites is tractable — in ~3 weeks rather than 8.
> 2. Then P1-004/021 storage, then the three-panel UI, then P1-023 assets.
> 3. Then Phase 2 search.
> 4. Then the MCP half of Phase 4, which is the differentiated feature.
> 5. Ship. Decide about sync afterwards, with users.

### Phase 2 Parallel Tracks

| Track | Tickets | Focus |
|-------|---------|-------|
| **A: Search Core** | P2-001 → P2-002 → P2-003, P2-006, P2-015 | Tantivy integration |
| **A0: Eval** | P2-019, P2-020 | **Build these first** — ranking work without an eval set is guesswork |
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
