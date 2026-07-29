# Continuation — picking up at S2-5 (fuzzy matching)

**Written:** 2026-07-29, after S2-4 landed. **Rewrite or delete this file when S2-5 lands.**

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
| **S2-5** | **Fuzzy matching** | **next** — the largest remaining block of failures |
| S2-6 | Symbol-aware search | read the `code` field finding below first |
| S2-7 | Search UI | the app still has no search box |
| S2-8..12 | in-page search, detection corpus, platform detection, scrapers, benchmarks | |

**Search works from the CLI and nowhere else.**

## The exit gate, honestly

Stage 2's gate is **≥ 0.90 recall@3 over ≥ 150 documents**. Current:

```
  MRR         0.8245
  recall@1    0.7585
  recall@3    0.8744      ← gate wants 0.90
  recall@10   0.9469

  by kind          n   recall@1  recall@3       MRR
  acronym          9     0.7778    0.8889    0.8611
  cross-source    18     0.5000    0.8333    0.6796
  misspelling     12     0.2500    0.4167    0.3500    ← S2-5's, and the gap
  natural         16     0.3750    0.4375    0.4596
  phrase          78     0.9231    0.9744    0.9498
  symbol          74     0.8108    0.9459    0.8792
```

**Not met.** Do not report Stage 2 as passing this gate. The gap is ~5 queries; `misspelling` alone
holds 7 queries outside the top three, so S2-5 is the ticket that can close it.

---

## What S2-5 should do

Add fuzzy matching (P2-009) and score it with the same harness. `misspelling` is 12 labelled
queries — `enviroment variables`, `manifset format`, `list comprehention`, `excepton handling`,
`modual search path` — and they are in the corpus precisely so this ticket has a target.

Things worth knowing before starting:

- **Tantivy has `FuzzyTermQuery`** (Levenshtein distance over the term dictionary). It does not go
  through `QueryParser`, so a fuzzy path means building the query by hand rather than boosting
  fields on a parsed one. That is a bigger change to `search_with` than S2-4 was.
- **Fuzzy matching must not fire on exact matches.** `Vec` is one edit from `Vev`, `Vex`, `Vec4`…
  A typical shape is: run the exact query, and only fall back to fuzzy when it returns too few
  results. Whatever shape is chosen, `symbol` at 0.8108 recall@1 is the number that must not fall.
- **The eval will tell you if it worked, and it is now sensitive enough to.** Run the sweep and the
  gate; do not reason about fuzzy quality from examples.

### The tuning target, still standing

**Owner decision (2026-07-29): optimise symbol lookup first.** It is what an agent asks for over
MCP, and exposing the library to coding agents is the PRD's differentiator. Do not trade symbol
recall for misspelling recall.

### Read this before S2-6

**The `code` field contributes almost nothing.** Removing it from the query entirely was a wash
(13 worse, 15 better), because on these platforms method names are *also* headings, so `headers`
already carries them. S2-4 duly measured its boost down to 1.0, below `headers`. S2-6 would
otherwise be built on the assumption that the code field is load-bearing; it may be that the right
move is to make it earn its place rather than add machinery on top of it.

---

## How to work on this

```bash
cargo test -p tome-core --test relevance -- --nocapture                      # the gate + report
TOME_RELEVANCE_DUMP=1 cargo test -p tome-core --test relevance -- --nocapture   # top-5 for poor queries
cargo test -p tome-core --test relevance --release -- --ignored --nocapture sweep   # tune
TOME_UPDATE_BASELINE=1 cargo test -p tome-core --test relevance              # accept a change
git diff -- crates/tome-core/corpus/relevance/baseline.json
```

Update mode **fails the run it changes anything in**, on purpose. The passing run is the one after
the diff has been read.

**The per-query movement report is the signal; the aggregate is not.** Read which queries moved.

The sweep is `#[ignore]`d and is **not a gate** — it optimises against the eval set, so of course it
improves on it. The number that means something is what `relevance_does_not_regress` reports
afterwards against the committed baseline.

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
394 workspace Rust tests + 66 Vitest. `npm audit` hits the live registry and fails the gate when
npmjs.org is down — that is an outage, not a finding.
