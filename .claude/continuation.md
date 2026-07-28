# Continuation note — Tome

**Written:** 2026-07-28. Updated the same day, after Stage 0 closed and PR #4 merged.
**Delete this file when Stage 1 lands.** It is in-flight task state, not documentation.

---

## Where things stand, in one paragraph

Stage 0 is complete (scaffold + fixture server + golden harness + fuzz). Both Stage 1 gate spikes
ran for real (SPIKE-002 reader bridge, SPIKE-010 legal posture). **The entire Stage 1 ingestion
backend is now built and merged: S1-1 through S1-10.** `tome-core` runs the whole pipeline —
config → fetch (robots/rate-limit/SSRF) → crawl → parse → normalize → sanitize → asset-localize —
all tested against the fixture server, and the **offline guarantee (Stage 1's exit gate) is proven**
by the asset-localization test that shuts the server down and asserts no `http` reference survives.
What's left of Stage 1 is the **reader half (S1-11..S1-15)**: syntax highlighting, typography, the
reader iframe (S1-13, the SPIKE-002 bridge — Opus), layout, navigation. The app still launches to
the scaffold UI; the reader is not wired yet.

```
main   ← everything below is merged; branch from here (newest first)
  #27  S1-10 asset localization + offline gate; hash module
  #26  S1-9  sanitizer (refute-panel: 5 defects fixed, held R2)
  #25  S1-8  normalization + seeded golden corpus
  #24  S1-6  BFS crawl + URL filter
  #23  S1-5  SSRF filter (refute-panel: 4 defects fixed, held R2)
  #22  S1-7  HTML→AST parser
  #21  S1-4  HTTP client (robots, rate-limit, retry, conditional GET)
  #20  S1-2  SQLite schema + repos
  #19  S1-3  source config parser
  #18       Paths → &SourceId (closed the S0 containment gap)
  #16  S1-1  core types frozen (serde shape pinned)
  #17  SPIKE-010 legal posture   ·   #14 SPIKE-002 reader bridge
  (Stage 0: fixture server, golden harness, fuzz, scaffold, plan audit)
```

**Pipeline order** (each stage a pure function over the previous, all in `tome-core`):
`SourceConfig::parse` → `Crawler::crawl` (→ `DocSet` of parsed AST) → per page:
`normalize::normalize` → `sanitize::sanitize` → `assets::localize_assets`. Persistence (`db.rs`)
and the reader consume the result. The AST (`model::Node`) is frozen; its serde shape is a storage
contract (`tests/model_serde_shape.rs`).

**Merge trap, learned on #15:** merging a stacked PR with `--delete-branch` does not retarget the
child PR — GitHub **closes** it unrecoverably. For a stack, merge the parent *keeping* its branch,
`gh pr edit <child> --base main`, then merge the child. (All S1 tickets were merged un-stacked
straight to main, which sidesteps this.)

**~141 Rust tests (tome-core) + 3 Vitest + 7 fuzz targets; the full gate is green including the app
build.** Every fuzz target asserts an invariant, not just no-panic.
`crates/tome-testkit` holds the fixture server and golden harness (**dev-dependency only** — if it
ever appears under `[dependencies]`, that is a bug); `fuzz/` is a separate workspace, type-checked
by `check.sh` on stable, run with `cargo +nightly fuzz`.

**PR #2 is still open and stale** — the abandoned `claude/project-scaffolding-agent-Z1Hxy`
scaffolding branch, superseded by everything above. It was left alone rather than closed
unilaterally; ask before closing it.

## First thing to run

```bash
cd /Users/alexandernodeland/anodeland/projects/code/apps/tome
git checkout main && git pull   # Stage 0 is all on main now
./scripts/check.sh --fast       # ~60s. Should print "All checks passed."
```

**`scripts/check.sh` is the gate — CI cannot run.** The repo is private until release *and* GitHub
Actions is blocked at the account level ("recent account payments have failed"). Every workflow run
fails in 2s without executing a step. The script runs exactly what `.github/workflows/ci.yml` runs.
**If you change one, change the other in the same commit** — drift here is the failure mode this
arrangement invites.

## Read these, in order

1. `docs/plans/18-implementation-plan.md` — the execution plan. Stages, gates, model routing.
2. `docs/reviews/2026-07-28-plan-review.md` — what was wrong with the original plan and why.
3. `CLAUDE.md` — the settled facts that are easy to regress.
4. `docs/decisions/` — six ADRs. **Do not relitigate these.**
5. `crates/tome-testkit/src/{server,golden}.rs` module docs — what the S1 tests are built on, and
   which simplifications were made deliberately.

---

## Settled — do not reopen without a reason

| | Decision | Why it matters |
|---|---|---|
| ADR-0001 | Sync = iCloud Drive container with per-device op logs, **not CloudKit** | CloudKit is Swift-only; the core is Rust and the CLI runs out-of-process |
| ADR-0002 | **No App Sandbox.** Developer ID posture, hardened runtime | A sandboxed app and a Homebrew CLI would resolve *different* data dirs |
| ADR-0003 | Dual MIT OR Apache-2.0 | |
| ADR-0004 | Bundle id `com.alexnodeland.tome` | Threads through Keychain, iCloud, prefs, zap |
| ADR-0005 | Solo + Fable/Opus agent workflows; **sync deferred**; scope cut to S0–S4 | Changes the constraint from typing speed to verification bandwidth |
| ADR-0006 | **Unsigned**, via `alexnodeland/homebrew-tap`. Apple Developer Program deferred | Revisit at v1.0 |

Also settled, from the plan review — regressing any of these undoes real work:

- Tauri **is** the shell. No Swift shell. Reader = sandboxed `<iframe>`, not a second WKWebView.
- MCP is **stdio**. There is no `mcp.sock`.
- Local API: bearer token on **every** request including loopback; no CORS by default.
- Annotations anchor by quote+prefix/suffix, **never** bare character offsets.
- `robots.txt` obeyed by default, non-overridable for registry configs.
- **No telemetry, ever** — so no metric expressed as a percentage of users.
- `tome pull` fetches docs. There is **no `tome sync`**.
- Data lives in `~/Library/Application Support/Tome` + `~/Library/Caches/Tome`. Never `~/.tome`.

---

## What to do next

### Done: S0-6, S0-7, S0-8 — Stage 0 is closed

| | Work | What landed |
|---|---|---|
| **S0-6** | Fixture HTTP server | `crates/tome-testkit/src/server.rs`. Hand-rolled over `std::net` — a fixture server has to *misbehave* on purpose (scripted 429s, redirect chains, truncated bodies), which a correct HTTP stack resists. Shutdown makes the port refuse connections, which is what the offline assertion rests on. 16 tests, driven by a deliberately dumb raw-socket client. |
| **S0-7** | Golden-corpus harness | `crates/tome-testkit/src/golden.rs`. Empty suite fails, orphan golden fails, and `TOME_UPDATE_GOLDEN=1` **fails the run it rewrites** — the passing run is the one after `git diff`. Corpus conventions in `crates/tome-core/corpus/README.md`; S1-8 fills it. |
| **S0-8** | Property + fuzz scaffolding | 7 `proptest` properties over `paths`, one live fuzz target, planned targets tabled in `fuzz/README.md`. |

**Deliberately not asserted:** that a hostile source id stays inside the cache directory. It does
not today (`pages_dir("../../etc")` escapes lexically) — validation is S1 work next to the page
loader. The property test says so in a comment rather than asserting something that would have to
be weakened to pass. When S1 lands path validation, that assertion belongs in
`crates/tome-core/tests/paths_properties.rs` and in the `paths` fuzz target.

### Done: SPIKE-002 — the Stage 1 entry gate is met

Ran 2026-07-28, three times, against the built debug app. **Read
`docs/spikes/002-reader-iframe-bridge.md` before touching S1-13** — raw output, eight findings,
and the protocol S1-13 inherits. The short version: one postMessage carries a full 500 KB page in
~0.1 ms (no streaming protocol, ever); the srcdoc frame inherits the app CSP and a meta CSP
stacks on top (violations log twice, once per policy); the bootstrap must be an external script
and `'self'` is meaningless in the opaque origin; `__TAURI_INTERNALS__` is unreachable from the
frame; `getSelection()` works for quote+prefix/suffix anchoring. Two traps found: **Tauri's event
API is deny-by-default** — no `src-tauri/capabilities/` file means `listen()` rejects, silently
if unawaited (`capabilities/default.json` now grants `core:event:default` and nothing else); and
**occluded windows suspend rAF and clamp timers to ~230 ms**, so frame pacing went unmeasured
headlessly and is an S1-13 interactive acceptance item instead. The spike harness
(`src-tauri/src/spike002.rs`, `src/spike/spike002.ts`, `public/spike002-frame.js`, the
`spike002_mode` hook in `src/main.ts`) comes out when S1-13 lands.

### Done: SPIKE-010 — the corpus is unblocked

Ran 2026-07-28. `docs/spikes/010-legal-posture.md` has the evidence (real robots.txt and ToS
text, fetched and quoted). The operative outcomes: **the corpus licence gate** is now in
`crates/tome-core/corpus/README.md` (only alteration-and-redistribution licences may be
committed; `SOURCES.md` per suite); the README gained the plain-language posture section;
RISK-011 dropped to probability 2 / score 8 with the per-host opt-out list mitigation removed
(nothing to put on it); S1-4's rate-limit spec gets an external bound from Read the Docs'
published guidelines (< 4 req/s, self-identifying UA — stay well under). One nuance worth
remembering at S1-6: **nodejs.org disallows `/docs/` but allows `/api/`** — crawl the current
API docs, never the versioned tree.

### Done: S1-1 — the core types are frozen

`crates/tome-core/src/model/` (2026-07-28). What later stages need to know:

- **The serde shape is a contract.** `tests/model_serde_shape.rs` pins exact JSON; failing it
  means stored data and IPC break — migration territory, never an accident. Additive changes
  (new variant, new field with a default) pass it untouched.
- **`SourceId` is a validated slug, not a Uuid; `Page` has no surrogate id** (identity =
  `(source, path)`). Both are PRD corrections with the reasoning in `model/mod.rs` — the short
  version is ADR-0001: bookmark sync needs the same source on two devices to be the same
  identity. The PRD data-model sketches carry corrective comments.
- **`SourceId`/`PagePath` validation is the containment story.** No separators, no dot
  segments, rejected not normalized; the `model_ids` fuzz target asserts it. **Follow-up that
  belongs to S1-2/S1-4:** migrate `Paths` accessors from `&str` to `&SourceId`, then add the
  containment assertion to `paths_properties.rs` and the `paths` fuzz target (the deliberate
  S0 gap closes then).
- **The `Node` AST has no raw-HTML variant, on purpose** — a `Raw(String)` node would be a
  sanitizer bypass by construction. `DefinitionList` exists because Sphinx renders every API
  entry as a `<dl>`. Unknown admonition kinds render as notes, never dropped.
- S1-3 maps the flat YAML sync fields into `SyncStrategy` (the enum makes
  `schedule`-without-`scheduled` unrepresentable); S1-2 owns the DB mapping.

### Done: the S1 fan-out — S1-2/3/4/7 all merged 2026-07-28

| | Where | Worth knowing |
|---|---|---|
| S1-2 | `tome-core/src/db.rs` (#20) | STRICT tables, bundled SQLite; `icon`/`sync` stored as JSON in the frozen serde shape; upsert is `ON CONFLICT DO UPDATE`, never `INSERT OR REPLACE` (REPLACE cascades and empties a source's pages); rows re-validate through the model on read; `SyncState` deliberately absent (ADR-0005) |
| S1-3 | `tome-core/src/config.rs` (#19) | Two layers: raw serde with `deny_unknown_fields`, then per-type `SourceSpec` variants; selectors validated by the same engine the parser uses; rate clamped to the 4 req/s cap; cargo-deny forced two dep swaps (serde_yaml→serde_yaml_ng, scraper 0.24→0.27 over fxhash) |
| S1-4 | `tome-core/src/fetch/` (#21) | **Sync client (ureq 3), manual redirects** — every hop passes robots + rate limit, and **S1-5's SSRF filter slots into the marked seam in `Fetcher::fetch`**. robots.txt hand-rolled (RFC 9309 subset; 5xx/unreachable ⇒ disallow; Crawl-delay capped 60 s; iterative wildcard matching — recursive is exponential on hostile rules) |
| S1-7 | `tome-core/src/parse.rs` (#22) | Content-root discovery then walk; unknown wrappers unwrap but keep `id` as `Anchor`; Sphinx pilcrows stripped, `highlight-<lang>` found by climbing ancestors; streaming parsing deliberately not implemented (SPIKE-002 numbers); links resolved beside the AST, hrefs inside as-written |

Four more fuzz targets live (`model_ids`, `source_config`, `robots`, `html_parser`), each
asserting an invariant, not just no-panic. The paths→`&SourceId` migration (#18) closed the S0
containment gap. 105 tome-core tests.

### S1-5 (SSRF filter) — done, and the refute-panel earned its keep

`tome-core/src/fetch/ssrf.rs` (classifier) + `resolver.rs` (installed as ureq's resolver, so the
validated addresses are the exact ones dialed — closes DNS-rebind TOCTOU). The **adversarial
refute-panel ran as a Workflow** (owner opted in): 4 Opus refuters by attack lens → 3 Opus judges
per candidate → majority-real gates merge. **Round 1 confirmed 4 distinct real bypasses** and they
were fixed before merge:

1. NAT64 `64:ff9b::/96` embedded IPv4 not unwrapped → reached `169.254.169.254`. Now `embedded_v4()`
   unwraps mapped/compatible/IPv4-translated/NAT64-WKP/6to4 to v4 and classifies as v4.
2. Site-local `fec0::/10` classified Public → the structural fix: **classify_v6 default-denies
   outside global unicast `2000::/3`** instead of naming bad ranges (the allowlist-by-omission was
   the root cause).
3. IPv4-translated `::ffff:0:0/96` (SIIT) not unwrapped — now handled.
4. **`HTTP_PROXY`/`ALL_PROXY` bypassed the filter entirely** (ureq defaults to
   `Proxy::try_from_env()`, dials the proxy so the destination is never classified) → `.proxy(None)`
   on the agent config. A docs fetcher must never tunnel through an ambient proxy.

The lesson worth keeping: **the first draft looked correct and passed its own tests; the panel
found what the author's tests could not.** Do the same for S1-9. Round 2 re-ran against the
hardened code to confirm the fixes held; see the plan's S1-5 row / the PR for its verdict.

### Done: S1-6 (crawl) and S1-8 (normalization), merged 2026-07-28

| | Where | Worth knowing |
|---|---|---|
| S1-6 | `tome-core/src/crawl/` (#24) | BFS over Fetcher+parser; politeness/SSRF inherited, not re-done. **Link discovery is whole-document, not content-root** — nav is how a site advertises pages (caught by a failing test). Errors per-page, never fatal; `hit_page_cap` explicit. Entry = depth 1. nodejs.org trap handled in `UrlFilter` (`include: ^/api/`). Dependency-free SHA-256 lands here for `ContentHash`. |
| S1-8 | `tome-core/src/normalize.rs` + `corpus/normalization` (#25) | URLs→absolute, headings shifted so shallowest=h1, code langs aliased, empties pruned, title/description extracted. Functional transforms over the typed AST (not the PRD's `dyn Transform`). **Golden corpus is SEEDED** with the repo's Sphinx fixture (licence-clean) — the `<dl>` API case comes through with its permalink id. `NormalizedPage` is now Serialize (part of the freeze). |

**S1-8's remaining acceptance:** the golden corpus is ≥20 real sites, not 3 fixtures. `SOURCES.md`
records the gate; the real pages (Python PSF-2.0, Go/K8s CC-BY, Rust/Node MIT/Apache) are cleared
by SPIKE-010 but not yet fetched+committed — deliberately deferred, not forgotten.

### Done: S1-9 (sanitizer) — the whole ingestion backend is complete

`tome-core/src/sanitize.rs` (#26). Refute-panel confirmed 5 defects round 1 (interior `\t\n\r`
scheme bypass, non-ASCII ids emptied, rustdoc anchors broken, id/fragment desync, unguarded
`language`/free-text fields), all fixed; round 2 held. **`sanitize_id` is a denylist, not an
allowlist** — keeps unicode + rustdoc/Sphinx punctuation, strips only whitespace/quotes/control.
Free-text attribute fields (`title`/`alt`) are the **renderer's escaping job — a documented hard
contract S1-13 must honour** (quote every attribute, escape every value + text node).

**Both refute-panel tickets (S1-5, S1-9) found real bugs the author's green tests missed.** That
is the standing lesson for any future security-critical work.

**Stage 1 backend done: S1-1..S1-9.** config → fetch (robots/rate-limit/SSRF) → crawl → parse →
normalize → sanitize, all fixture-tested. ~135 tome-core tests, 7 fuzz targets.

### Done: S1-10 (asset localization) — the offline guarantee is real

`tome-core/src/assets.rs` (#27). `AssetStore` trait splits the AST rewrite from fetch/store;
`FetchingAssetStore` content-addresses `assets/<sha256>.<ext>` through the shared Fetcher, dedups,
caps. Failed assets become inline "unavailable offline" notes, never live remote refs. **The
offline exit-gate test passes.** SHA-256 now lives in `crate::hash` (shared by crawl + assets).
Deferred + flagged: srcset/video/inline-style `url()` aren't in the typed AST (nothing to smuggle);
SVG byte-sanitization and asset GC are noted follow-ups.

### Next: the reader half (S1-11..S1-15) — a different mode of work

The backend is a pure Rust pipeline tested against a fixture server. The reader is
Rust-renders-to-HTML **plus the Svelte/iframe frontend**, so it needs a running app to verify, not
just `cargo test`. Remaining tickets:

- **S1-11 (syntax highlighting).** Architecture call to make first: highlighting is a **render
  concern, not an AST mutation** — the `CodeBlock` node stays `{language, code}`, and highlighting
  happens when the AST is rendered to reader HTML. Build it as a standalone
  `highlight(code, language) -> classed HTML` (syntect with `ClassedHTMLGenerator`, so themes are
  CSS the S1-12 tokens supply and light/dark switch without re-highlighting — offline-friendly, no
  client JS, works under the strict CSP). S1-13 calls it during render. syntect is a heavy dep
  (bundled syntax/theme dumps) — confirm that's acceptable or pick a lighter highlighter.
- **S1-12 (typography + design tokens).** CSS variables per `docs/plans/15-design-system.md` and
  P1-015 (New York serif body, SF Mono code, 17px/1.6, 70ch measure, light/dark). Frontend CSS.
- **S1-13 (reader iframe + IPC bridge) — Opus, the big one.** Inherits everything from SPIKE-002:
  one postMessage per page, external bootstrap script, `capabilities/default.json` already exists,
  CSP names the app origin, `__TAURI_INTERNALS__` unreachable from the frame. **It must honour the
  sanitizer's renderer contract: quote every attribute and HTML-escape every attribute value and
  text node** (S1-9 depends on this — it is why title/alt aren't charset-stripped). The
  AST→HTML renderer lives here (or in a `render` module it calls). Frame-pacing is the interactive
  acceptance item SPIKE-002 couldn't measure headlessly.
- **S1-14 (three-panel layout, sidebar, TOC)** and **S1-15 (navigation + history).** Frontend.

**Stage 1 exit gate** (`docs/plans/18` § Stage 1): the app renders `docs.python.org` with the
network off, images included, anchors working, golden corpus committed. The pipeline half of that
is done and offline-proven; the reader half makes it visible.

### Then: Stage 1 — the vertical slice

One real docs site → fetched → normalized → sanitized → asset-localized → **rendered offline**.
Full ticket table in `docs/plans/18-implementation-plan.md` § Stage 1. The shape of it:

- **S1-1 (freeze `Source`/`Page`/`Node`/`DocSet`) is serial and blocks the whole stage.** Opus.
  Everything else fans out behind it, so it is worth spending real time on and not worth rushing.
- **S1-5 (SSRF filter)** and **S1-9 (sanitizer)** are the two Opus-plus-adversarial-verify tickets.
  Prompt the verifiers to *refute* — "find an input that defeats this filter" beats "review this
  filter" — and let a majority-refute kill the change.
- The sanitizer must pass **two** corpora and both matter: XSS payloads (nothing survives) *and*
  anchors (nothing breaks). The original allowlist stripped `id`, which would have silently
  disabled the TOC — a security control breaking a headline feature. One corpus cannot catch that.
- **The infrastructure to use is now there, so use it rather than inventing per-ticket harnesses:**
  every fetch/crawl test drives `FixtureServer`; normalization is judged by a golden suite under
  `crates/tome-core/corpus/`; the HTML parser gets its fuzz target on day one, not after it works
  (`fuzz/README.md` names the planned targets and the invariant each should assert).
- **The offline assertion is the exit gate**: shut the fixture server down, render, assert the HTML
  contains no `http` references. `FixtureServer::shutdown()` makes the port refuse connections
  precisely so that a page still reaching for the network fails loudly instead of quietly passing.

---

## Traps already hit — don't rediscover these

Each of these cost real time. They are fixed; this is so a future change doesn't reintroduce them.

**Test infrastructure (new, from S0-6/7/8)**

- **macOS accepted sockets inherit `O_NONBLOCK` from the listener** (BSD behaviour; Linux does
  not). The accept loop polls non-blocking so shutdown is prompt, so every accepted stream needs an
  explicit `set_nonblocking(false)` — without it every read returns `WouldBlock` and no request is
  ever served.
- **Percent-decode before checking for `..`**, not after. `/%2e%2e%2f` is the standard way that
  check gets defeated; there is a test for each encoding.
- **Fixtures and corpora must be excluded from Prettier *and* ESLint.** `searchindex.js` mimics
  Sphinx's output and references a global the real page defines — ESLint calls that `no-undef`, and
  Prettier reformatting a golden file means it no longer records what the pipeline produced.
- **`fuzz/` needs `[workspace]` in its own `Cargo.toml` *and* `exclude = ["fuzz"]` in the root**,
  or `libfuzzer-sys` and nightly leak into every `cargo test`.
- A proptest generator for "arbitrary absolute root" will happily generate `/~`, which fails the
  no-literal-tilde property for a reason that is not a bug. The generator excludes `~`; the
  property is about what Tome constructs, not about what a user may name a directory.

**App shell (from SPIKE-002 — details in `docs/spikes/002-reader-iframe-bridge.md`)**

- **Tauri core APIs are deny-by-default and fail silently when the rejection is not awaited.**
  No `src-tauri/capabilities/` directory meant `listen()` rejected and 180 events vanished with
  no error. `capabilities/default.json` grants `core:event:default` only; extend it deliberately.
- **An occluded window suspends rAF entirely and clamps timers to ~230 ms** (WKWebView). Nothing
  in the reader may gate correctness on rAF, and headless runs cannot measure frame pacing.
- **`'self'` in a CSP inside the sandboxed frame matches nothing** — the origin is opaque. Name
  the app origin explicitly (from `location.origin`), and keep the frame bootstrap external:
  the srcdoc document inherits the app CSP, which has no `unsafe-inline` for scripts.

**Build / tooling**

- **`Icon?` in the user's global gitignore matches `icons/`** case-insensitively on macOS, silently
  excluding the entire app icon set. `.gitignore` has explicit `!src-tauri/icons/` negations. Caught
  only by cloning fresh — **do that before any release**.
- **Svelte 5 + Vitest** resolves the *server* build and `mount()` throws
  `lifecycle_function_unavailable`. Fixed with `conditions: ['browser']` scoped to `process.env.VITEST`.
- **`$lib` needs two aliases**: `paths` in `tsconfig.json` (type checker) *and* `resolve.alias` in
  `vite.config.ts` (bundler). One alone fails at the other layer.
- `vite.config.ts` must import `defineConfig` from **`vitest/config`**, not `vite`, or the `test`
  key is a type error.
- Workspace lints **deny `unwrap`/`expect`**. Test files need a file-level
  `#![allow(clippy::expect_used, clippy::unwrap_used)]` — panicking on setup failure is correct there.
- **Prettier deliberately excludes markdown.** The docs are hand-aligned; reflowing produces huge
  diffs that bury real changes. Don't "fix" this.
- `npm` `overrides: { "brace-expansion": "^5.0.8" }` clears a transitive high-severity advisory.

**cargo-deny**

- Graph is scoped to `aarch64-apple-darwin`, which excludes Tauri's Linux GTK backend and its ten
  unmaintained advisories. Narrowing beats ignoring IDs — if those crates ever *do* enter a macOS
  build we want to hear about it.
- Five rust-unic advisories (via `tauri-utils` → `urlpattern`) are listed **individually with
  reasons**, deliberately not `unmaintained = "warn"`, so a *new* unmaintained crate still fails.
- A path dependency with no `version` reads as a wildcard error. Workspace dep has `version = "0.0.0"`.

**macOS / distribution**

- **`Apple Development` ≠ `Developer ID Application`.** The cert on this machine (TV35L29SN5) is a
  *development* cert and **cannot sign for distribution**. Only the paid program gives Developer ID.
- **macOS 15 removed the Control-click→Open Gatekeeper bypass.** Caveats must lead with
  `xattr -dr com.apple.quarantine`, which works on every version.
- `brew style` **refuses to lint a cask outside a tap.** Copy into
  `$(brew --repository)/Library/Taps/alexnodeland/homebrew-tap/Casks/`, lint, copy back, then
  **remove it** — the user's tap must stay clean until release.
- The built app is `adhoc, linker-signed`, `TeamIdentifier=not set`. `spctl` rejects it. Expected.

---

## Open — needs the user, don't decide alone

- **DEC-005** docset import priority · **DEC-006** `watch` fetch vs notify · **DEC-007** note format ·
  **DEC-008** export targets. All non-blocking.
- **PR #2** — the stale scaffolding branch, superseded. Left open; ask before closing.
- **Dependabot opened nine PRs (#5–#13)** right after #4 merged. Several are majors (TypeScript 7,
  eslint-plugin-svelte 3, prettier-plugin-svelte 4, jest-dom 7) and the Actions bumps are moot
  until Actions can run. Untouched — batching the dev-dep majors through `check.sh` is a chore
  worth doing deliberately, not silently.
- ~~Which docs site S1 targets first~~ — **answered 2026-07-28: `docs.python.org`**, as the
  plan's exit gate names. S1-7/S1-8 fixtures and the first corpus suite are Sphinx-first;
  the `<dl>`-based API entries are the hard case, and `DefinitionList` in the frozen AST
  exists for exactly them.
- **Going public + Actions billing.** They said they'll flip to public "once we're pretty much done
  and ready to release". Until then CI stays red-but-meaningless.
- **Suggested to them, no response yet:** backport the macOS 15 Gatekeeper caveats fix to
  `curio.rb` in their tap — its caveats lead with right-click, which no longer works on 15+.

---

## Standing decisions (owner, 2026-07-28)

- **Refute-panel is the standing method for security-critical tickets** — run the Workflow
  automatically (no re-ask) for S1-9 and any future SSRF/auth/sync work; majority-refute gates the
  merge. This is a durable opt-in, not per-ticket.
- **Keep merging green PRs ticket-by-ticket** through the rest of Stage 1.
- **Real golden-corpus expansion is deferred until the reader renders** — validate real pages
  visually then, not as JSON diffs now.

## Working with this user

- They want **costs stated, not glossed**. The ADRs say plainly that Gatekeeper friction will lose
  users and that agents make breadth cheap but depth no cheaper. Keep that register.
- They chose the agent-driven build deliberately. Route by task shape: Fable for exactly-specified
  and loudly-verified work, **Opus for anything that fails silently** (sanitize, ssrf, paths, sync,
  auth) and for every verification pass.
- They have prior art worth checking before inventing: `alexnodeland/homebrew-tap` had the cask
  conventions already, including the CLI-symlink-out-of-bundle trick in `statusbar.rb`.
- One process note: a `screencapture` window-id lookup silently fell back to full-screen and caught
  an unrelated session of theirs. Deleted it immediately. **Don't screenshot the app** unless the
  window can be targeted reliably — functional verification is better evidence anyway.

## Verified environment

Rust 1.96.1 · Node 26.3.0 · npm 11.16.0 · tauri-cli 2.5.0 · macOS 26.5 · arm64 · Xcode CLT present.
`cargo-deny` installed; `cargo-audit` **not** installed (the script skips it and says so).
`cargo-fuzz` and a nightly toolchain are **not** installed — `check.sh` only type-checks the fuzz
targets, which stable can do, so nothing is silently skipped there.

37 Rust tests + 3 Vitest tests pass. `npm run tauri build --debug` produces `Tome.app` + DMG.
