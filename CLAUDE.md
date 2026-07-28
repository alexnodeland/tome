# Tome — notes for AI assistants

> **Work in progress: read [`.claude/continuation.md`](.claude/continuation.md) first.**
> It has current branch/PR state, what to do next, and the traps already hit. Delete it when
> Stage 1 lands.

## What this repository is

Tome is a macOS documentation reader: ingest any documentation site, read it offline with good
typography, search across everything, and expose the library to coding agents over MCP.

**Mostly specification, with a working scaffold.** The app builds and launches, the path layer is
real, and the test infrastructure that makes later work verifiable — fixture server, golden-corpus
harness, property and fuzz scaffolding — is in place. Ingestion, the reader, and search do not
exist yet. When asked whether something works, check rather than assume — most of this repo
describes intent, not behaviour.

## Where things are

| | |
|---|---|
| `docs/PRD.md` | Product requirements. **The authoritative source** for architecture, the API and CLI surface, config schema, keyboard shortcuts, data locations, and success metrics. |
| `docs/plans/00-project-overview.md` | Master plan, phase gate, critical path, and the **ownership table** naming which document owns each shared fact |
| `docs/plans/01..05` | 90 tickets with acceptance criteria |
| `docs/plans/06..17` | Dependency map, spikes, testing, NFRs, CI/CD, risk, security, observability, versioning, design system, support, recovery |
| `docs/plans/18-implementation-plan.md` | **The execution plan** — stages, gates, model routing, orchestration patterns |
| `crates/`, `src-tauri/`, `src/` | The code that exists so far |
| `crates/tome-testkit/` | Test infrastructure: the fixture HTTP server and the golden-corpus harness. Dev-dependency only — never under `[dependencies]` |
| `fuzz/` | Fuzz targets. Its own workspace; needs nightly to run |
| `scripts/check.sh` | The verification gate, standing in for CI |
| `dist/homebrew/` | Cask source of truth, mirrored to `alexnodeland/homebrew-tap` on release |
| `docs/reviews/` | Point-in-time critical reviews of the plan |
| `docs/spikes/` | Results of spikes that have actually run, raw output included. `docs/plans/07` defines the spikes; this directory is where their answers live |
| `docs/decisions/` | Open decisions (DEC-*) and accepted ADRs |

## There is code now

`crates/tome-core` (shared library), `crates/tome-cli` (`tome` binary), `crates/tome-testkit`
(test infrastructure), `src-tauri` (the app), `src/` (Svelte frontend). The app builds and
launches; of the product itself, only the path layer is implemented.

**`./scripts/check.sh` is the gate — CI cannot run** (private repo, and Actions is blocked at the
account level). It runs exactly what `.github/workflows/ci.yml` runs. Change one, change the other.

## Rules for editing these documents

**Link, do not restate.** The single biggest defect found in the 2026-07-28 review was the same
fact living in three documents with three different values — the critical path, the memory targets,
the data directory, the keyboard shortcuts, the API base path, the entitlements file. Before adding
a table, check the ownership table in `00-project-overview.md`. If another document owns the topic,
link to it.

**Keep the corrective comments.** Many code samples now carry a comment explaining what was wrong
with the previous version ("this panics on multi-byte input", "loopback is not a trust boundary",
"`surroundContents` throws when the range partially covers a node"). They exist so the mistake is
not reintroduced. Do not tidy them away.

**If you add an external surface, specify it.** New HTTP route, MCP tool, CLI command, config key,
or keyboard shortcut → update its specification document in the same change. Several commands
entered this plan only as examples in unrelated documents and were never specified.

**Do not resolve the open decisions.** `docs/decisions/README.md` lists eight (licence, bundle id,
funding, team size, and four product questions). They are the owner's calls. Flag them; do not pick.

## Facts that are easy to get wrong

These are settled, and earlier drafts had them wrong. Do not regress them.

- **Tauri is the application shell.** There is no separate Swift/AppKit shell, and no second
  `WKWebView` — the reader is a sandboxed `<iframe>` inside Tauri's primary webview.
- **App Sandbox is off** (ADR-0002). Data lives in `~/Library/Application Support/Tome` and
  `~/Library/Caches/Tome`, never `~/.tome`.
- **Sync is an iCloud Drive container with per-device op logs** (ADR-0001), not CloudKit.
- **MCP is stdio.** There is no `mcp.sock`; a Unix socket is not an MCP transport.
- **The local HTTP API requires a bearer token on every request, including loopback**, emits no CORS
  headers by default, and is off by default.
- **Annotations anchor by quote + prefix/suffix**, never bare character offsets.
- **`robots.txt` is obeyed by default** and is not overridable for registry-shipped configurations.
- **There is no telemetry of any kind**, including opt-in. Any metric expressed as a percentage of
  users is therefore unmeasurable — do not add one.
- **`tome pull` fetches documentation. There is no `tome sync`** (bookmark sync is automatic).
- **The plan assumes ~2.5 engineers** (~381 person-days / 30 weeks). If asked about schedule, say
  so; DEC-004 is unresolved.

## Style

Match the surrounding documents: ticket format (`Priority` / `Complexity` / `Dependencies` /
`Blocks`, then Description, Acceptance Criteria, Technical Notes, Success Metrics), sentence-case
headings, tables for anything comparative. Prefer specific and falsifiable over aspirational —
"P95 < 100 ms measured by the P2-018 benchmark" rather than "fast".
