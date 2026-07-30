# Changelog

All notable changes to this project are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] — 2026-07-30

**The first release.** Ingestion, the reader, search, agent access over MCP, and a local HTTP API
all work; the app ships with the `tome` CLI inside it so the two resolve one library.

**Not** in this release: bookmarks, annotations, and cross-device sync — designed, deferred, and
absent rather than stubbed. Tome is **unsigned**, so macOS blocks the first launch; the cask's
caveats carry the `xattr` command.

Everything below happened before this tag and is kept in the order it was written.

### Added — 2026-07-30 — the release pipeline (S4-8, S4-9)

- **The `tome` CLI now ships inside the app bundle**, at `Tome.app/Contents/MacOS/tome`, as a
  Tauri `externalBin` sidecar. The cask symlinks it onto `PATH`, so one install delivers the app
  and the CLI **from one build** — which is what makes them resolve the same library (ADR-0002).
  `src-tauri/build.rs` refuses to compile the app without it staged, rather than letting a bundle
  ship without it.
- **`scripts/verify-bundle.sh`** — the only check that inspects the artifact a user installs:
  the CLI is present and executable, its architectures match the app's, it is byte-identical to
  the binary this tree built, its version matches `CFBundleShortVersionString`, it resolves
  `~/Library/Application Support/Tome`, and the cask's zap list covers every path it reports.
  Run by `scripts/check.sh` after the app build and by the release workflow before publishing.
- **A DMG with a designed window** — background generated from `public/tokens.css` by
  `scripts/make-dmg-background.mjs`, drag-to-Applications layout, volume icon. Unsigned, per
  ADR-0006.
- **`.github/workflows/release.yml`** — on tag: stamp the version, build, verify, publish the
  DMG, rewrite the cask's `version` and `sha256`, mirror it to `alexnodeland/homebrew-tap`.
- **`packaging/homebrew/Casks/tome.rb`**, actually committed this time, with a zap list whose
  every entry was observed on a machine that had run Tome.
- **`scripts/check-cask.sh`** — stages a throwaway tap so `brew style` will lint the cask at all.
- **`tome config forget-token`** — `brew uninstall --zap` removes files, and the Keychain is not
  a file. Without this the one secret Tome creates outlives the uninstall.
- **`scripts/set-version.sh`** — one version, written to `Cargo.toml` and `package.json`.
  `tauri.conf.json` deliberately has no `version` key so the bundle inherits Cargo's.

### Fixed — 2026-07-30 — CI's first three real runs

Actions started working. `ci.yml` had never executed a step since it was written, and it took three
fixes to go green — all of them things `scripts/check.sh` structurally cannot catch:

- **The secret scan was scanning nothing.** `actions/checkout` is shallow by default; gitleaks
  scans the range `<sha>^..<sha>`, so git failed and it logged `scanned ~0 bytes (0)` followed by
  `no leaks found in partial scan`. It failed the job only because it propagated git's error —
  **had it exited 0, the repository would have had a permanently green secret scan that had never
  read a line of code.** Fixed with `fetch-depth: 0`; the SARIF confirms zero real findings.
- **`rustsec/audit-check` was gating the checks that work.** It is not target-scoped, so it failed
  on `unmaintained` advisories for Tauri's Linux GTK bindings — never compiled into a macOS build
  — while reporting zero vulnerabilities, and it ran first, so cargo-deny, `npm audit` and the
  secret scan were all skipped. Removed from `ci.yml` and `check.sh` together.
- **A wall-clock gate with no headroom.** The indexing-cost assertion was `< 10 ms` per page; a
  shared runner measured 12.2 ms against ~1 ms locally. Raised to 50 ms, still an order of
  magnitude below the ~500 ms a politely-fetched page costs.

All five jobs now pass, including **Build app**, which stages the CLI sidecar and runs
`scripts/verify-bundle.sh` on a runner that has never built Tome.

`pages.yml` builds and does not deploy: Pages needs enabling in Settings.

### Changed — 2026-07-30 — the tap bumps itself, so the release needs no token

- **`alexnodeland/homebrew-tap` exists** and already carried three casks. Tome's is proposed in
  [homebrew-tap#1](https://github.com/alexnodeland/homebrew-tap/pull/1), which also pins the other
  three to real releases — they were all `version :latest` / `sha256 :no_check`, and that tap's
  README claimed they "update via `brew upgrade --cask`", which **`brew upgrade` does not do for
  `:latest` casks without `--greedy`**.
- **The release workflow no longer touches the tap.** The tap bumps its own casks on a schedule by
  reading this repository's latest release, so a release here needs **no cross-repository token**
  at all — no `HOMEBREW_TAP_TOKEN` to create, hold or rotate. Publishing the release is the whole
  handoff.
- **CI still has never executed a step**, and the reason changed. The repository is public now and
  Actions dispatches; every run is refused at the runner with "recent account payments have failed
  or your spending limit needs to be increased", which applies to free public-repo minutes too.
  Documented wherever the old "Actions is blocked at the account level" wording appeared.

### Added — 2026-07-30 — user documentation for what now exists (S4-10)

- **A troubleshooting page** — `site/pages/help.html`. Worth having only because S4-3 gave it
  commands to recommend: `tome debug check`, `rebuild-index`, `report`. It ends with "things that
  are not bugs", which is the half of an FAQ that earns its place.
- The guide covers installing, onboarding, preferences, the menu bar and the global shortcut; the
  CLI reference covers `debug` and `config forget-token`; the landing page no longer says there is
  no DMG.

### Fixed — 2026-07-30 — the bug hunt (S4-1)

- **`tome_get_page` panicked on any page over 48 KiB whose budget boundary landed inside a
  multi-byte character.** `&s[..n]` is not byte-safe, and documentation is full of em dashes and
  curly quotes. The same bug had been written and fixed in `fetch::robots`, with a comment
  explaining the trap, three files away. `tome_core::text::truncate_at_char_boundary` is now the
  one implementation.
- **The same function returned zero content** for a page whose first 48 KiB contained no blank
  line — one long table, one long code block — reporting "showing 0 of N KiB" as though that were
  a result. It now falls back to a line break, then to the boundary.
- **And it could cut inside a code fence**, rendering the truncation notice and everything after
  it as code. It closes an unbalanced fence.

### Added — 2026-07-30 — pruning pages the site no longer has (S4-1)

Agreed 2026-07-29, recorded on `Database::delete_page`, unimplemented until now: a page removed
upstream stayed in the library and in search for ever.

`pull` deletes pages it did not see — but **only** when the crawl is trustworthy, and the guard is
the whole feature. Three conditions, and implementing it found that the first draft of the guard
was wrong twice:

- **Not capped.** A crawl that stopped at `max_pages` saw a prefix of the site.
- **No *ambiguous* errors.** The first version refused to prune if the crawl reported any error at
  all — and a page removed upstream reports **404**, so the one case pruning exists for was the
  case that disabled it. 404 and 410 mean gone; 5xx and timeouts mean unknown.
- **It produced at least one page.** A moved site or a 404 entry point is a crawl that completes
  cleanly with no ambiguous errors and finds nothing. Without this guard, that empties the
  library.

Five tests, four of which assert that nothing was deleted.

### Added — 2026-07-30 — measured performance, and a focus trap (S4-2, S4-7)

- **`scripts/measure-startup.sh`** — startup and idle memory, with a stated definition of what it
  measures and what "cold" does not mean. Release build, 7 runs: **625 ms median** against a
  500 ms budget, **118 MB idle** against 200 MB.
- **The page list is windowed** (P5-002). A 20 000-page source is 19 ms out of SQLite and 1 ms to
  serialise — the backend is fine — but it was also 20 000 DOM nodes. The sidebar renders 200 rows
  and says how many remain.
- **`trapFocus`** — Tab stays inside the search and preferences modals. `aria-modal="true"` tells
  assistive technology the rest of the page is inert and does **nothing** to the keyboard; without
  this, Tab moved focus behind the overlay, where the focus ring is invisible and the next Return
  activates something the user cannot see.

### Changed — 2026-07-30

- **The startup target is recorded as missed**, with the breakdown, rather than moved to match
  what was measured. **10 ms of the 625 is Tome's own code**; the rest is Tauri and WKWebView
  process and window creation, and a debug build measures the same.
- **The syntax-set warm-up comment was wrong for two stages.** It claimed to keep "several
  megabytes of inflated syntax dumps" off the first page view. Measured: **0 ms**. syntect's
  bundled defaults are lump data that is not parsed at load. The call is kept — it costs nothing
  either — but its justification is now a measurement.
- **`scripts/verify-bundle.sh` picks the most recently built bundle**, not release-first. The old
  order meant a tree holding both would verify one nobody had just built, then fail the same-build
  digest check against the sidecar staged for the other — a false negative that reads exactly like
  a real defect.

### Added — 2026-07-30 — the menu bar, and a global shortcut (SPIKE-001, S4-6)

- **A menu bar item**, with no Swift. SPIKE-001 asked whether native menu bar integration needs an
  AppKit shell; it does not. Tauri's `tray-icon` feature *is* `NSStatusItem`, and the whole thing
  is 170 lines of Rust with no `unsafe` and no bridge. Left-click raises the window and opens
  search; right-click opens a menu. The glyph is a template image so macOS recolours it for light,
  dark and highlighted states.
- **A system-wide shortcut**, ⌘⇧D, **off by default** — one claimed at first launch is one taken
  from whatever the user had bound to it. Rebindable by a recorder that captures the next
  keystroke.
- **Hide from the Dock**, making Tome menu-bar-only. The menu bar item is created unconditionally
  and first, so this never leaves someone with no way back in.

### Fixed — 2026-07-30 — conflict detection that actually detects

- **A registered global shortcut is not a working one.** SPIKE-001 measured that registering
  `⌘Space` — Spotlight's — **succeeds**, and the handler then never fires, because macOS consumes
  the keystroke first. `RegisterEventHotKey` refuses a combination held by another *application's*
  hotkey, not one held by the system. The first draft of `tray.rs` said "the failure IS the
  detection" in a comment; it was wrong, and one experiment showed it. Detection is now two-sided:
  the registration error, plus a refusal list and a two-modifier minimum in the frontend.

### Added — 2026-07-30 — a first run that is not a configuration exercise (S4-4, S4-5)

- **Onboarding installs a documentation source in one click**, from the registry, on first launch.
  The catalogue **ships inside the app bundle** as a Tauri resource, so the list renders with no
  network; installing goes through the ordinary pipeline, from the documentation's own origin,
  with robots.txt, the rate limit and the SSRF filter all inherited. Progress is pushed while the
  pull runs — the crawl phase deliberately shows a count and no denominator, because the total is
  unknown until the crawl ends.
- **Preferences (⌘,)** — theme, text size, column width, code line numbers, confirm-before-remove,
  and where the library lives. Changes apply immediately, to the DOM, to storage, **and to the
  reader frame**: `ReaderFrame.settings` was built in S1-13 and had never had a caller, so a
  theme change would have repainted the chrome and left the page alone.
- **⌘= / ⌘- / ⌘0** step the text size, on the same scale the panel offers.
- **`data-measure`** joins `data-theme`, `data-text-size` and `data-line-numbers` as a preference
  attribute in `tokens.css`. In `ch`, so the column holds the same number of words at any text
  size.
- **`tome_core::registry`** — a typed reader for the registry index, used by the app. The loose
  parser in `tests/registry.rs` still *defines* the shape, and the two are checked against each
  other.

### Added — 2026-07-30 — errors that say what to do, and a way to repair (S4-3)

- **`tome debug check`** — reports; never repairs. Library writable, database opens, every source
  configuration parses, index opens, and **the index and the database agree on every source's page
  count**. That last one is the failure with no symptom: an interrupted pull leaves the database
  ahead of the index and search quietly misses pages that are on disk.
- **`tome debug rebuild-index`** — discards the index and rebuilds it from pages already on disk,
  with **no network**. A discarded index previously left search silently empty until a full
  re-crawl; SPIKE-003 measured the difference at 5–21 seconds against about seven hours.
- **`tome debug report`** — a redacted bundle to paste into a bug report. No page paths, no search
  queries, no note text, `$HOME` rewritten to `~`. There is no telemetry and there will not be, so
  this is the only path from a broken machine to something a maintainer can read.
- **Logs, at last.** `~/Library/Application Support/Tome/logs/tome-<date>.log`, daily rotation,
  7-day retention. `logs/` had been in the PRD and created by `Paths::ensure_created` since S0-3
  and **nothing had ever written to it**. One `write_all` per event, so the app and the CLI cannot
  interleave mid-line; created on the first event, so a read-only command still creates nothing.
- **`Error::suggestion` is exhaustive** — no `_ =>` arm, so a new variant stops the build until
  someone decides what a person should do about it. Every command an error names is checked against
  a list of commands that exist, which is P5-004's own warning turned into a test.

### Fixed — 2026-07-30

- **Six error messages were sentence fragments** ("The download failed: connection reset"), found
  by the audit above on its first run. An interpolated detail now goes in parentheses and every
  message ends in a full stop.
- **`IndexSchemaOutdated` told users to run `tome pull --all`**, which works and also re-crawls
  every site to rebuild a file derived from content already on disk. It names
  `tome debug rebuild-index` now.

- **`npm run build` was deleting the Claude Code plugin and the cask.** Both were committed under
  `dist/` behind gitignore negations; `dist/` is Vite's `outDir` and Vite empties it on every
  build. They now live in `packaging/`, and `dist/` is gitignored outright.

### Changed — 2026-07-28 — distribution and verification

- **DEC-003 resolved: Apple Developer Program deferred** (ADR-0006). Tome ships **unsigned and
  un-notarized** through `alexnodeland/homebrew-tap`, following the cask conventions already used
  by `curio` and `statusbar`. Gatekeeper blocks first launch; the cask's `caveats` document the
  `xattr` fix and the click-through alternatives per macOS version. Revisit at v1.0.
- ~~`dist/homebrew/Casks/tome.rb` added as the cask's source of truth, validated with `brew
  style`.~~ **Neither half of that was true, and both were fixed on 2026-07-30.** The file was
  under `dist/`, which is Vite's output directory — `npm run build` empties it, so the cask was
  deleted by the next frontend build. And `brew style` refuses to lint a cask outside a tap, so
  nothing had ever linted it. See the 2026-07-30 entry.
- P5-010 (notarization) deferred but left intact, so enabling it later is a credentials change.
- **`scripts/check.sh` added.** The repository is private and GitHub Actions is unavailable, so
  this runs everything CI would — formatting, clippy, tests, svelte-check, eslint, prettier,
  vitest, `npm audit`, `cargo-deny`, `cargo-audit`, and the app build — and is the gate until the
  repository goes public.
- `cargo-deny` graph scoped to `aarch64-apple-darwin`, which removes ten unmaintained-crate
  advisories for Tauri's Linux GTK backend that are never compiled into a macOS build. The five
  remaining rust-unic advisories are listed individually with reasons rather than blanket-suppressed.
- Workspace path dependency given an explicit version, fixing a `cargo-deny` wildcard error.

### Added — 2026-07-28 — implementation plan and Stage 0 scaffold

- **Implementation plan** restructured for agent-driven execution: six stages with machine-checked
  gates, verification artifacts required before the code they verify, model routing by task shape,
  and a vertical slice ahead of breadth
- **Decisions resolved:** DEC-001 dual MIT OR Apache-2.0 (ADR-0003), DEC-002
  `com.alexnodeland.tome` (ADR-0004), DEC-004 solo + agent workflows with sync deferred (ADR-0005)
- **Cargo workspace:** `tome-core` (shared library), `tome-cli` (`tome` binary), `src-tauri`
  (desktop app). The app and CLI share one library, which is the constraint ADR-0002 exists to
  protect
- **`tome-core::paths`** — the only place a data path is constructed. State in
  `~/Library/Application Support/Tome`, cache in `~/Library/Caches/Tome`, `$TOME_HOME` override,
  `0700` directories. 9 unit tests plus a cross-binary integration test that runs the real `tome`
  binary and asserts it resolves the same paths the app links
- **Error taxonomy** frozen early, with user-facing messages that carry no user content
- **Svelte 5 + Vite + TypeScript frontend**, with the Tauri IPC boundary isolated behind one module
  so tests stub a single seam
- **CI:** fmt, clippy `-D warnings`, tests, `cargo-audit`, `cargo-deny`, gitleaks, `npm audit`,
  svelte-check, and an unsigned macOS build — with least-privilege token permissions
- **Hardened-runtime entitlements** with `allow-jit` as the only exception, and comments recording
  what is deliberately absent

### Changed — 2026-07-28 — plan audit

Full review of the PRD and all 18 planning documents, with fixes applied. Findings in
[`docs/reviews/2026-07-28-plan-review.md`](docs/reviews/2026-07-28-plan-review.md).

- Architecture corrected: Tauri is the application shell; the reader is a sandboxed iframe, not a
  second WKWebView; Swift demoted to an optional Phase-5 plugin
- App Sandbox disabled, resolving an incompatibility with the shared app/CLI data directory; data
  moved to standard macOS locations
- Local HTTP API secured: mandatory bearer token including on loopback, no CORS by default,
  Host/Origin validation, working SSRF filter
- MCP transport corrected from a non-existent Unix-socket transport to stdio
- Annotation anchoring changed from character offsets to W3C Web Annotation selectors
- Sync consolidated from three contradictory mechanisms to one iCloud Drive container design
- Effort reconciled against calendar; staffing assumption made explicit
- Critical path derived once and made consistent across three documents
- Test strategy replaced (Playwright cannot drive a Tauri app on macOS; Jest cannot compile Svelte)
- Unmeasurable success metrics removed or replaced with lab metrics and eval sets
- Added: 3 tickets (asset localization, relevance eval, detection corpus), 2 spikes (legal posture,
  sanitizer validation), 4 risks (legal, capacity, cold start, annotation drift), 8 recorded
  decisions
- 32 defects in specifications and code samples fixed in place

### Added — 2026-01-25

- Initial project plan: 5 phases, 87 tickets, and supporting documents
