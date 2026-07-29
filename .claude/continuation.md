# Continuation — picking up at S2-9 (detection corpus)

**Written:** 2026-07-29, after S2-8 landed. **Rewrite or delete this file when S2-9 lands.**

In-flight state only: what is done, what is decided, what to do next. It deliberately carries **no**
durable knowledge — mistakes and invariants live in [`.claude/traps.md`](traps.md), which does not go
stale. Do not let this file grow a "traps" section; an earlier one was deleted for exactly that.

---

## Where Stage 2 is

| | Ticket | State |
|---|---|---|
| S2-1 ✅ | Relevance eval set + harness | 207 queries, 339 documents, 6 sources |
| S2-2 ✅ | Tantivy integration + schema | |
| S2-3 ✅ | Incremental indexing | `tome pull` indexes; `tome search` queries |
| S2-4 ✅ | Ranking + boosts | `search/ranking.rs`, tuned by measured sweep |
| S2-5 ✅ | Fuzzy matching | `search/fuzzy.rs` — query correction, not `FuzzyTermQuery` |
| S2-6 ✅ | Symbol-aware search | `search/symbols.rs` — from headings, not code blocks; `@symbol` |
| S2-7 ✅ | Search UI | `SearchModal.svelte` + `src-tauri/src/search.rs` + `search/snippet.rs` |
| S2-8 ✅ | In-page search (⌘F) | `FindBar.svelte` + find in `public/reader-frame.js` |
| **S2-9** | **Detection corpus + harness** | **next** — the gate S2-10 is scored against |
| S2-10..12 | platform detection, scrapers, benchmarks | |

**Search works from the CLI and in the app.** ⌘K opens the modal; ⌘F finds within the open page.

## The relevance gate, honestly

Stage 2's relevance gate is **≥ 0.90 recall@3 over ≥ 150 documents**. Current:

```
  MRR         0.8374
  recall@1    0.7585
  recall@3    0.9082      ← gate wants 0.90. Met, not comfortably met.
  recall@10   0.9662

  by kind          n   recall@1  recall@3       MRR
  acronym          9     0.5556    1.0000    0.7593
  cross-source    18     0.5000    0.7778    0.6750
  misspelling     12     0.5000    0.7500    0.6389
  natural         16     0.3750    0.5000    0.4633    ← weakest
  phrase          78     0.8974    0.9872    0.9395
  symbol          74     0.8243    0.9595    0.8919
```

**The relevance number is met; Stage 2 is not done.** The exit gate also requires the search *UI*
and the P2-018 benchmark (< 100 ms on the benchmark corpus), and neither exists. S2-7..S2-12 remain.

**Do not close future gaps by picking the parameter that crosses a threshold.** During S2-5 a
neighbour reached 0.9034 and was rejected for exactly that reason; S2-6 then cleared the gate with a
configuration that won on a different objective and dominated on every column. Re-labelling queries
to raise the number is worse — it destroys the one instrument here that cannot be rebuilt in
seconds. Differences of one or two queries are inside the noise.

---

## What S2-9 should do

The **detection corpus and harness** (P2-020): labelled fixtures of real documentation pages, and a
test that scores platform detection against them. It is to S2-10 exactly what S2-1's relevance eval
was to S2-4/5/6 — the thing that makes the next ticket measurable instead of an opinion.

**Build it before S2-10, not after.** That ordering was not negotiable for search and is not here.
Every measured finding in Stage 2 so far came from a harness that existed before the code it
scored: the query-parser defect, the code-block symbol defect, the "all declarations" regression.
Each was invisible to inspection and obvious to the corpus.

Two lessons from S2-1 that transfer directly:

- **A corpus too small cannot discriminate.** At 26 documents, removing an entire indexed field
  moved MRR by 0.0036 and tripped nothing; the metrics compressed near the top because there was
  rarely a strong wrong answer to beat. Size the detection corpus so a *wrong* detector visibly
  fails it, and prove that by perturbing a working one.
- **Only pages whose licence permits alteration and redistribution may be committed** (the
  SPIKE-010 gate), each recorded in a `SOURCES.md`. `corpus/relevance/pages/SOURCES.md` is the
  pattern.

S2-11's four scrapers are the canonical fan-out — four parallel implementations, one interface, all
scored against this corpus — so the corpus is on the critical path for the largest remaining chunk
of Stage 2.

### The tuning target, still standing
### The tuning target, still standing
### The tuning target, still standing
### The tuning target, still standing

**Owner decision (2026-07-29): optimise symbol lookup first.** It is what an agent asks for over
MCP, and exposing the library to coding agents is the PRD's differentiator. Do not trade symbol
recall for misspelling recall.

### What is weakest now

`natural` (0.4633 MRR), and it is a genuine tension rather than a bug: the pages that answer a
"how do I …" question in prose are the enormous single-page ones (`go:doc/faq`,
`cargo:cargo/print.html`) that the length penalty exists to demote. Closing it probably needs
passage-level retrieval, not another boost. Not S2-7's.

`cross-source` (0.6750) is second and is partly a labelling artefact: a query three platforms answer
equally well has three right answers, and the labels name a subset.

---

## How to work on this

```bash
cargo test -p tome-core --test relevance -- --nocapture                      # the gate + report
TOME_RELEVANCE_DUMP=1 cargo test -p tome-core --test relevance -- --nocapture   # top-5 for poor queries
cargo test -p tome-core --test relevance --release -- --ignored --nocapture sweep   # tune
cargo test -p tome-core --test relevance --release -- --ignored --nocapture fuzzy_cost  # latency
cargo test -p tome-core --test relevance --release -- --ignored --nocapture symbol_extraction  # symbols
TOME_UPDATE_BASELINE=1 cargo test -p tome-core --test relevance              # accept a change
git diff -- crates/tome-core/corpus/relevance/baseline.json
```

Update mode **fails the run it changes anything in**, on purpose. The passing run is the one after
the diff has been read.

**The per-query movement report is the signal; the aggregate is not.** Read which queries moved.

The sweep is `#[ignore]`d and is **not a gate** — it optimises against the eval set, so of course it
improves on it. The number that means something is what `relevance_does_not_regress` reports
afterwards against the committed baseline.

Coordinate descent is greedy and path-dependent, so the sweep also prints a **one-step
neighbourhood of `Ranking::TUNED`** in full columns. That is the table to read when choosing
between two configurations, because the choice is usually a judgement about which category to
favour rather than a single number going up.

---

## Decisions made 2026-07-29, not yet implemented

**Pruning: prune only after a clean crawl.** `tome pull` never removes pages that vanished upstream.
The agreed policy — delete pages not seen this run **only when the crawl completed without errors
and without hitting the page cap** — is recorded on `Database::delete_page`. Needs a test that a
capped or errored crawl deletes nothing. Not S2-5's work; it is a destructive path and that guard is
the whole point.

## Still open — do not decide alone

- **`two-face`** for TypeScript/TOML syntax highlighting. A licence decision, not a technical one.
- **DEC-005** docset import priority · **DEC-006** `watch` fetch vs notify · **DEC-007** note format
  · **DEC-008** export targets. All non-blocking.
- **PR #10 (TypeScript 7)** — left open deliberately as a reminder. `npm ci` fails outright:
  `svelte-check@4.7.4` peers on `typescript@^5 || ^6`. Re-check when svelte-check and
  typescript-eslint support TS 7.
- **Going public + Actions billing.** Until then CI carries no information: every run fails in ~2 s
  without executing a step. **Judge by `./scripts/check.sh`**, never by a PR's checks.
- **Playwright E2E.** `main` has none; the frontend is Vitest-only. A real gap, but Stage 4
  hardening's, against the app as it exists.

## Environment

Rust 1.96.1 · Node 26.3.0 · npm 11.16.0 · tauri-cli 2.5.0 · macOS 26.5 · arm64.
`cargo-deny` installed; `cargo-audit`, `cargo-fuzz`, and nightly are **not** (the gate says so).
459 workspace Rust tests + 136 Vitest. `npm audit` hits the live registry and fails the gate when
npmjs.org is down — that is an outage, not a finding.
