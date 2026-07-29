# Continuation — picking up at S2-6 (symbol-aware search)

**Written:** 2026-07-29, after S2-5 landed. **Rewrite or delete this file when S2-6 lands.**

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
| **S2-6** | **Symbol-aware search** | **next** — read the `code` field finding below first |
| S2-7 | Search UI | the app still has no search box |
| S2-8..12 | in-page search, detection corpus, platform detection, scrapers, benchmarks | |

**Search works from the CLI and nowhere else.**

## The exit gate, honestly

Stage 2's gate is **≥ 0.90 recall@3 over ≥ 150 documents**. Current:

```
  MRR         0.8351
  recall@1    0.7585
  recall@3    0.8986      ← gate wants 0.90. One query of 207.
  recall@10   0.9662

  by kind          n   recall@1  recall@3       MRR
  acronym          9     0.7778    0.8889    0.8611
  cross-source    18     0.5000    0.8333    0.6704
  misspelling     12     0.5000    0.7500    0.6333
  natural         16     0.3125    0.5000    0.4273    ← now the weakest
  phrase          78     0.8974    0.9744    0.9405
  symbol          74     0.8108    0.9459    0.8817
```

**Not met.** Do not report Stage 2 as passing.

**And do not close it by picking the parameter that crosses it.** `length_penalty` 0.2 reaches
0.9034 and costs `symbol` MRR (0.8817 → 0.8606). That is fitting the gate, not passing it, and the
owner ranked symbol first. Re-labelling queries to make the number rise is worse: it destroys the
one instrument here that cannot be rebuilt in seconds.

---

## What S2-6 should do

Symbol-aware search (P2-015), scored by the same harness. `symbol` is 74 of the 207 queries and
sits at 0.8108 recall@1 — the owner's stated priority, and the PRD's differentiator, because it is
what an agent asks for over MCP.

**Read this first, it is the whole context for the ticket.** The `code` field contributes almost
nothing. Removing it from the query entirely was a wash (13 worse, 15 better), and S2-4 duly
measured its boost down — the neighbourhood check says dropping it to 0.5 is *better* than 1.0 on
MRR. The reason is that on these platforms method names are **also headings**, so `headers` already
carries them.

So the tempting shape for S2-6 — more machinery layered on the code field — is building on
something the measurements say is not load-bearing. The alternative worth weighing is making the
code field earn its place: index symbols as their own field with their own extraction (declarations
rather than every token in every code block), rather than boosting a field that currently holds
undifferentiated code text.

Whatever shape is chosen, the harness will say whether it worked. Run the sweep and the gate; do
not reason about symbol quality from examples.

### The tuning target, still standing

**Owner decision (2026-07-29): optimise symbol lookup first.** It is what an agent asks for over
MCP, and exposing the library to coding agents is the PRD's differentiator. Do not trade symbol
recall for misspelling recall.

### What is weakest now

`natural` (0.4273 MRR), and it is a genuine tension rather than a bug: the pages that answer a
"how do I …" question in prose are the enormous single-page ones (`go:doc/faq`,
`cargo:cargo/print.html`) that the length penalty exists to demote. Closing it probably needs
passage-level retrieval, not another boost. Not S2-6's.

---

## How to work on this

```bash
cargo test -p tome-core --test relevance -- --nocapture                      # the gate + report
TOME_RELEVANCE_DUMP=1 cargo test -p tome-core --test relevance -- --nocapture   # top-5 for poor queries
cargo test -p tome-core --test relevance --release -- --ignored --nocapture sweep   # tune
cargo test -p tome-core --test relevance --release -- --ignored --nocapture fuzzy_cost  # latency
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
412 workspace Rust tests + 66 Vitest. `npm audit` hits the live registry and fails the gate when
npmjs.org is down — that is an outage, not a finding.
