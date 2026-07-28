# Changelog

All notable changes to this project are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project will follow [Semantic Versioning](https://semver.org/) once it has releases.

## [Unreleased]

Nothing released yet. See [`docs/plans/18-implementation-plan.md`](docs/plans/18-implementation-plan.md).

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
