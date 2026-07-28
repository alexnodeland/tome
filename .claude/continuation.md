# Continuation note — Tome

**Written:** 2026-07-28. Updated the same day, after Stage 0 closed and PR #4 merged.
**Delete this file when Stage 1 lands.** It is in-flight task state, not documentation.

---

## Where things stand, in one paragraph

Tome was a docs-only repo: a PRD plus 18 planning documents, no code. The plan was audited
(PR #3, **merged**), then an agent-driven implementation plan and the Stage 0 scaffold landed
(PR #4, **merged**). **Stage 0 is complete** — the scaffold plus S0-6 (fixture HTTP server), S0-7
(golden-corpus harness) and S0-8 (property + fuzz scaffolding), so agent output on the ingestion
path is verifiable *before* it is written. The app builds, launches, and creates its library at the
right paths; the CLI is a stub except `tome status`. **No ingestion, no reader, no search — Stage 1
is where the product starts existing.**

```
main   ← everything below is merged; branch from here
  8eda146  fixture server, golden corpus harness, fuzz scaffolding   (S0-6/7/8)
  e3ddbca  unsigned distribution via own tap; local verification gate
  2acc9a2  implementation plan and Stage 0 scaffold
  8306422  plan audit                                                (#3)
```

**51 Rust tests + 3 Vitest tests pass; the full gate is green including the app build.**
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

### Immediately: SPIKE-002, the entry gate for Stage 1

**It must actually run, not be reasoned about.** The original plan's Unix-socket MCP design is what
happens when an agent answers a question about an external system from memory, and it survived
review because it read plausibly. Paste real output into the spike write-up; an agent's summary of
what a WebView bridge does is not evidence.

SPIKE-002 is the reader's IPC bridge: a sandboxed `<iframe>` inside Tauri's primary webview, page
HTML in, selection and scroll events out. What has to come back with real output attached:

- Can the iframe be given a restrictive CSP *and* still receive page HTML from the Rust side?
- What is the actual message shape and cost for a 500 KB page — one postMessage, or streamed?
- Does `window.getSelection()` inside the sandboxed iframe give ranges usable for annotation
  anchoring (quote + prefix/suffix), or does sandboxing break it?

Also worth running early: **SPIKE-010** (legal posture, ~1 day) — it gates committing real fetched
pages to the golden corpus, which S1-8 needs. `crates/tome-core/corpus/README.md` already says the
question is open; the corpus should not fill up before it is answered.

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
- **Which docs site S1 targets first.** The plan's exit gate names `docs.python.org` (Sphinx). If
  they would rather see rustdoc or mdBook render first, that changes S1-7/S1-8 fixture priorities
  and nothing else — worth one question before starting S1, not a blocker.
- **Going public + Actions billing.** They said they'll flip to public "once we're pretty much done
  and ready to release". Until then CI stays red-but-meaningless.
- **Suggested to them, no response yet:** backport the macOS 15 Gatekeeper caveats fix to
  `curio.rb` in their tap — its caveats lead with right-click, which no longer works on 15+.

---

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
