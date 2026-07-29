# Tome — notes for AI assistants

> **Read [`.claude/traps.md`](.claude/traps.md) before changing anything.**
> It records the mistakes already made here and the invariants that a plausible change would break
> silently. Almost none of them fail loudly — that is why they are written down. Add to it when a
> defect turns out to have been non-obvious; delete an entry when it stops being true.
>
> **[`.claude/continuation.md`](.claude/continuation.md) is the current in-flight state** — where
> Stage 2 stands and what the next ticket should do. It is deliberately volatile and is rewritten
> or deleted as tickets land. The two files must not merge: durable knowledge goes in `traps.md`,
> status goes in the continuation note, and an earlier continuation note was deleted for blurring
> that line.

## What this repository is

Tome is a macOS documentation reader: ingest any documentation site, read it offline with good
typography, search across everything, and expose the library to coding agents over MCP.

**Stage 1 is built; Stage 2 has started; Stage 3 onwards is still specification.** Ingestion and
the reader work end to end: `tome pull` fetches a documentation site (robots, rate limit, SSRF,
scope), normalizes, sanitizes, and localizes it into the library, and the app renders it offline in
a sandboxed iframe with a library sidebar, a page outline, and back/forward.

**Search works from the CLI.** `tome pull` indexes as it goes and only rewrites pages whose content
hash changed (S2-2, S2-3), `tome search` queries it, ranking is tuned against a measured eval set
(S2-4), misspelled queries are corrected and told about (S2-5), and `@symbol` searches declared
symbols only (S2-6). The relevance eval **clears its 0.90 recall@3 gate at 0.9082** — 188 queries
of 207, which is met rather than comfortably met. **Search works in the app too** (S2-7): Cmd+K
opens a modal with snippets, scoping, history and full keyboard control. **Stage 2 is not
finished**: S2-8..12 (in-page search, detection corpus, platform detection, scrapers, the P2-018
benchmark) do not exist, and the Stage 2 exit gate wants that benchmark. **Sync, annotations, MCP,
and the local HTTP API do not exist at all.**
When asked whether something works, check rather than assume — much of this repo still describes
intent rather than behaviour.

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
| `scripts/check-contrast.mjs` | Design-token gate: WCAG contrast in both themes, light/dark parity, and that every `var(--token)` resolves. Specified by `docs/plans/15`; it found three real palette defects on its first run |
| `dist/homebrew/` | Cask source of truth, mirrored to `alexnodeland/homebrew-tap` on release |
| `docs/reviews/` | Point-in-time critical reviews of the plan |
| `docs/spikes/` | Results of spikes that have actually run, raw output included. `docs/plans/07` defines the spikes; this directory is where their answers live |
| `docs/decisions/` | Open decisions (DEC-*) and accepted ADRs |

## There is code now

`crates/tome-core` (shared library), `crates/tome-cli` (`tome` binary), `crates/tome-testkit`
(test infrastructure), `src-tauri` (the app), `src/` (Svelte frontend).

The core model (`tome-core/src/model/`, S1-1) is **frozen** — its serde shape is pinned by test and
is a storage/IPC contract. The pipeline is `config → crawl → normalize → sanitize → assets →
relink → store + db`, composed by `pipeline::pull`; `render` turns the stored AST into the HTML the
reader iframe displays. Two contracts thread through it and are easy to break silently:

- **The renderer must quote every attribute and escape every value and text node.** The sanitizer
  deliberately leaves free-text fields unstripped and delegates their safety here; `tome-core/src/html.rs`
  is the one escaping helper and everything routes through it.
- **Nothing rendered may reach the network.** Asset localization rewrites every image to a local
  content-addressed path, the renderer re-checks that at emission, and the frame's CSP allows only
  the `tome:` protocol. `tests/reader_offline.rs` shuts the fixture server down and asserts it.

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
- **The reader is one sandboxed `<iframe>` with `sandbox="allow-scripts"` and nothing else.**
  `allow-same-origin` would hand page content the app's origin and with it the IPC layer. Its CSP
  names the app origin explicitly, because `'self'` in an opaque origin matches nothing.
- **Highlighting is a render concern, not an AST mutation**, and emits CSS classes rather than
  colours, so a theme change needs no re-highlighting.
- **Stored pages carry library paths, not URLs.** A page that names the host it was crawled from
  only makes sense on the machine that crawled it.
- **The plan assumes ~2.5 engineers** (~381 person-days / 30 weeks). If asked about schedule, say
  so; DEC-004 is unresolved.

## Style

Match the surrounding documents: ticket format (`Priority` / `Complexity` / `Dependencies` /
`Blocks`, then Description, Acceptance Criteria, Technical Notes, Success Metrics), sentence-case
headings, tables for anything comparative. Prefer specific and falsifiable over aspirational —
"P95 < 100 ms measured by the P2-018 benchmark" rather than "fast".
