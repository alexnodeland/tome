# Continuation — picking up at S2-4 (ranking and boosts)

**Written:** 2026-07-29, after S2-3 merged. **Delete this file when S2-4 lands.**

This is in-flight state only: what is done, what is decided, what to do next. It deliberately
carries **no** durable knowledge — mistakes and invariants live in
[`.claude/traps.md`](traps.md), which does not go stale and is not superseded by this. The previous
continuation note was deleted precisely because it mixed the two and drifted within days; do not
let this one grow a "traps" section.

---

## Where Stage 2 is

| | Ticket | State |
|---|---|---|
| S2-1 ✅ | Relevance eval set + harness | 207 queries, 339 documents, 6 sources. #42, #43 |
| S2-2 ✅ | Tantivy integration + schema | #40 |
| S2-3 ✅ | Incremental indexing | #44 — `tome pull` indexes; `tome search` queries |
| **S2-4** | **Ranking + boosts** | **next** |
| S2-5 | Fuzzy matching | blocked on nothing; misspellings sit at 0.17 recall@1 by design |
| S2-6 | Symbol-aware search | read the `code` field finding below first |
| S2-7 | Search UI | the app still has no search box |
| S2-8..12 | in-page search, detection corpus, platform detection, scrapers, benchmarks | |

**Search works from the CLI and nowhere else.** `tome pull` indexes as it fetches and only rewrites
pages whose content hash changed; `tome search "go.mod"` returns results. There is no search UI in
the app (S2-7).

## The exit gate, honestly

Stage 2's gate is **≥ 0.90 recall@3 over ≥ 150 documents** (the document floor was added
2026-07-29 — see below). Current, from `cargo test -p tome-core --test relevance -- --nocapture`:

```
  MRR         0.7489
  recall@1    0.6377
  recall@3    0.8357      ← gate wants 0.90
  recall@10   0.9227

  by kind          n   recall@1  recall@3       MRR
  acronym          9     0.6667    0.8889    0.7870
  cross-source    18     0.5000    0.7222    0.6440
  misspelling     12     0.1667    0.4167    0.3125    ← S2-5's, expected to be bad
  natural         16     0.0625    0.2500    0.2419    ← the worst thing here
  phrase          78     0.7564    0.9487    0.8549
  symbol          74     0.7432    0.9324    0.8384
```

**Not met, and not close on recall@3.** Do not report Stage 2 as passing this gate.

---

## What S2-4 should do

The eval set already found the work. These are measured, not guessed — each came from perturbing
the ranker and reading the per-query movement report.

### 1. Long single-page documents dominate natural-language queries

`go:doc/faq` ranks **first** for nearly every "how do I …" query — it is enormous and full of
question-shaped prose, so it matches `how`, `do`, `I` heavily. `cargo:cargo/print.html`, which is
the entire Cargo book concatenated onto one page, does the same to Cargo queries.

This is why `natural` sits at 0.0625 recall@1. It is the single biggest defect in the corpus.

**Owner decision (2026-07-29): fix this in ranking only.** Do *not* add ingest-time detection of
whole-book dumps, and do not exclude pages from the library — a user who wants the FAQ should still
find it, and the crawler stays free of content policy. The levers are BM25 length normalisation
(`k1`/`b`), stopword handling for question words, and the field boosts.

`cargo/print.html` is deliberately **not** labelled as a correct answer in `queries.yaml`, even
though it contains every answer. Labelling it would make the numbers rise and hide this defect.
Leave it unlabelled.

### 2. A title boost of 3.0 is too high

Cutting it to 0.05 *improved* MRR (0.7489 → 0.7625): 17 queries got worse, 28 better. That is not
a recommendation to set 0.05 — it is evidence the current value is wrong and the real optimum is
somewhere below 3.0. Sweep it against the eval set.

### 3. The `code` field contributes almost nothing

Removing it from the query entirely is a **wash**: 13 worse, 15 better, MRR +0.005. On these
platforms method names are *also* headings, so `headers` already carries them.

**Read this before starting S2-6 (symbol-aware search)**, which would otherwise be built on the
assumption that the code field is load-bearing. It may be that the right move is to make the code
field earn its place rather than to add more machinery on top of it.

### Tuning target when categories conflict

**Owner decision (2026-07-29): optimise symbol lookup first.** `Vec::new`, `os.cpus()`,
`read_to_string` — currently 0.7432 recall@1. The reasoning is that this is what an agent asks for
over MCP, and exposing the library to coding agents is the differentiator in the PRD. Natural
language may stay weak for now; do not sacrifice symbol recall to fix it.

---

## How to work on this

```bash
cargo test -p tome-core --test relevance -- --nocapture      # the report
TOME_RELEVANCE_DUMP=1 cargo test -p tome-core --test relevance -- --nocapture   # top-5 for poor queries
TOME_UPDATE_BASELINE=1 cargo test -p tome-core --test relevance   # accept a change
git diff -- crates/tome-core/corpus/relevance/baseline.json
```

Update mode **fails the run it changes anything in**, on purpose. The passing run is the one after
the diff has been read.

**The per-query movement report is the signal; the aggregate is not.** Every perturbation measured
so far moved the aggregate by less than the margin while visibly reshuffling 15–45 queries. Read
which queries moved, not the mean.

**Boosts live in `search::schema::boost`** and are applied at query time, so changing them needs no
reindex. They are labelled as unmeasured placeholders — that label should come off when S2-4 sets
them from measurement.

---

## Decisions made 2026-07-29, not yet implemented

**Pruning: prune only after a clean crawl.** `tome pull` currently never removes pages that vanished
upstream, and the reasoning is recorded in `Database::delete_page`. The agreed policy is to delete
pages not seen this run **only when the crawl completed without errors and without hitting the page
cap** — any doubt and nothing is deleted. Needs a test that a capped or errored crawl deletes
nothing. This is not S2-4's work; it can land alongside or before, but it is a destructive path and
the guard is the whole point.

## Still open — do not decide alone

- **`two-face`** for TypeScript/TOML syntax highlighting. A licence decision, not a technical one.
- **DEC-005** docset import priority · **DEC-006** `watch` fetch vs notify · **DEC-007** note format
  · **DEC-008** export targets. All non-blocking.
- **PR #10 (TypeScript 7)** — left open deliberately as a reminder. `npm ci` fails outright:
  `svelte-check@4.7.4` peers on `typescript@^5 || ^6`. Re-check when svelte-check and
  typescript-eslint support TS 7.
- **Going public + Actions billing.** Until then CI carries no information: every run fails in ~2 s
  without executing a step. **Judge by `./scripts/check.sh`**, never by a PR's checks.
- **Playwright E2E.** PR #2 (closed) had a harness; `main` has none, and the frontend is
  Vitest-only. A real gap, but Stage 4 hardening's, against the app as it exists.

## Environment

Rust 1.96.1 · Node 26.3.0 · npm 11.16.0 · tauri-cli 2.5.0 · macOS 26.5 · arm64.
`cargo-deny` installed; `cargo-audit`, `cargo-fuzz`, and nightly are **not** (the gate says so).
384 workspace Rust tests + 66 Vitest. `npm audit` hits the live registry and fails the gate when
npmjs.org is down — that is an outage, not a finding.
