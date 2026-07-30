# Implementation Plan

**Build model:** solo maintainer directing Fable + Opus agent workflows
**Created:** 2026-07-28
**Supersedes:** the phase *sequencing* in `00-project-overview.md`. Ticket detail in `01`–`05`
remains the specification; this document changes the order they are executed in and how they are
verified.

---

## Why this is not just the old plan with dates

The original plan is sized in person-days and sequenced horizontally: build all of the storage
layer, then all of the scraping layer, then all of the UI. That is the right shape when the
constraint is **how long a human takes to type correct code**.

That is not the constraint here.

With agents doing the implementation, three things change, and they change the plan structurally:

| | Human-typed | Agent-written |
|---|---|---|
| **Scarce resource** | Time to write code | **Confidence that the code is right** |
| **Cost of parallelism** | High — needs more people | Near zero — needs more *interfaces* |
| **Cost of a wrong turn** | Noticed while typing | Noticed much later, after it has propagated |
| **What review catches** | Most defects | Only defects you can still afford to read for |

**The binding constraint becomes verification bandwidth.** An agent will produce 400 lines of
confident, plausible, well-formatted Rust for the SSRF filter in two minutes. Whether it is correct
is a separate question, and reading it carefully costs you roughly what writing it would have. If
verification is manual, agents buy you nothing and cost you vigilance.

So the whole plan reorganizes around one rule:

> **Nothing gets implemented before the thing that proves it works.**

That single inversion drives most of what follows: fixture corpora before scrapers, eval sets
before ranking, property tests before sync, the path module before anything that touches disk.

### The second inversion: interfaces before implementations

Parallelism is cheap, but only across **stable boundaries**. Ten agents editing overlapping code
produce merge conflicts and incoherent abstractions. Ten agents implementing ten traits that were
frozen beforehand produce ten working modules.

Every stage below therefore has the same internal shape:

```
freeze the interface  →  fan out implementations  →  adversarially verify  →  integrate
   (Opus, serial)         (Fable, parallel)          (Opus, parallel)         (serial)
```

### What this does *not* change

Effort estimates in `01`–`05` are kept as-is. They are still the honest human-equivalent size of
each ticket, and they remain the best available proxy for *complexity* — which is what determines
how much verification a ticket needs. A 7.5-day ticket does not become a 20-minute ticket because
an agent writes it; it becomes a 20-minute *draft* that needs a 7.5-day-ticket's worth of scrutiny,
most of which should be automated.

---

## The gate is local until the repository goes public

The plan leans on machine-checked gates. **GitHub Actions is unavailable**: the repository is
private until release, and Actions is currently blocked at the account level, so every workflow
run fails in two seconds without executing a step.

That does not weaken the rule, it relocates it. **`./scripts/check.sh` is the gate.** It runs
exactly what `.github/workflows/ci.yml` runs — formatting, clippy `-D warnings`, Rust tests,
svelte-check, eslint, prettier, vitest, `npm audit`, `cargo-deny`, `cargo-audit`, and the app
build — and it must pass before anything is committed or merged.

```bash
./scripts/check.sh          # everything
./scripts/check.sh --fast   # skip the app build
```

Two consequences worth holding onto:

- **The script and the workflow must not drift.** If they do, the first CI run after going public
  will fail on things that passed locally for months — which is precisely the failure this
  arrangement exists to avoid. Changing one means changing the other, in the same commit.
- **Some checks genuinely cannot run locally**: the hosted actions (`rustsec/audit-check`,
  `cargo-deny-action`, `gitleaks`) and the clean-runner build. Treat the first green CI run after
  going public as a real gate, not a formality, and expect it to find something.

## Stage structure

Six stages. Each has an entry gate that must pass before work starts and an exit gate that must
pass before the next begins. Gates are machine-checked wherever possible.

```
S0  Foundations         make agent output verifiable at all
S1  Vertical slice      one real docs site, fetched → rendered → offline
S2  Search              plus the eval set that says whether it is any good
S3  Agent access        CLI + MCP — the differentiated feature
S4  Hardening & release notarized, distributed, documented
S5  Sync                deferred; only if S1–S4 shipped and users asked
```

**Why a vertical slice before breadth.** The riskiest assumption in this product is not "can we
build a scraper" — it is **"can arbitrary documentation sites be normalized into one consistent
reading experience?"** If that is false, or only 60% true, everything downstream is worth less. The
original Phase 1 would not answer it until week 8. S1 answers it in the first working chunk, on
real sites, and the answer is a rendered page you can look at.

---

## Ticket format for agent execution

Existing tickets in `01`–`05` are specifications. Before an agent runs one, it is restated in this
shape. The additions are the last four fields, and they are the ones that make the difference.

```markdown
### T-042 · Implement the URL filter

**Spec:** P1-011
**Contract:** `pub struct UrlFilter { … }` with `fn matches(&self, url: &Url) -> bool`
              — frozen in `crates/tome-core/src/scraper/mod.rs`, do not change it
**Files:** `crates/tome-core/src/scraper/filter.rs` (create), `filter/tests.rs` (create)
            Touch nothing else.

**Done when:**
  - [ ] `cargo test -p tome-core scraper::filter` passes
  - [ ] `cargo clippy -- -D warnings` clean
  - [ ] The table-driven cases in `fixtures/url-filter-cases.toml` all pass
  - [ ] No new dependency added

**Verification:** table-driven tests (committed BEFORE this ticket runs)
**Model:** Fable — mechanically specified, fully covered by fixtures
**Isolation:** none — single new file, no shared state
**Parallel with:** T-040, T-041, T-043
```

Four rules that make this work, each learned from a specific way agent work goes wrong:

1. **State the contract, don't describe it.** "Implement a filter that matches URLs" invites the
   agent to invent a signature that then conflicts with three other tickets. Paste the exact
   signature; it was frozen in the interface step for this reason.
2. **Name the files it may touch.** Unscoped agents refactor adjacent code helpfully and
   destructively. Scope is a correctness control, not tidiness.
3. **"Done when" must be a command, not a judgement.** `cargo test X passes` is checkable.
   "Handles edge cases gracefully" is not, and will be reported as done regardless.
4. **The verification artifact is committed first**, by a different task, ideally by a different
   model. An agent that writes both the implementation and the test that proves it will write a
   test that passes.

---

## Model routing

Routing is by **task shape**, not by importance. The question is always: *what does being subtly
wrong here cost, and would the failure be visible?*

| Route to Fable | Route to Opus |
|---|---|
| Contract is exact and fixtures already exist | The contract itself has to be designed |
| Failure is loud (test fails, build breaks) | Failure is **silent** (wrong data, subtly weak check) |
| Mechanical breadth: one scraper per platform, one component per design spec, CRUD, glue | Security-critical: sanitizer, SSRF, path validation, auth |
| Well-trodden patterns with an obvious right answer | Concurrency, sync convergence, anything with ordering |
| Test and fixture authoring from a spec | **Adversarial verification** of anything Fable wrote |
| Docs, changelog, boilerplate | Anything touching an external system's real behaviour |

**Always Opus, regardless of apparent size:** `sanitize/`, `ssrf/`, `paths/`, `sync/`, the auth
middleware, and every verification pass. These are the places where "looks right" and "is right"
diverge, and where the plan review already found the original human-written specs were wrong.

**Never an agent alone — these need real execution against the real system:** the P0 spikes. An
agent asked "does Claude Code connect to an MCP server over a Unix socket?" will produce a
confident answer from pattern-matching. The original plan's Unix-socket MCP design is exactly what
that failure looks like. Spikes must **run** and paste real output.

---

## Orchestration patterns

Which workflow shape to use for which work. These are the shapes worth reaching for; most stages
use two or three.

**Interface-freeze → fan-out → verify.** The default. One Opus agent designs and freezes the trait
or schema; N Fable agents implement behind it in parallel; N Opus agents adversarially verify.
Used for: scrapers, UI components, CLI subcommands, API handlers.

**Adversarial verification.** For every security-critical or silent-failure module, spawn
verifiers *prompted to refute*, not to confirm — "find an input that defeats this filter" beats
"review this filter". Majority-refute kills the change. Used for: sanitizer, SSRF, auth, path
validation.

**Perspective-diverse verification.** Where a module can fail in several unrelated ways, give each
verifier a distinct lens rather than running identical reviewers: correctness / security /
performance / does-it-actually-reproduce. Redundancy catches less than diversity.

**Golden-corpus regression.** For anything whose output is judged rather than asserted —
normalization, rendering, snippet generation. Commit the corpus, diff the output, review the diff.
This is what makes normalization quality tractable at all.

**Loop-until-dry.** For unbounded discovery — bug hunts, edge-case sweeps before a release gate.
Keep spawning finders until K consecutive rounds surface nothing new, deduping against everything
already seen.

**Judge panel.** For genuinely open design questions with a wide solution space (the normalization
pipeline shape, the reader's IPC protocol): generate N independent approaches from different
angles, score them with independent judges, synthesize from the winner while grafting the best
ideas from the runners-up. Better than iterating one attempt.

---

## Stage 0 — Foundations

**Goal:** make it possible to tell whether agent output is correct.
**Gate to leave S0:** CI is green, an empty app launches, and `cargo test` runs a real test.

**Status: complete.** The workspace builds, the app bundles and launches, `paths` has 9 unit tests
plus a cross-binary integration test and 7 properties, and the gate runs fmt / clippy `-D warnings` /
tests / fuzz-target type-check / cargo-audit / cargo-deny / gitleaks / npm audit. The verification
infrastructure — fixture server, golden harness, property and fuzz scaffolding — lives in
`crates/tome-testkit` and `fuzz/`, and is a dev-dependency of the shipping crates rather than part
of them.

Nothing in S0 is a feature. All of it is leverage — everything after it moves faster and more
safely because it exists.

| # | Work | Model | Notes |
|---|------|-------|-------|
| ✅ S0-1 | Cargo workspace: `tome-core` (lib), `tome-cli` (bin), `src-tauri` (app) | Opus | **`tome-core` shared by CLI and app is the architecture** (ADR-0002) — get this boundary right on day one; it is expensive to retrofit |
| ✅ S0-2 | Svelte + Vite + TS frontend, app launches | Fable | |
| ✅ S0-3 | `paths` module + tests | **Opus** | P1-006. The first real code. A test asserts the app and CLI binaries resolve byte-identical paths |
| ✅ S0-4 | Error taxonomy (`Error`) | Opus | Frozen early; every later ticket returns into it |
| ✅ S0-5 | CI: fmt, clippy `-D warnings`, test, audit, deny, gitleaks | Fable | The audit job the old plan claimed to have |
| ✅ S0-6 | Fixture HTTP server (serves committed doc-site fixtures offline) | Fable | **Prerequisite for S1.** Every scraper test needs it. Scripted 4xx/5xx, redirects, delays, truncated bodies, conditional GET, a request log, and a shutdown that makes the port *refuse* connections |
| ✅ S0-7 | Golden-corpus harness (snapshot + diff normalized output) | Opus | Makes normalization quality reviewable. An empty suite and an orphan golden both fail; update mode fails the run it rewrites |
| ✅ S0-8 | Property-test + fuzz scaffolding (`proptest`, `cargo-fuzz`) | Fable | 7 properties over `paths` now; the interesting targets land with their modules (`fuzz/README.md` names them) |
| ✅ S0-9 | `LICENSE-MIT` + `LICENSE-APACHE`, bundle id threaded through one constant | Fable | DEC-001, DEC-002 — now resolved |

**S0-3 deserves its own note.** Every path in the codebase comes from this module; nothing else
constructs one. The plan review found four different data locations across documents and several
samples passing a literal `~` to APIs that do not expand it. A single module with a test that both
binaries agree makes that class of bug impossible rather than merely discouraged.

**One thing S0 did *not* settle, deliberately — and S1 then did.** The S0 scaffold's
`Paths::pages_dir("../../etc")` escaped the cache directory lexically, and the property tests said
so in a comment rather than asserting a containment invariant that did not hold. After S1-1 froze
`SourceId`, the accessors moved from `&str` to `&SourceId` (2026-07-28): hostile ids now fail
construction rather than being caught at call sites, and the containment assertion lives in the
property tests and the `paths` fuzz target, as promised.

---

## Stage 1 — Vertical slice

**Goal:** one real documentation site, fetched, normalized, sanitized, asset-localized, indexed for
nothing yet, and **rendered offline in the reader**.
**Entry gate:** S0 exit + **SPIKE-002** (WebView bridge) has run for real. ✅ **Both are met** —
SPIKE-002 ran 2026-07-28 ([write-up](../spikes/002-reader-iframe-bridge.md)); S1-13 inherits its
protocol and frame posture, plus an interactive frame-pacing check in its acceptance criteria.
**Exit gate:** the app renders `docs.python.org` with the network off, images included, anchors
working, and the golden corpus is committed.

This is the stage that answers whether the product is possible.

| # | Work | Spec | Model | Parallel |
|---|------|------|-------|----------|
| S1-1 ✅ | Freeze core types: `Source`, `Page`, `Node` (AST), `DocSet` | P1-004/012 | **Opus** | done 2026-07-28 — `tome-core/src/model/`; serde shape pinned by test |
| S1-2 ✅ | SQLite schema + migrations + repos | P1-004/021 | Fable | done 2026-07-28 — `tome-core/src/db.rs`; SyncState deferred with sync (ADR-0005) |
| S1-3 ✅ | Source config YAML parser + validation | P1-005 | Fable | done 2026-07-28 — `tome-core/src/config.rs`; `source_config` fuzz target live |
| S1-4 ✅ | HTTP client: rate limit, robots.txt, retry, conditional GET | P1-008 | Fable | done 2026-07-28 — `tome-core/src/fetch/`; sync client, manual redirects (per-hop robots + the S1-5 seam); `robots` fuzz target live |
| S1-5 ✅ | **SSRF filter** | P1-008 / security | **Opus + adversarial verify** | done 2026-07-28 — `tome-core/src/fetch/ssrf.rs` + `resolver.rs`; refute-panel workflow ran; `ssrf` fuzz target live |
| S1-6 ✅ | BFS crawl + URL filter | P1-010/011 | Fable | done 2026-07-28 — `tome-core/src/crawl/`; links discovered from whole doc (nav is discovery), politeness+SSRF inherited from Fetcher |
| S1-7 ✅ | HTML → AST parser (html5ever) | P1-012 | Opus | done 2026-07-28 — `tome-core/src/parse.rs`; `html_parser` fuzz target live from day one |
| S1-8 ✅ | Normalization pipeline | P1-013 | Opus + golden corpus | done 2026-07-28 — `tome-core/src/normalize.rs` + `corpus/normalization`. **Acceptance complete 2026-07-29:** 26 real pages across six platforms (Sphinx, mdBook, rustdoc, Node, Hugo, go.dev), licences verified from each source and recorded in `input/SOURCES.md`. The corpus found a defect on its first run (Node permalinks are `#`, so every Node page was titled `OS#`) |
| S1-9 ✅ | **Sanitizer** | security | **Opus + adversarial verify** | done 2026-07-28 — `tome-core/src/sanitize.rs`; two corpora (XSS + anchors); refute-panel ran; `sanitizer` fuzz target live |
| S1-10 ✅ | Asset localization | P1-023 | Fable | done 2026-07-28 — `tome-core/src/assets.rs`; AST `Image.src` localized, content-addressed, offline-gate test passes; srcset/video/SVG-sanitize/GC flagged (not in typed AST) |
| S1-11 ✅ | Syntax highlighting | P1-014 | Fable | done 2026-07-28 — `tome-core/src/highlight.rs` + `html.rs` (the one escaping helper S1-9's contract delegates to); syntect without themes or oniguruma, CSS classes not inline styles; `highlight` fuzz target + proptest properties live. **TypeScript aliases to JavaScript and TOML/Dockerfile/Kotlin/Swift render plain** — syntect's bundled set stops at Sublime's shipped packages; `two-face` is the upgrade path and is a licence decision |
| S1-12 ✅ | Typography + design tokens | P1-015 / design system | Fable | done 2026-07-28 — `public/tokens.css` (linked by BOTH the app and the reader frame; the frame's opaque origin cannot share the app cascade), `public/reader.css`, `src/app.css`. `scripts/check-contrast.mjs` is now a gate step: it found 3 real palette defects (dark accent as link text 3.36:1, as focus ring 2.70:1, status colours as text 1.96:1) → new `--color-link` / `--color-focus` / `--color-*-text` tokens |
| S1-13 ✅ | Reader iframe + IPC bridge | P1-016 | **Opus** | done 2026-07-28 — `tome-core/src/render.rs` (AST→HTML, honours S1-9's escaping contract), `store.rs` (AST on disk, hashed filenames — macOS case-insensitivity would merge two pages), `pipeline.rs` (**the missing write side**: nothing persisted a crawl before this, so the exit gate was unreachable), `src-tauri/src/reader.rs` (`tome://` asset protocol, four-way path validation), `public/reader-frame.js` + `src/lib/reader/bridge.ts`. `tome pull` / `tome list` implemented so a real source can reach the reader. spike002 harness deleted. `reader_offline.rs` is the exit gate at the HTML layer |
| S1-14 ✅ | Three-panel layout, library sidebar, TOC | P1-017/018/019 | Fable | done 2026-07-28 — `Layout.svelte` (collapsible, drag- and keyboard-resizable, widths persisted; sidebars are zero-width rather than unmounted, and `visibility:hidden` so a collapsed panel leaves the tab order), `Library.svelte` (categories → sources → pages, filtered, arrow-key navigable), `Outline.svelte` + `OutlineList.svelte` (recursive, scroll-spy fed by the frame — the app cannot read that document). `$lib/keys.ts` carries the PRD rule that single-letter reading keys must not fire while a text field has focus, which the new filter box makes load-bearing |
| S1-15 ✅ | Navigation + history | P1-020 | Fable | done 2026-07-28 — `$lib/navigation.ts` (a plain, exhaustively-tested history stack: truncates the forward branch, replaces rather than duplicating a repeat visit, remembers scroll per entry, trims the OLDEST past its cap; `classifyLink` routes on the shape the renderer already decided). Toolbar buttons + `Cmd+[` / `Cmd+]`. External links go to `open_external` in Rust, which parses with `url::Url` and allowlists http/https/mailto — validation split from the spawn so the tests never launch a browser |

**Verification that matters here:**

- **S1-9 sanitizer** runs against two corpora, and must pass both: an XSS payload set (nothing
  survives) *and* an anchor set (nothing breaks). The original sanitizer stripped `id` and would
  have silently disabled the TOC — a security control breaking a headline feature. One corpus
  alone cannot catch that.
- **S1-8 normalization** is judged by golden-corpus diff across ≥ 20 real sites spanning all target
  platforms. This is where "does the product work?" is actually answered.
- **S1-7 parser** gets a fuzz target from day one. "Zero panics on any input" is in the spec and is
  otherwise unverified.
- **Offline is an assertion, not a vibe:** a test shuts the fixture server down and asserts the
  rendered HTML contains no `http` references.

---

## Stage 2 — Search

**Goal:** fast, relevant search, with a number attached to "relevant".
**Entry gate:** S1 exit + **SPIKE-003** (Tantivy at 100k pages) has run — ✅ [ran 2026-07-29](../spikes/003-tantivy-scale.md). All four criteria pass with margin (439 MB peak indexing, 18.7 ms worst p95, 224 MB index, 3 MB idle). Three findings change how S2 is built: the writer budget is a **speed** knob not a memory one (use 512 MB); segment count is what degrades search, so set an explicit merge policy; and indexing is 3 orders of magnitude cheaper than crawling, so **S2-3's justification is avoiding re-crawls, not avoiding re-indexing**.
**Exit gate:** relevance eval ≥ 0.90 recall@3 **over a corpus of at least 150 documents**, P95 <
100 ms on the benchmark corpus. **Both met, 2026-07-29** — recall@3 **0.9082** over 339 documents,
P95 **158 µs**. The relevance half is met by one query of 207 and should be read as *met*, not as
comfortable; the latency half has three orders of magnitude of headroom.

> The document floor was added 2026-07-29, after S2-1 measured that the metric is not
> discriminating below it. On the original 26-document corpus, removing an entire indexed field
> from the query moved MRR by 0.0036 — so `≥ 0.90 recall@3` was satisfiable by a corpus too small
> to have a wrong answer worth beating. A threshold that a weaker system also passes is not a gate.

| # | Work | Spec | Model |
|---|------|------|-------|
| S2-1 ✅ | **Relevance eval set + harness** | P2-019 | Opus — done 2026-07-29 — `corpus/relevance/` (207 labelled queries, 339 documents, 6 sources) + `tests/relevance.rs`. Found a real defect on its first run: `()` and `[]` are query-parser syntax, so twelve symbol queries returned nothing. Symbol recall@1 0.7465 → **0.9474** after the fix. **Also measured its own weakness** — see below |
| S2-2 ✅ | Tantivy integration + schema | P2-001/002 | Opus — done 2026-07-29 — `tome-core/src/search/`: `schema.rs` (P2-002's seven fields), `tokenizer.rs` (camelCase/snake_case aware, emits the identifier *and* its parts), `extract.rs` (AST → fields), `mod.rs` (`SearchEngine` + `IndexSession`). SPIKE-003's harness removed as planned; its write-up stays |
| S2-3 ✅ | Incremental indexing | P2-003 | Fable — done 2026-07-29 — `pipeline::index_source`, wired into `pull`. Change detection reads **the index**, not the database, so a cleared cache repopulates instead of reporting "all indexed" forever. One commit per sync, because a commit creates a segment and segment count is what degrades search (SPIKE-003 finding 4). A corrupt index is discarded and rebuilt — it is derived and lives in the cache. `tome search` implemented alongside (P4-005, brought forward) so the result is checkable by hand |
| S2-4 ✅ | Ranking + boosts | P2-006 | Opus — done 2026-07-29 — `search/ranking.rs`. Coordinate descent over field boosts, a document-length penalty, and a query-time stopword policy, scored by S2-1 and constrained never to rank `symbol` queries below the untuned ranker. MRR 0.7489 → **0.8245**, recall@1 0.6377 → **0.7585**, `natural` MRR 0.2419 → **0.4596**; 48 queries better, 10 worse. **recall@3 reached 0.8744 against the 0.90 gate and tuning cannot close the rest** — see below |
| S2-5 ✅ | Fuzzy matching | P2-009 | Opus — done 2026-07-29 — `search/fuzzy.rs`. **Not** `FuzzyTermQuery`: it produces a `ConstScorer`, so every fuzzy hit would score identically and BM25 would be discarded for that term. Corrects the *query* against the term dictionary instead, which keeps scoring intact and makes "did you mean?" a by-product. `misspelling` recall@3 0.4167 → **0.7500**; nothing outside that category moved. Overhead ~2 µs when nothing is misspelled |
| S2-6 ✅ | Symbol-aware search | P2-015 | Opus — done 2026-07-29 — `search/symbols.rs`. P2-015's technical note extracts declarations from **code blocks**; measured over the corpus that finds `main`, `buf`, `foo`, `options` — the examples' scaffolding, while `with_capacity` is declared *never*. Signatures live in **headings**, so extraction reads path, title and headings instead. Two fields: `symbol` (the page's primary symbol, blended) and `declarations` (everything, reachable only via `@symbol`) — blending all declarations was measured at −0.08 MRR. `symbol` MRR 0.8817 → **0.8919** |
| S2-7 ✅ | Search UI, results, scoping, history, keyboard | P2-004/005/008/016/017 | Opus — done 2026-07-29 — `SearchModal.svelte` + `src-tauri/src/search.rs` + `search/snippet.rs`. Cmd+K, 150 ms debounce with stale-response discarding, scoping remembered and revalidated, 50-entry history, full keyboard control. **Snippets cross IPC as spans, never HTML** — they are crawled content drawn in the *app's* DOM, where the IPC bridge is reachable. Four listed criteria deliberately not built (score display, grouping, category scope, scope shortcut); each is marked `[~]` in P2-005/008/017 with the reason |
| S2-8 ✅ | In-page search | P2-007 | Opus — done 2026-07-29 — `FindBar.svelte` + find in `public/reader-frame.js`. `window.find()` is unusable: the frame has no `allow-same-origin`, so the app cannot read its document. Matches are painted with the **CSS Custom Highlight API** rather than wrapped in `<mark>` — `surroundContents` throws whenever a range partially covers a node, which is the normal case for a match crossing the syntax highlighter's `<span>`s. Fixed a latent bug in `$lib/keys.ts`: `event.key` is `'G'` when Shift is held, so **every shifted shortcut in Appendix C silently did nothing** |
| S2-9 ✅ | Detection corpus + harness | P2-020 | Opus — done 2026-07-29 — `corpus/detection/` (129 real homepages, 6 generators) + `tests/detection.rs` (accuracy, confusion matrix, per-fixture deltas). Built **before** the detector, as S2-1 was before S2-4; the baseline detector scores **0.2016** and the matrix says why. The fetch script's `<meta name="generator">` cross-check caught 8 mislabels on its first run — 2 sites had migrated generator, 6 were Zensical emitting Material markup. **GitBook has no fixtures**: it is a hosted product with no redistributable public instances, so the matrix reports an empty row rather than a score |
| S2-10 ✅ | Platform detection | P2-014 | Opus — done 2026-07-29 — `detect.rs`, scored by S2-9: **0.2016 → 0.9922** accuracy with **zero confidently-wrong** classifications, against a ≥ 0.95 target. Every marker chosen by counting across the corpus, which contains a counter-example to three of P2-014's own sketched rules — `readthedocs.io` hosting MkDocs, the mdBook-built *rustdoc book*, and `(Generic, 1.0)`. Building the corpus first also caught **three mislabels of mine**: two sites I had labelled by domain rather than markup, and one that had migrated |
| S2-11 ✅ | Scrapers: ReadTheDocs, rustdoc, mdBook, man | P2-010..013 | Opus — done 2026-07-29. **Three of the four turned out to be largely already built**: S1-8's furniture rules were developed against a corpus spanning six platforms, so adding profiles for Sphinx, rustdoc and mdBook moved *four* golden files, all rustdoc, removing exactly the `1.0.0 · Source` sub-heading. `scrape.rs` holds the platform profiles (exact class tokens, which the generic substring list cannot safely use); the Sphinx and mdBook rules are **unmeasured** and say so. Man pages (P2-013) were genuinely absent and are built: `man.rs` discovers, renders through `mandoc -T html`, links cross-references only to installed pages, and extracts the NAME one-liner |
| S2-12 ✅ | Benchmarks + regression alerts | P2-018 | Opus — done 2026-07-29 — `tests/search_bench.rs`. The exit gate's latency half, measured over the real corpus with the real 207 queries: **P95 158 µs in release, 1.47 ms in debug, against a 100 ms budget**. Latency grows sub-linearly across a 16× corpus. **No committed timing baseline, deliberately**: a cross-machine wall-clock baseline fails on a slower laptop while hiding real regressions, so the gate is an absolute threshold with ~600× headroom that fires on a lost index rather than a busy machine |

**S2-1 before S2-4 is not negotiable.** Tuning ranking without an eval set is guesswork, and with
agents it is *fast* guesswork — you will get twenty confident boost-factor changes and no way to
tell which helped. The eval set is what converts search from an opinion into a gradient.

**The eval set measured its own corpus into existence.** At its first size — 26 documents — three
perturbations (title boost cut 60×, code boost cut 1500×, `code` field removed from the query
entirely) each moved MRR by ≤ 0.0036 and tripped nothing. The metrics compressed near the top
because with 26 documents there is rarely a strong wrong answer for a right one to beat.

It was therefore expanded to **339 real pages across six sources** (2026-07-29, owner-approved),
crawled with the real `tome pull` from the hosts already licence-verified for the normalization
corpus. At that size it discriminates:

| Perturbation | MRR | Movement | Gate |
|---|---|---|---|
| Title boost 3.0 → 0.05 | 0.7489 → 0.7625 | 17 worse, 28 better | silent — a net **improvement** |
| Remove `code` from the query | 0.7489 → 0.7536 | 13 worse, 15 better | silent — a genuine wash |
| Search `body` only | 0.7489 → **0.4293** | 118 worse, 27 better | **fires**, both thresholds |

Three findings fell out, and **S2-4 acted on all three**:

- **A title boost of 3.0 was too high** — cutting it *improved* MRR. Now 0.75. A title is a handful
  of tokens, so BM25's own length normalisation already multiplies it heavily and an explicit boost
  on top was double-counting.
- **The `code` field contributes almost nothing.** Removing it is a wash, because on these
  platforms method names are also headings, so `headers` already carries them. Its boost is now
  1.0, below `headers`. Worth knowing before S2-6 builds symbol-aware search assuming that field is
  load-bearing.
- **Long single-page documents dominated natural-language queries.** `natural` sat at 0.0625
  recall@1 — the worst category by far — because one enormous FAQ page ranked first for nearly every
  "how do I …" query, and the Cargo book's single-page `print.html` did the same to Cargo queries.
  Fixed by a document-length penalty plus query-time stopword removal; `natural` MRR 0.2419 →
  0.4596.

**BM25's own `b` was the textbook lever and is not reachable.** In tantivy 0.26 `k1` and `b` are
private constants in `src/query/bm25.rs`, not weight parameters, so `Ranking::length_penalty`
applies a post-hoc divisor from the collector instead. Anyone re-reading this plan looking for
`set_bm25_params` will not find it.

**The relevance gate is met after S2-6.** recall@3 is **0.9082 against 0.90** — 188 queries of 207
— and it took all three ranking tickets to get there: S2-4 reached 0.8744, S2-5 0.8986, S2-6
0.9082. The route matters as much as the number. During S2-5 a neighbouring configuration reached
0.9034 and was deliberately **rejected**, because it was chosen for crossing the threshold and cost
`symbol` MRR (0.8817 → 0.8606) to do it; fitting a gate is not passing it. The configuration that
did clear it was the optimum on a different objective and dominates its predecessor on every
column. Do not close relevance gaps by re-labelling queries — that destroys the instrument.

Differences of one or two queries between neighbouring configurations are inside the noise of a
207-query corpus. The gate is met, not comfortably met.

**S2-2 nonetheless landed first, and the table's order is the misleading part.** P2-019 lists
P2-001 as a dependency for the obvious reason: a relevance harness needs an index to score. The
constraint the plan is actually asserting is S2-1 before *ranking* — S2-4, S2-5, S2-15 — not S2-1
before integration. S2-2 therefore shipped with the field boosts as **unmeasured placeholders** and
one test pinning only their *direction*; S2-4 replaced them with `Ranking::TUNED`.

**S2-11 is the canonical fan-out**: four scrapers, one interface, four parallel Fable agents, four
Opus verifiers, all scored against the same detection corpus. This is the shape agent workflows
are best at, and it is why breadth of platform support gets cheaper under this build model — one of
the few places where "agents help" is straightforwardly true.

---

## Stage 3 — Agent access

**Goal:** `tome` on the command line, and Claude Code reading your docs.
**Entry gate:** S2 exit + **SPIKE-008** run against a real MCP client.
**Exit gate:** Claude Code connects over stdio and answers a question from a locally indexed page.

| # | Work | Spec | Model |
|---|------|------|-------|
| S3-1 | CLI scaffolding + all subcommands | P4-001..007 | Fable |
| S3-2 | **MCP stdio server** | P4-013/014 | **Opus** |
| S3-3 | MCP tools | P4-015/016 | Fable |
| S3-4 | Result truncation + `section` selection | new | Fable |
| S3-5 | HTTP API + **auth middleware** | P4-009..012 | **Opus for auth**, Fable for handlers |
| S3-6 | Claude Code plugin | P4-017 | Fable |
| S3-7 | Sync strategies (fetch scheduling) | P4-018 | Fable |
| S3-8 | Source registry + CI verification job | PRD | Fable |

**Verification specific to this stage:**

- **A test asserts `tome mcp` writes nothing but JSON-RPC to stdout.** One stray `println!` breaks
  every client with an opaque parse error, and it is exactly the kind of thing an agent adds while
  debugging.
- **A real browser test** confirms a cross-origin `fetch()` cannot read an API response. Reading the
  CORS config and concluding it is fine is how the original plan got this wrong.
- **The MCP handshake is tested against an actual client**, not a mock. The original design failed
  precisely because nobody tried it.

---

## Stage 4 — Hardening and release

**Entry gate:** S3 exit.
**Exit gate:** `brew install --cask alexnodeland/tap/tome` works on a machine that has never built
Tome, the app launches after the documented quarantine step, `tome status` reports the same paths
the app shows, and `--zap` leaves nothing behind.

> **No notarization.** [ADR-0006](../decisions/0006-unsigned-distribution.md) defers the Apple
> Developer Program, so Tome ships unsigned through `alexnodeland/homebrew-tap` — the same channel
> and cask conventions as `curio` and `statusbar`. Gatekeeper will block first launch; the cask's
> `caveats` carry the fix. Revisit at v1.0, when the friction starts costing real adoption.

| # | Work | Spec | Model |
|---|------|------|-------|
| S4-1 | Loop-until-dry bug hunt across the codebase | — | ✅ 2026-07-30 |
| S4-2 | Performance profiling + lazy loading | P5-001/002/003 | Opus |
| S4-3 | Error taxonomy audit + recovery | P5-004/005 | Fable |
| S4-4 | Onboarding (registry-first) | P5-006 | Fable |
| S4-5 | Preferences UI | P5-007 | Fable |
| S4-6 | Menu bar + global shortcut *(conditional on SPIKE-001)* | P5-008/009 | Fable |
| S4-7 | Accessibility pass: contrast CI, keyboard, VoiceOver | design system | Opus |
| S4-8 | Unsigned DMG + release workflow + tap mirror | P5-011/012 | Fable |
| S4-9 | **Ship `tome` inside the app bundle** at `Contents/MacOS/tome` | new | Opus |
| S4-10 | User docs + landing page | P5-013/014 | Fable |

**S4-8 is much simpler than the original plan assumed**, because there are no signing identities,
no Apple credentials, and no notarization step — the single most sensitive secret the release
pipeline would have held is simply absent. The workflow builds, packages, publishes, and mirrors
the cask.

**S4-9 is the one that matters and is easy to get wrong.** The cask symlinks
`Tome.app/Contents/MacOS/tome` onto `PATH`, so `brew install --cask tome` must deliver *both* the
app and the CLI **from the same build**. That is what makes them resolve the same library — the
invariant in [ADR-0002](../decisions/0002-no-app-sandbox.md), delivered through one install. A
release that ships the app without the CLI will look fine and silently break every integration.

**Verify on a clean machine regardless.** No notarization to get wrong, but a DMG missing the CLI,
a `zap` list that leaves data behind, or an app that will not launch all pass every automated check
and fail for every user.

---

## Stage 5 — Sync (deferred)

**Not scheduled.** Per DEC-004 and the plan review, sync is the largest single chunk (68
person-equivalent days), carries the highest-scoring technical risk (RISK-002), and is the least
painful thing to lose — bookmarks work fine on one machine.

**Revisit when:** v1.0 has shipped, and more than a handful of users have actually asked. Design is
already specified (ADR-0001, P3-010..015) and does not decay while waiting.

Bookmarks and annotations are still built in S1–S4 as **local** features — the personal layer is
part of the product, only its propagation between machines is deferred.

---

## Definition of done, revised for agent work

Replaces the global DoD in `00-project-overview.md` for tickets executed this way.

1. All contract signatures match the frozen interface exactly
2. `cargo test --workspace` and `npm run test` pass
3. `cargo clippy --all-targets -- -D warnings` and `npm run lint` clean
4. **The verification artifact existed before the implementation** and was authored by a different
   task
5. **For security-critical modules:** an adversarial verification pass, prompted to refute, did not
   find a defeating input
6. No files touched outside the ticket's declared scope
7. No new dependency without an explicit line in the ticket authorizing it
8. Specification updated in the same change if an external surface changed
9. `git diff --stat` is within an order of magnitude of the ticket's expected size — **a 40-line
   ticket that produced 900 lines did something else**, and that is worth looking at before merging

Rule 9 is cheap and catches a surprising amount. Scope drift is the most common failure mode of
otherwise-correct agent output.

---

## Risks specific to this build model

These are additional to `11-risk-register.md`, which covers the product's risks. These are the
risks of *building it this way*.

| Risk | Why it bites | Mitigation |
|---|---|---|
| **Plausible-but-wrong code merged** | Agent output is confident, well-formatted, and idiomatic whether or not it is correct — the strongest signals humans use to judge code are exactly the ones agents produce regardless | Verification artifacts first; adversarial passes on silent-failure modules; rule 9 |
| **Verification theatre** | Tests written by the same task as the implementation will pass. They always pass. | Different task, ideally different model, ideally authored first |
| **Interface drift** | Parallel agents each invent a slightly different signature; integration becomes a rewrite | Freeze interfaces in a serial Opus step; paste exact signatures into tickets |
| **Silent scope creep** | An agent "helpfully" refactors three adjacent modules | Declared file scope; rule 9 |
| **Confident answers about external systems** | The original Unix-socket MCP design is what this failure looks like in the wild | Spikes must execute and paste real output; never accept a recalled API shape |
| **Reviewer fatigue** | The human stops reading carefully somewhere around the fifth large diff of the day, and that is when something lands | Keep tickets small; automate the checkable; batch review by module, not by chronology |
| **Dependency sprawl** | Every agent adds a crate to solve its local problem | Explicit authorization per ticket; `cargo deny` in CI |

**The honest summary of this build model:** it makes breadth cheap and depth no cheaper. Four
scrapers in parallel is a genuine win. The sanitizer, the SSRF filter, and sync convergence are
exactly as hard as they were, and the main thing agents change about them is how quickly you can
generate something that *looks* finished.

---

## Immediate next actions

1. ✅ **DEC-001** — dual MIT OR Apache-2.0
2. ✅ **DEC-002** — `com.alexnodeland.tome`
3. ✅ **DEC-004** — solo + agent workflows; scope cut to S0–S4, sync deferred
4. ✅ **S0 scaffold** — workspace, frontend, paths module, CI, test infrastructure (S0-6/7/8)
5. ✅ **SPIKE-002** — ran 2026-07-28; [write-up](../spikes/002-reader-iframe-bridge.md). Single
   postMessage carries a full page; CSP inheritance into srcdoc holds; the sandbox seals the IPC
   layer; frame pacing moved to S1-13's interactive acceptance
6. ✅ **SPIKE-010** — ran 2026-07-28; [write-up](../spikes/010-legal-posture.md). No source
   forbids the planned behaviour; the corpus gained a licence gate; takedown policy drafted;
   no shipped opt-out list
7. ✅ **DEC-003** — Apple Developer Program **deferred**; unsigned distribution via
   `alexnodeland/homebrew-tap` ([ADR-0006](../decisions/0006-unsigned-distribution.md))
8. **Keep `scripts/check.sh` and `ci.yml` in lockstep** — the local script is the gate until the
   repository goes public, and drift between them is the failure mode that arrangement invites
