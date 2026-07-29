# Continuation — picking up at S2-7 (search UI)

**Written:** 2026-07-29, after S2-6 landed. **Rewrite or delete this file when S2-7 lands.**

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
| **S2-7** | **Search UI** | **next** — the app still has no search box |
| S2-8..12 | in-page search, detection corpus, platform detection, scrapers, benchmarks | |

**Search works from the CLI and nowhere else.**

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

## What S2-7 should do

The search UI (P2-004/005/008/016/017): a search box in the app, a results list, scoping, history
and keyboard handling. Everything below it already works and is measured — this is the first Stage 2
ticket that is mostly frontend.

Things the backend already gives you, and which the UI should not reinvent:

- **`SearchEngine::search`** returns `Hit { source, path, title, score, symbol_kind }`. The
  `symbol_kind` is `Some(Function | Type | Trait | Module | Constant | Macro)` for a reference page
  and `None` for prose — it is what lets a result list show `Vec [type]` without opening anything.
- **`SearchEngine::suggest`** is "did you mean?", and costs ~2 µs on a correctly spelled query.
  Show it; a search that silently answers a different question is worse than one that says what it
  did.
- **`@symbol`** already works in the query string. The UI should not need to parse it.
- **Snippets (P2-005) cannot use Tantivy's `SnippetGenerator`** — it needs a stored field and the
  schema deliberately stores no body. Re-read the page from `PageStore` and highlight there, which
  is the better place anyway: the store holds structured nodes, so a snippet can respect block
  boundaries instead of slicing raw text.
- **Highlighting is a render concern, not an AST mutation**, and emits CSS classes rather than
  colours, so a theme change needs no re-highlighting.

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
435 workspace Rust tests + 66 Vitest. `npm audit` hits the live registry and fails the gate when
npmjs.org is down — that is an outage, not a finding.
