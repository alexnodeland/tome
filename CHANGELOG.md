# Changelog

All notable changes to this project are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project will follow [Semantic Versioning](https://semver.org/) once it has releases.

## [Unreleased]

Nothing released yet. See [`docs/plans/18-implementation-plan.md`](docs/plans/18-implementation-plan.md).

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

### Fixed — 2026-07-30

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
