# Tome Project Plan: Master Overview

**Version:** 1.0
**Created:** January 2026
**Status:** Planning Complete

---

## Executive Summary

Tome is a native macOS application providing unified, offline access to technical documentation. This document serves as the master project plan, organizing development into 5 phases with 87 detailed tickets.

### Project Metrics

| Metric | Value |
|--------|-------|
| Total Phases | 5 |
| Total Tickets | 87 |
| Critical Path Tickets | 23 |
| Estimated Duration | 30 weeks |

---

## Phase Summary

| Phase | Name | Tickets | Focus |
|-------|------|---------|-------|
| **P1** | Foundation | 22 | Core reading experience, scaffolding |
| **P2** | Search & Platforms | 18 | Full-text search, platform scrapers |
| **P3** | Bookmarks & Sync | 15 | Personal layer, iCloud sync |
| **P4** | Automation & Integration | 18 | CLI, API, MCP, Claude Code plugin |
| **P5** | Polish & Launch | 14 | Performance, UX, distribution |

---

## Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Native Shell | Swift + AppKit | Menu bar, notifications, global shortcuts |
| UI Framework | Svelte | Reactive, lightweight interface |
| Core Engine | Rust | Performance for parsing, indexing, sync |
| Doc Rendering | WKWebView | Native WebKit with typography control |
| Search Index | Tantivy | Rust-native full-text search |
| Database | SQLite | Metadata, bookmarks, sync state |
| File Storage | Filesystem | Raw doc content, organized by source |
| IPC Bridge | Tauri | Rust ↔ JavaScript communication |
| Sync | CloudKit | Native macOS iCloud integration |

---

## Critical Path

The critical path represents the minimum set of tickets that must be completed in sequence. Any delay in these tickets directly impacts project completion.

```
P1-001 → P1-002 → P1-003 → P1-004 → P1-008 → P1-012 → P1-015 → P1-018
    ↓
P2-001 → P2-002 → P2-004 → P2-005 → P2-010
    ↓
P3-001 → P3-005 → P3-008 → P3-012
    ↓
P4-001 → P4-005 → P4-009 → P4-013
    ↓
P5-001 → P5-007 → P5-012 → P5-014
```

---

## Phase Dependencies

```
┌─────────────────┐
│   Phase 1:      │
│   Foundation    │
│   (22 tickets)  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Phase 2:      │
│   Search &      │
│   Platforms     │
│   (18 tickets)  │
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

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Cold search latency | < 100ms | P95 across 10+ sources |
| Index build time | < 30s for 1000 pages | Benchmark suite |
| Memory usage | < 200MB idle | Activity Monitor |
| Sync reliability | > 99.5% | Automated sync tests |
| User-reported bugs | < 5 critical in first month | GitHub issues |

---

## Risk Register

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Tauri/Swift integration complexity | High | Medium | Prototype early in P1, consider fallbacks |
| iCloud sync edge cases | Medium | High | Extensive testing matrix, conflict resolution |
| Platform-specific scraper maintenance | Medium | High | Modular scraper architecture, fallback to generic |
| Performance with large doc sets | High | Medium | Incremental indexing, lazy loading from P1 |
| Tantivy API changes | Low | Low | Pin dependency versions |

---

## Definition of Done (Global)

A ticket is complete when:

1. All acceptance criteria are met
2. Unit tests pass (where applicable)
3. Integration tests pass (where applicable)
4. Code review completed
5. Documentation updated (if public API changed)
6. No regressions in existing functionality

---

## Document Index

| Document | Description |
|----------|-------------|
| [01-phase-1-foundation.md](./01-phase-1-foundation.md) | Phase 1 detailed tickets |
| [02-phase-2-search-platforms.md](./02-phase-2-search-platforms.md) | Phase 2 detailed tickets |
| [03-phase-3-bookmarks-sync.md](./03-phase-3-bookmarks-sync.md) | Phase 3 detailed tickets |
| [04-phase-4-automation-integration.md](./04-phase-4-automation-integration.md) | Phase 4 detailed tickets |
| [05-phase-5-polish-launch.md](./05-phase-5-polish-launch.md) | Phase 5 detailed tickets |
| [06-dependency-map.md](./06-dependency-map.md) | Full dependency visualization |

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
