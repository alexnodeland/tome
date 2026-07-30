# Continuation — Stage 4 is built; the release is not cut

**Written:** 2026-07-30, after S4-10 landed. **Rewrite or delete this file when the release ships.**

In-flight state only: what is done, what is decided, what to do next. It deliberately carries **no**
durable knowledge — mistakes and invariants live in [`.claude/traps.md`](traps.md), which does not go
stale. Do not let this file grow a "traps" section; an earlier one was deleted for exactly that.

---

## Stage 4 is built

| | Ticket | Landed as |
|---|---|---|
| S4-9 ✅ | The CLI ships inside the app bundle | `bundle.externalBin`, `scripts/build-cli-sidecar.sh`, `scripts/verify-bundle.sh` |
| S4-8 ✅ | DMG, release workflow, tap cask | `packaging/homebrew/Casks/tome.rb`, `.github/workflows/release.yml` |
| S4-3 ✅ | Error taxonomy audit + recovery | `error.rs`'s exhaustive `suggestion`, `logging.rs`, `tome debug` |
| S4-4 ✅ | Onboarding, registry-first | `src-tauri/src/onboarding.rs`, `Onboarding.svelte` |
| S4-5 ✅ | Preferences | `Preferences.svelte`, `$lib/appearance` |
| SPIKE-001 ✅ | Menu bar without Swift | [`docs/spikes/001-menu-bar.md`](../docs/spikes/001-menu-bar.md) |
| S4-6 ✅ | Menu bar + global shortcut | `src-tauri/src/tray.rs`, `$lib/accelerator` |
| S4-2 ✅ | Performance, measured | `scripts/measure-startup.sh`; the page list is windowed |
| S4-7 ✅ | Accessibility | `$lib/a11y`'s `trapFocus` — the one real gap |
| S4-1 ✅ | Bug hunt | three defects in one function, plus pruning |
| S4-10 ✅ | User docs | `site/pages/help.html`, and the rest brought up to date |

## The exit gate needs a tag

`brew install --cask alexnodeland/tap/tome` is the gate, and as of 2026-07-30 **one thing stands in
front of it**: no tag has been pushed. `git tag v0.1.0 && git push --tags` runs `release.yml`.

The tap exists, carries the cask, lints it in CI, and bumps itself from this repository's latest
release — verified by dispatching it. Actions works; CI is green across all five jobs.

Then it has to be **installed on a machine that has never built Tome**. That is the part the plan
insists on and the part no check here can do: a DMG missing the CLI, a zap list that leaves data
behind, or an app that will not launch all pass every automated check and fail for every user. Two
of those three now have automated checks (`scripts/verify-bundle.sh`); the third does not.

## What is weakest in what was built

- **The startup budget is missed** — 625 ms median against 500 ms — and **10 ms of it is Tome's own
  code**. The rest is Tauri and WKWebView creation. There is no hot spot to find; the honest choices
  are to accept it or to show a window before the webview is ready.
- **The idle-memory figure excludes the webview.** 118 MB is the app process; `ps` does not
  attribute WKWebView's own processes to it.
- **The reserved-shortcut list is a snapshot** of macOS 26's defaults. A user who has rebound their
  own system shortcuts will find it both incomplete and occasionally wrong.
- **The registry still has four sources** against a v1.0 target of thirty. Unchanged since S3-8, and
  now it is also what onboarding shows on first launch.
- **Nothing has run against a large real library.** Every end-to-end test uses the 4-page fixture;
  the 20 000-page numbers in S4-2 come from synthetic rows, not a crawl.
- **The DMG has never been installed by anyone.** It has been built, mounted and inspected.

## Next

**Stage 5 (sync) is deferred** and unscheduled — see `docs/plans/18` § Stage 5. Before it, in
rough order of value:

1. **Cut a release.** The three non-code items above.
2. **Grow the registry.** Four to thirty is the gap between "the registry works" and "onboarding
   works", and it is roughly a weekend per ten.
3. **Playwright E2E.** `main` still has none; the frontend is Vitest-only, so nothing exercises the
   reader frame, the sandbox, or onboarding in a real browser.
4. **The remaining spikes.** Seven are Not Started; SPIKE-011 (sanitizer against real docs) and
   SPIKE-005/006/007 (mandoc, Sphinx, rustdoc index formats) all inform work that is already built,
   so they would now be *audits* rather than spikes.

## How to run the gates

```bash
./scripts/check.sh                      # everything, including the app bundle
./scripts/check.sh --fast               # everything except the bundle

cargo test -p tome-core --test relevance -- --nocapture          # search quality
cargo test -p tome-core --test detection -- --nocapture          # platform detection
cargo test -p tome-core --test pruning                           # the deletion guards
cargo test -p tome-core --test registry                          # registry, offline
cargo test -p tome-core --test search_bench --release -- --nocapture   # latency

./scripts/verify-bundle.sh <path/to/Tome.app>   # what a user installs
./scripts/measure-startup.sh 7                  # startup and idle memory
./scripts/verify-registry.sh                    # registry, LIVE — deliberately not in check.sh
./scripts/check-cask.sh --fix                   # brew style, via a throwaway tap
./scripts/set-version.sh 0.1.0                  # one version, everywhere it is written
```

`measure-startup.sh` and `verify-registry.sh` are deliberately outside `check.sh`: one moves with
whatever else the machine is doing, the other fails when someone else's website is down. Both would
be sources of flakes rather than information in a gate.

The MCP handshake against real Claude Code cannot run in the suite (it needs a logged-in client).
The command shape is in the SPIKE-008 write-up; re-run it by hand when the protocol handler
changes.

Update modes — `TOME_UPDATE_GOLDEN`, `TOME_UPDATE_BASELINE`,
`TOME_UPDATE_DETECTION_BASELINE` — all **fail the run they change anything in**, on purpose. The
passing run is the one after the diff has been read.

## Still open — do not decide alone

- **`two-face`** for TypeScript/TOML syntax highlighting. A licence decision, not a technical one.
- **DEC-005** docset import priority · **DEC-006** `watch` fetch vs notify · **DEC-007** note
  format · **DEC-008** export targets. DEC-006 has code waiting on it (`sync::due` returns
  `WatchUndecided`); the rest remain non-blocking.
- **PR #10 (TypeScript 7)** — left open deliberately as a reminder. `npm ci` fails outright:
  `svelte-check@4.7.4` peers on `typescript@^5 || ^6`. Re-check when svelte-check and
  typescript-eslint support TS 7.
- **GitHub Pages is not enabled.** `pages.yml` builds and the deploy 404s: *"Ensure GitHub Pages
  has been enabled"*. One setting — **Settings → Pages → Source = GitHub Actions**.
- **`release.yml` has still never run.** CI is green, but the release workflow only fires on a tag,
  and no tag exists. Expect it to need a fix on its first run, the way `ci.yml` did.
- **The tap now owns cask bumping.** `alexnodeland/homebrew-tap` reads this repository's latest
  release on a schedule, so a release here needs no credential for that repository —
  `.github/workflows/release.yml` deliberately does not touch it. Opened as
  [homebrew-tap#1](https://github.com/alexnodeland/homebrew-tap/pull/1).
- **The 500 ms startup target.** Missed, measured, and left in place rather than moved to match the
  measurement. Accepting ~600 ms or faking it with an early window is the owner's call.

## Environment

Rust 1.96.1 · Node 26.3.0 · npm 11.16.0 · tauri-cli 2.5.0 · macOS 26.5 · arm64.
`cargo-deny`, `mandoc` and `brew` present; `cargo-audit`, `cargo-fuzz`, and nightly are **not** (the
gate says so). Bash is 3.2 — no `mapfile`, no `${arr[-1]}`. 594 workspace Rust tests + 176 Vitest (measured 2026-07-30).
`npm audit` hits the live registry and fails the gate when npmjs.org is down — that is an outage,
not a finding.
