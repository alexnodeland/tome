# Continuation — Stage 2 is complete; Stage 3 is next

**Written:** 2026-07-29, after S2-12 landed. **Rewrite or delete this file when Stage 3 starts.**

In-flight state only: what is done, what is decided, what to do next. It deliberately carries **no**
durable knowledge — mistakes and invariants live in [`.claude/traps.md`](traps.md), which does not go
stale. Do not let this file grow a "traps" section; an earlier one was deleted for exactly that.

---

## Stage 2 is done

| | Ticket | Landed as |
|---|---|---|
| S2-1 ✅ | Relevance eval set + harness | 207 queries, 339 documents, 6 sources |
| S2-2 ✅ | Tantivy integration + schema | `search/schema.rs`, `tokenizer.rs`, `extract.rs` |
| S2-3 ✅ | Incremental indexing | `pipeline::index_source`, wired into `pull` |
| S2-4 ✅ | Ranking + boosts | `search/ranking.rs`, tuned by measured sweep |
| S2-5 ✅ | Fuzzy matching | `search/fuzzy.rs` — query correction, not `FuzzyTermQuery` |
| S2-6 ✅ | Symbol-aware search | `search/symbols.rs` — from headings, not code blocks |
| S2-7 ✅ | Search UI | `SearchModal.svelte`, `src-tauri/src/search.rs`, `search/snippet.rs` |
| S2-8 ✅ | In-page search (⌘F) | `FindBar.svelte` + find in `public/reader-frame.js` |
| S2-9 ✅ | Detection corpus + harness | 128 homepages, confusion matrix |
| S2-10 ✅ | Platform detection | `detect.rs` — 0.9922 accuracy, zero confident errors |
| S2-11 ✅ | Scrapers + man pages | `scrape.rs` profiles, `man.rs` |
| S2-12 ✅ | Benchmarks | `tests/search_bench.rs` |

**The exit gate is met.** Relevance **0.9082 recall@3** over 339 documents (target ≥ 0.90), search
**P95 158 µs** (budget 100 ms).

Read the first number as *met*, not comfortable: it is 188 queries of 207, and one query either way
moves it across the line. The second has ~600× headroom.

## What Stage 2 did not build

Not omissions to discover later — each is marked `[~]` in its ticket with the reason:

- **A visible relevance score** and **grouping by source** in the results list. A BM25 score is not
  a percentage; grouping fights the single cross-source ranking S2-4/5/6 exist to produce.
- **Category scope** and a **scope-change shortcut** — Appendix C allocates no chord, and
  allocating one ad hoc is how the shortcut table drifted into four contradictory copies before.
- **`search-index.js`, `SUMMARY.md` and `book.toml` parsing.** None of them is served; only the
  rendered HTML is.
- **Memory measurement in the benchmark.** SPIKE-003 measured peak RSS at 100 000 pages with a
  purpose-built harness; a figure taken at 339 pages is a number without a meaning.
- **A committed timing baseline.** A cross-machine wall-clock baseline fails on a slower laptop
  while hiding real regressions. The benchmark gates on an absolute threshold instead.
- **GitBook fixtures.** It is a hosted product with no redistributable public instances; the
  confusion matrix prints an empty row rather than a score.

## What is weakest in what *was* built

- **`natural` queries** (0.4633 MRR) are the weakest search category, and it is a genuine tension:
  the pages that answer "how do I …" in prose are the enormous single-page ones the length penalty
  exists to demote. Closing it probably needs passage-level retrieval, not another boost.
- **Two `misspelling` queries are unreachable** under P2-009's distance schedule (`modual` →
  `module`, `pth` → `path`). Widening it is a specification change, not a tuning decision.
- **The Sphinx and mdBook scraping profiles are unmeasured** — nothing in the 26-page golden corpus
  exercises them. They encode real markup, but "kept" is not "verified".
- **One detection fixture is unclassifiable** from a homepage (`docs.djangoproject.com`, Sphinx with
  no Sphinx marker anywhere). It falls back to `Generic` at low confidence, which is correct, and
  caps achievable accuracy at 127/128.

---

## Stage 3

Read [`docs/plans/18-implementation-plan.md`](../docs/plans/18-implementation-plan.md) for the
stage's own entry gate, ticket list and ordering — it is the execution plan and it owns that.

Two things from this stage that should carry into the next:

**Build the harness before the thing it scores.** Every measured finding in Stage 2 came out of
that order and none was visible to inspection: the query-parser defect that killed twelve symbol
queries, the code-block symbol extraction that found `main`/`buf`/`foo`, the all-declarations
ranking regression, and three of my own corpus labels being wrong. Each looked fine in review.

**Do not close a gate by picking the parameter that crosses it.** During S2-5 a configuration
reached 0.9034 against the 0.90 target and was rejected because it was chosen for crossing;
S2-6 then cleared it at 0.9082 with a configuration that won on a different objective and dominated
its predecessor on every column. Re-labelling corpus entries to raise a number is worse still — it
destroys the one instrument here that cannot be rebuilt in seconds.

## How to run the gates

```bash
./scripts/check.sh                      # everything, including the app bundle
./scripts/check.sh --fast               # everything except the bundle

cargo test -p tome-core --test relevance -- --nocapture          # search quality
cargo test -p tome-core --test detection -- --nocapture          # platform detection
cargo test -p tome-core --test search_bench --release -- --nocapture   # latency

# tuning and measurement tools, all #[ignore]d
cargo test -p tome-core --test relevance --release -- --ignored --nocapture sweep
cargo test -p tome-core --test relevance --release -- --ignored --nocapture fuzzy_cost
cargo test -p tome-core --test relevance --release -- --ignored --nocapture symbol_extraction
cargo test -p tome-core --test search_bench --release -- --ignored --nocapture
```

Update modes — `TOME_UPDATE_GOLDEN`, `TOME_UPDATE_BASELINE`,
`TOME_UPDATE_DETECTION_BASELINE` — all **fail the run they change anything in**, on purpose. The
passing run is the one after the diff has been read.

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
- **Pruning after a clean crawl.** Agreed 2026-07-29, recorded on `Database::delete_page`, still
  unimplemented. Needs a test that a capped or errored crawl deletes nothing.
- **`tome add` (P1-022) does not exist**, so platform detection has no user-facing consumer yet.

## Environment

Rust 1.96.1 · Node 26.3.0 · npm 11.16.0 · tauri-cli 2.5.0 · macOS 26.5 · arm64.
`cargo-deny` and `mandoc` present; `cargo-audit`, `cargo-fuzz`, and nightly are **not** (the gate
says so). 506 workspace Rust tests + 136 Vitest. `npm audit` hits the live registry and fails the
gate when npmjs.org is down — that is an outage, not a finding.
