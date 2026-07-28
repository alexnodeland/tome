# Changelog

All notable changes to this project are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project will follow [Semantic Versioning](https://semver.org/) once it has releases.

## [Unreleased]

Nothing released yet. The project is in planning; see
[`docs/plans/00-project-overview.md`](docs/plans/00-project-overview.md).

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
