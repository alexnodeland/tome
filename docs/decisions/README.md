# Decisions

Two kinds of record live here.

- **DEC-*** — open decisions. Things not yet decided, with what each one blocks. When a DEC is
  resolved it becomes an ADR and is struck from the table below.
- **ADR-*** — architecture decision records. Decisions that *have* been made, with the reasoning
  and the alternatives, so that in a year the question "why is it like this?" has an answer better
  than "it always was".

Both exist because this plan previously contained several decisions that had been made implicitly —
scattered across documents, contradicting each other, with no record of who chose or why.

---

## Open decisions

| ID | Decision | Blocks | Notes |
|----|----------|--------|-------|
| **DEC-001** | Licence: MIT vs Apache-2.0 | Any public release; accepting contributions | No `LICENSE` file exists. The Phase-5 landing-page copy asserted MIT while the NFR document said "TBD". Apache-2.0 adds an explicit patent grant; MIT is shorter and more common in this space. Either is fine — **not choosing is not.** |
| **DEC-002** | Bundle identifier and domain | Notarization, iCloud container, Keychain service, Homebrew zap | `com.example.tome` is currently threaded through the release pipeline. Needs a domain you control. |
| **DEC-003** | Funding model | Distribution | The Apple Developer Program is $99/yr and is required for **both** notarization and iCloud. This is a standing cost, not a one-off task, and nothing in the plan accounted for it. |
| **DEC-004** | Team size | **Every date in the roadmap** | The plan is ~381 person-days against 30 weeks ≈ 2.5 FTE. `06-dependency-map.md` assumes three developers; RISK-010 assumes one. See below. |
| **DEC-005** | Dash docset import: v1.0 or v1.2? | Cold-start strategy | Currently v1.2. Importing existing docsets may be the cheapest possible answer to an empty library (RISK-013) and is worth reconsidering. |
| **DEC-006** | `watch` strategy: fetch eagerly or notify? | P4-018 | Leaning **notify**, to honour "no background network activity the user did not configure". |
| **DEC-007** | Annotation notes: plain text or Markdown? | P3-008 | Leaning Markdown, rendered read-only (rendering user Markdown in the reader frame must not become an injection path). |
| **DEC-008** | Export destinations | Post-v1 | Plain Markdown files first — they satisfy most of Obsidian and Notion without an integration. |

### DEC-004 in more detail

This is the decision that changes the most, so it deserves more than a table row.

| Scenario | Duration | Recommended scope |
|----------|----------|-------------------|
| ~2.5 engineers | 30 weeks | Full plan as written |
| 1 engineer, full time | ~77 weeks | Cut to P1 + P2 + MCP portion of P4 |
| 1 engineer, part time | 3–4 years | Do not attempt the full plan — scrapers will rot faster than they are fixed (RISK-003) |

The recommended cut retains both genuinely differentiated features (arbitrary-site ingestion, agent
access) and drops Phase 3 — 68 person-days, the highest-scoring technical risk, and the least
painful loss, since bookmarks still work locally. **This is a scope decision, not a schedule
decision**; extending the calendar does not fix a capacity problem.

---

## Accepted decisions

Recorded as ADRs. Both were resolved during the 2026-07-28 plan review, because leaving three
contradictory answers in the documents was worse than choosing — and in each case the plan's own
material pointed at the answer.

| ID | Decision | Status |
|----|----------|--------|
| [ADR-0001](./0001-file-based-icloud-sync.md) | Sync via an iCloud Drive container, not CloudKit | Accepted |
| [ADR-0002](./0002-no-app-sandbox.md) | Ship without App Sandbox; Developer ID + hardened runtime | Accepted |

---

## Template

```markdown
# ADR-NNNN: <short title>

**Status:** Proposed | Accepted | Superseded by ADR-MMMM
**Date:** YYYY-MM-DD
**Deciders:** <who>

## Context
What forces are at play? What makes this hard? What did we know at the time?

## Decision
What we are doing, stated so someone can act on it.

## Alternatives considered
Each with why it lost. An ADR with no rejected alternatives is not recording a decision.

## Consequences
What gets better, what gets worse, and what we are now committed to.

## Reversibility
How expensive is it to change our mind later, and what would trigger that?
```
