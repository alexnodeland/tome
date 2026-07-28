# Tome

**A personal library for technical documentation.** Point it at any documentation site — ReadTheDocs,
rustdoc, mdBook, man pages, or a plain HTML site — and read it offline in a single, well-typeset
place, with fast search across everything, bookmarks that follow you between machines, and an MCP
server so your coding agent can read your docs too.

macOS, Apple Silicon. Local-first. Open source.

---

## Status: planning. No code yet.

This repository currently contains **specifications, not software**. Nothing has been built, no
technical spike has been run, and several foundational decisions are still open.

If you are here to use Tome: there is nothing to install yet.
If you are here to understand or shape it: start below.

| | |
|---|---|
| **What it should be** | [`docs/PRD.md`](docs/PRD.md) — product requirements, architecture, API and CLI surface |
| **How it would be built** | [`docs/plans/`](docs/plans/) — 90 tickets across 5 phases, plus testing, security, CI/CD, design system, risk register |
| **What's wrong with the plan** | [`docs/reviews/2026-07-28-plan-review.md`](docs/reviews/2026-07-28-plan-review.md) — a full critical review |
| **What's undecided** | [`docs/decisions/`](docs/decisions/) — open decisions and ADRs |

**Start with the review.** It is the shortest path to understanding both the plan and its gaps.

---

## Why this exists

Documentation is scattered across browser tabs, terminal `man` pages, and IDE hover cards. Each
source looks different, searches differently, and disappears when you close the tab.

Dash and DevDocs solve much of this well — with large, curated catalogs, offline support, and years
of polish. Tome is not trying to beat them at that. It is aimed at two things they do not do:

1. **Ingest arbitrary documentation sites**, including your company's internal docs and that one
   library whose docs nobody has packaged, and give them the same reading experience as everything
   else.
2. **Expose your documentation library to coding agents** over MCP, so an assistant working on your
   code can read the actual docs for the actual version you have pinned.

Everything else — the typography, the search, the bookmarks — is table stakes that has to be good
enough not to be the reason you go back to browser tabs.

---

## Design commitments

These are constraints, not aspirations. Where the plan violated one, the plan was changed.

- **Local-first.** Your documentation and reading history live on your machine. iCloud sync is
  optional and carries bookmarks only, never content.
- **No telemetry. None.** No analytics, no crash reporting, no phone-home, not even opt-in. The
  cost of this is real and stated plainly: there is no usage data, so product direction comes from
  conversations rather than dashboards.
- **Offline means offline.** Images and assets are fetched at sync time and rewritten to local
  paths; opening a page never touches the network. The reader's content-security policy enforces
  this rather than trusting it.
- **A good citizen on the web.** Tome fetches other people's documentation. It obeys `robots.txt`,
  rate-limits itself, identifies itself honestly, caches for one user, links back to the origin,
  and **never redistributes**. The source registry ships configuration files, never content.
- **Programmable.** A local HTTP API and an MCP server, both authenticated, both off by default.

---

## Open decisions

Recorded in [`docs/decisions/`](docs/decisions/). The ones that block progress:

| ID | Decision | Blocks |
|----|----------|--------|
| **DEC-001** | Licence — MIT or Apache-2.0 | **Any public release.** There is no `LICENSE` file yet. |
| **DEC-002** | Bundle identifier and domain | Notarization, iCloud container, Keychain |
| **DEC-003** | Funding — the Apple Developer Program is $99/yr and mandatory for notarization | Distribution |
| **DEC-004** | Team size | Every date in the roadmap. The plan assumes ~2.5 engineers. |

> **On DEC-004.** The plan totals ~381 person-days against a 30-week calendar. Solo, that is closer
> to 77 weeks. If this stays a one-person project, the recommended cut is Phase 1 + Phase 2 + the
> MCP portion of Phase 4 — about 55% of the work, keeping both differentiated features and dropping
> cross-device sync, which is the largest and riskiest piece.

---

## Repository layout

```
docs/
├── PRD.md                    product requirements — the authoritative specification
├── plans/                    phase plans (01-05), dependency map, and supporting documents
│   ├── 00-project-overview.md    start here for the plan
│   ├── 01..05-phase-*.md         90 tickets with acceptance criteria
│   ├── 06-dependency-map.md      graph, critical path, parallelization
│   ├── 07-technical-spikes.md    the questions that must be answered first
│   ├── 08..17-*.md               testing, NFRs, CI/CD, risk, security, observability,
│   │                             versioning, design system, support, recovery
├── decisions/                open decisions and architecture decision records
└── reviews/                  point-in-time reviews of the plan
```

Each shared fact has exactly one owning document — see the ownership table in
[`docs/plans/00-project-overview.md`](docs/plans/00-project-overview.md). Please link rather than
restate; the plan previously drifted badly because the same table lived in three places.

---

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). While the project is pre-implementation, the most useful
contributions are:

- **Disagreement with the plan.** Especially about the architecture, the scope, or whether the
  product is worth building at all. Open a discussion.
- **Answers to the open decisions above.**
- **Running a spike** from [`docs/plans/07-technical-spikes.md`](docs/plans/07-technical-spikes.md).
  Each is one to three days and unblocks a phase.

Security issues: see [`SECURITY.md`](SECURITY.md) — please do not open a public issue.

---

## Licence

**Not yet chosen** (DEC-001). Until a `LICENSE` file exists, no licence is granted. This is being
resolved before anything is published.
