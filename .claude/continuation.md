# Continuation note — Tome

**Written:** 2026-07-28, before context compaction.
**Delete this file when Stage 1 lands.** It is in-flight task state, not documentation.

---

## Where things stand, in one paragraph

Tome was a docs-only repo: a PRD plus 18 planning documents, no code. I audited the whole plan
(PR #3, **merged**), then wrote an agent-driven implementation plan and built the Stage 0 scaffold
(PR #4, **open, mergeable, not yet merged**). The app builds, launches, and creates its library at
the right paths; the CLI is a stub except `tome status`. Nothing else works yet — no ingestion, no
reader, no search.

```
main                                    ← PR #3 merged (plan audit)
feat/implementation-plan-and-scaffold   ← PR #4 open, 2 commits, tree clean
  e3ddbca  unsigned distribution via own tap; local verification gate
  2acc9a2  implementation plan and Stage 0 scaffold
```

## First thing to run

```bash
cd /Users/alexandernodeland/anodeland/projects/code/apps/tome
./scripts/check.sh --fast     # ~40s. Should print "All checks passed."
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

### Immediately: S0-6, S0-7, S0-8 — all three gate Stage 1

These are the "make agent output verifiable" foundations. **Build them before any ingestion code**;
that ordering is the whole point of the plan (`18-implementation-plan.md` § Why this is not just
the old plan with dates).

| | Work | Notes |
|---|---|---|
| **S0-6** | Fixture HTTP server | Serves committed doc-site fixtures offline. Every scraper test needs it. Must be able to **shut down mid-test** so the offline assertion works. |
| **S0-7** | Golden-corpus harness | Snapshot + diff normalized output across ≥20 real sites. This is how normalization quality becomes reviewable at all. |
| **S0-8** | Property-test + fuzz scaffolding | `proptest` + `cargo-fuzz`. Targets get added per-module later; the scaffolding lands now. |

Suggested order: S0-6 first (S0-7 depends on having fixtures to serve), then S0-7, then S0-8.

### Then: SPIKE-002 before Stage 1

**Must actually run, not be reasoned about.** The original plan's Unix-socket MCP design is what
happens when an agent answers a question about an external system from memory. Paste real output.

Also worth running early: **SPIKE-010** (legal posture, 1 day) before the registry gets entries.

### Then: Stage 1 — the vertical slice

One real docs site → fetched → normalized → sanitized → asset-localized → **rendered offline**.
`S1-1` (freeze core types) is serial and blocks everything else in the stage.

The two Opus-and-adversarial-verify tickets in S1 are **S1-5 (SSRF filter)** and **S1-9
(sanitizer)**. The sanitizer must pass *two* corpora: XSS payloads (nothing survives) **and**
anchors (nothing breaks). The original allowlist stripped `id` and would have silently disabled the
TOC — a security control breaking a headline feature.

---

## Traps already hit — don't rediscover these

Each of these cost real time. They are fixed; this is so a future change doesn't reintroduce them.

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
- **Merging PR #4** — not merged yet. Ask before merging; they merged #3 explicitly.
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

12 Rust tests + 3 Vitest tests pass. `npm run tauri build --debug` produces `Tome.app` + DMG.
