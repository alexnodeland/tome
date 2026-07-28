# Tome Project Plan: Master Overview

**Version:** 1.1
**Created:** 2026-01-25
**Last reviewed:** 2026-07-28
**Status:** Planning drafted, **not validated**. Zero tickets started. All P0 spikes outstanding.

> **Status honesty.** The previous status was "Planning Complete", which reads as
> ready-to-implement. Six months on, nothing has been built, no spike has run, and the plan still
> contains unresolved architectural forks. Planning is not complete until the
> [phase gate](#phase-gate-before-any-implementation) passes. See
> [the plan review](../reviews/2026-07-28-plan-review.md).

---

## Executive Summary

Tome is a native macOS application providing unified, offline access to technical documentation. This document serves as the master project plan, organizing development into 5 phases with 90 detailed tickets.

### Project Metrics

| Metric | Value |
|--------|-------|
| Total Phases | 5 |
| Total Tickets | 90 |
| Critical Path Tickets | 15 (+3 release-gate) |
| Total Effort | ~381 person-days ≈ 76 person-weeks |
| Calendar Duration | 30 weeks **at ~2.5 FTE** |
| Serial floor (infinite parallelism) | ~18 weeks |
| Solo duration (full time) | ~77 weeks |

**Read the FTE column.** The 30-week figure is only achievable with roughly two and a half
full-time engineers. See [Effort Reality](#effort-reality).

---

## Phase Gate: before any implementation

These block Phase 1. They are ordered by how much rework their answers avoid.

| Gate | Why it blocks | Owner | Status |
|------|---------------|-------|--------|
| DEC-004 — team size | Determines whether the 30-week plan or the 77-week plan is real, and therefore what scope survives | — | **Open** |
| SPIKE-001 — Tauri/Swift | 23 Phase-1 tickets are written against an unvalidated shell architecture | — | Not started |
| SPIKE-002 — WebView bridge | Reader, scroll tracking, highlighting and in-page search all sit on this | — | Not started |
| SPIKE-003 — Tantivy at scale | Four different memory targets exist in the plan; none is measured | — | Not started |
| DEC-001 — licence | Nothing should be published without one | — | **Open** |
| DEC-002 — bundle identifier | `com.example.tome` is currently threaded through notarization, Keychain, iCloud, and the Homebrew zap list | — | **Open** |

---

## Phase Summary

| Phase | Name | Tickets | Effort (pd) | Calendar | Implied FTE | Focus |
|-------|------|---------|-------------|----------|-------------|-------|
| **P1** | Foundation | 23 | 102 | 8 wk | 2.6 | Core reading experience, scaffolding |
| **P2** | Search & Platforms | 20 | 85 | 6 wk | 2.9 | Full-text search, platform scrapers, eval sets |
| **P3** | Bookmarks & Sync | 15 | 68 | 6 wk (∥P4) | 4.7 combined | Personal layer, iCloud sync |
| **P4** | Automation & Integration | 18 | 71.5 | 6 wk (∥P3) | ↑ | CLI, API, MCP, Claude Code plugin |
| **P5** | Polish & Launch | 14 | 54.5 | 4 wk | 2.7 | Performance, UX, distribution |

### Effort Reality

Effort is the sum of per-ticket complexity estimates (S ≈ 1.5 d, M ≈ 4 d, L ≈ 7.5 d), plus 16 days
of spikes. It excludes code review, design iteration, dependency breakage, and the overhead every
real project carries.

- **The critical path is ~88 working days (~18 weeks).** That is the floor with unlimited people.
- **The work is ~381 person-days.** That is the constraint.
- `06-dependency-map.md` allocates work to "Developer 1 / 2 / 3"; `11-risk-register.md` RISK-010
  describes a single maintainer. **Both cannot be true.** This is DEC-004 and it is unresolved.
- **If solo: cut, don't stretch.** P1 + P2 + the MCP half of P4 is ~55 % of the effort and retains
  both genuinely differentiated features (arbitrary-site ingestion; agent access). P3 (sync) is
  68 person-days, carries the highest-scoring risk in the register, and its loss costs the least —
  bookmarks still work, they just don't follow you between machines.

---

## Technology Stack

**Authoritative version: [PRD § Technical Architecture](../PRD.md#stack).** Restating it here
caused drift — this table previously listed Swift+AppKit, Svelte, WKWebView and Tauri as four peer
layers, which is not a coherent architecture (Tauri *is* the shell, and it *owns* the WKWebView).

Summary only:

| Layer | Technology | Note |
|-------|------------|------|
| Application shell | Tauri (Rust) | Owns process, window, menus. Not a separate Swift shell. |
| UI | Svelte + TypeScript | Runs in Tauri's primary webview |
| Reader surface | Sandboxed `<iframe>` | Isolates untrusted doc HTML; not a second WKWebView |
| Core engine | Rust | Scraping, parsing, indexing, sync |
| Search | Tantivy | |
| Metadata | SQLite (`sqlx`) | |
| Sync | iCloud Drive container (files) | **Not CloudKit** — see PRD § iCloud Sync Architecture |
| Native extras | Swift plugin, Phase 5, conditional on SPIKE-001 | Menu-bar popover + global hotkey only |

---

## Critical Path

The critical path is the **longest chain of strictly dependent tickets**, weighted by estimated
effort. Three mutually inconsistent versions previously existed (here, and twice in
`06-dependency-map.md`), naming different tickets. This is the derived one; `06` now agrees.

```
P1-001 ─ Tauri init                    4 d
   └─ P1-012 HTML→AST parser         7.5 d
        └─ P1-013 Normalization      7.5 d
             └─ P1-016 Reader bridge 7.5 d      (also needs P1-003, P1-015)
                  └─ P1-020 Navigation  4 d
                       └─ P2-001 Tantivy      7.5 d   (also needs P1-004, P1-021)
                            └─ P2-002 Index schema  4 d
                                 └─ P2-003 Incremental indexing 7.5 d
                                      └─ P3-001 Bookmark model  4 d
                                           └─ P3-010 Sync design    7.5 d
                                                └─ P3-011 Sync records 4 d
                                                     └─ P3-012 Sync engine 7.5 d
                                                          └─ P3-013 Conflicts 4 d
                                                               └─ P5-001 Perf 7.5 d
                                                                    └─ P5-002 Lazy load 4 d
                                                                              ≈ 88 working days
```

**15 critical-path tickets.** The previous count of 23 came from a table that included tickets not
on any single chain.

**Release-gate chain** (mandatory to ship, not the longest path):
`P5-010 notarize → P5-011 DMG → P5-012 distribution` (9.5 d).

**Near-critical, watch it:** the agent-integration chain
`P4-008 → P4-013 → P4-014 → P4-015 → P4-017` is 23.5 d against P3's 27 d. If Phase 3 is cut per
DEC-004, this becomes the critical path.

---

## Phase Dependencies

```
┌─────────────────┐
│   Phase 1:      │
│   Foundation    │
│   (23 tickets)  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Phase 2:      │
│   Search &      │
│   Platforms     │
│   (20 tickets)  │
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌────────┐  ┌────────────────┐
│ Phase  │  │    Phase 4:    │
│   3:   │  │   Automation   │
│Bookmark│  │  & Integration │
│ & Sync │  │  (18 tickets)  │
│  (15)  │  └────────┬───────┘
└───┬────┘           │
    │                │
    └───────┬────────┘
            ▼
    ┌───────────────┐
    │   Phase 5:    │
    │   Polish &    │
    │    Launch     │
    │  (14 tickets) │
    └───────────────┘
```

**Note:** Phases 3 and 4 can run in parallel after Phase 2 is complete.

---

## Success Metrics (Target)

**Authoritative version: [PRD § Success Metrics](../PRD.md#success-metrics).**

The short version: Tome collects no telemetry, so any metric expressed as a percentage of *users*
is unmeasurable and has been removed. What remains are lab metrics measured in CI against owned
test corpora, plus public signals (GitHub, Homebrew). "User-reported bugs < 5 critical in first
month" was neither — it measures adoption as much as quality, and it goes down if nobody installs
the app.

---

## Risk Register

**Authoritative version: [`11-risk-register.md`](./11-risk-register.md).** This table previously
duplicated it with *different* impact and probability values, and included a risk ("Tantivy API
changes") that the register does not contain. Top risks by score:

| ID | Risk | Score |
|----|------|-------|
| RISK-011 | Legal/ToS exposure from scraping third-party docs | 16 Critical |
| RISK-002 | Sync correctness and data loss | 16 Critical |
| RISK-001 | Shell architecture unvalidated (Tauri/Swift) | 15 Critical |
| RISK-012 | Capacity: plan needs ~2.5 FTE, project has ~1 | 15 Critical |
| RISK-003 | Scraper maintenance burden | 12 High |
| RISK-008 | Dependency vulnerabilities | 12 High |
| RISK-009 | Scope creep | 12 High |

---

## Definition of Done (Global)

A ticket is complete when:

1. All acceptance criteria are met
2. Unit tests pass (where applicable)
3. Integration tests pass (where applicable)
4. Code review completed
5. Documentation updated (if public API changed)
6. No regressions in existing functionality
7. **Any user-facing string is externalized** (the NFR document requires i18n-readiness from v1.0;
   retrofitting string extraction across a finished UI is far more expensive than doing it as you
   go, and nothing in the ticket list enforced it)
8. **Any new external surface — HTTP route, MCP tool, CLI command, config key — is added to its
   specification document in the same PR.** Several commands and endpoints entered this plan only
   as examples in unrelated documents and were never specified.

---

## Document Index

### Phase Plans
| Document | Description |
|----------|-------------|
| [01-phase-1-foundation.md](./01-phase-1-foundation.md) | Phase 1 detailed tickets (23) |
| [02-phase-2-search-platforms.md](./02-phase-2-search-platforms.md) | Phase 2 detailed tickets (20) |
| [03-phase-3-bookmarks-sync.md](./03-phase-3-bookmarks-sync.md) | Phase 3 detailed tickets (15) |
| [04-phase-4-automation-integration.md](./04-phase-4-automation-integration.md) | Phase 4 detailed tickets (18) |
| [05-phase-5-polish-launch.md](./05-phase-5-polish-launch.md) | Phase 5 detailed tickets (14) |
| [06-dependency-map.md](./06-dependency-map.md) | Full dependency visualization |

### Supporting Documents
| Document | Description |
|----------|-------------|
| [07-technical-spikes.md](./07-technical-spikes.md) | Research tasks to de-risk unknowns |
| [08-testing-strategy.md](./08-testing-strategy.md) | Unit, integration, E2E test plans |
| [09-non-functional-requirements.md](./09-non-functional-requirements.md) | Performance, security, accessibility |
| [10-cicd-devops.md](./10-cicd-devops.md) | GitHub Actions, release process |
| [11-risk-register.md](./11-risk-register.md) | Detailed risk assessment |
| [12-security-considerations.md](./12-security-considerations.md) | Security design and threat model |
| [13-observability-plan.md](./13-observability-plan.md) | Logging, metrics (dev only) |
| [14-api-versioning-strategy.md](./14-api-versioning-strategy.md) | API and protocol versioning |
| [15-design-system.md](./15-design-system.md) | UI components and visual language |
| [16-support-maintenance.md](./16-support-maintenance.md) | Post-launch support model |
| [17-rollback-recovery.md](./17-rollback-recovery.md) | Disaster recovery procedures |

### Outside the plan set
| Document | Description |
|----------|-------------|
| [../PRD.md](../PRD.md) | Product requirements — the authoritative source for architecture, metrics, shortcuts, and config schema |
| [../reviews/2026-07-28-plan-review.md](../reviews/2026-07-28-plan-review.md) | Full critical review of this plan |
| [../decisions/](../decisions/) | Architecture decision records and the open-decision log |

### Where the single source of truth lives

Duplication was the most common defect in this plan set: the same table appeared in three
documents with three sets of values. Ownership is now explicit.

| Topic | Owner | Everyone else |
|-------|-------|---------------|
| Architecture / stack | `PRD.md` | links |
| Success metrics | `PRD.md` | links |
| Keyboard shortcuts | `PRD.md` Appendix C | links |
| Source config schema | `PRD.md` Appendix A | links |
| HTTP API + MCP surface | `PRD.md` §8–9 + `14-api-versioning-strategy.md` | links |
| Performance targets | `09-non-functional-requirements.md` | links |
| Risks | `11-risk-register.md` | links |
| Design tokens / components | `15-design-system.md` | links |
| Data locations | `PRD.md` § File System Layout | links |
| Ticket detail | phase plans `01`–`05` | links |

---

## Ticket ID Convention

```
P{phase}-{number}

Examples:
- P1-001: First ticket of Phase 1
- P2-015: Fifteenth ticket of Phase 2
```

## Complexity Estimates

| Size | Description |
|------|-------------|
| **S** | 1-2 days, isolated change |
| **M** | 3-5 days, moderate scope |
| **L** | 1-2 weeks, significant feature |
| **XL** | 2+ weeks, major system component |

---

## Changelog

| Date | Author | Change |
|------|--------|--------|
| 2026-01-25 | Claude | Initial project plan created |
| 2026-01-25 | Claude | Added supporting documents (spikes, testing, NFRs, CI/CD, etc.) |
| 2026-07-28 | Review sweep | Plan audit applied. Corrected critical path (23→15 tickets), reconciled effort against calendar and stated the FTE assumption, removed duplicated risk/metric/stack tables in favour of single owners, corrected status from "Planning Complete", added phase gate. Full findings in `../reviews/2026-07-28-plan-review.md`. |
