# Tome — notes for AI assistants

> **Read [`.claude/traps.md`](.claude/traps.md) before changing anything.**
> It records the mistakes already made here and the invariants that a plausible change would break
> silently. Almost none of them fail loudly — that is why they are written down. Add to it when a
> defect turns out to have been non-obvious; delete an entry when it stops being true.
>
> **[`.claude/continuation.md`](.claude/continuation.md) is the current in-flight state** — what
> has landed and what the next ticket should do. It is deliberately volatile and is rewritten
> or deleted as tickets land. The two files must not merge: durable knowledge goes in `traps.md`,
> status goes in the continuation note, and an earlier continuation note was deleted for blurring
> that line.

## What this repository is

Tome is a macOS documentation reader: ingest any documentation site, read it offline with good
typography, search across everything, and expose the library to coding agents over MCP.

**Stages 1, 2 and 3 are built; Stage 4 onwards is still specification.** Ingestion and the reader
work end to end: `tome pull` fetches a documentation site (robots, rate limit, SSRF, scope),
normalizes, sanitizes, and localizes it into the library, and the app renders it offline in a
sandboxed iframe with a library sidebar, a page outline, and back/forward.

**Search works from the CLI and in the app.** `tome pull` indexes as it goes and only rewrites
pages whose content hash changed; `tome search` queries it; ⌘K opens a search modal with snippets,
scoping and history; ⌘F finds within the open page. Ranking, typo tolerance and symbol search are
all tuned against measured corpora. **Man pages are a first-class source** — `mandoc` renders them
and cross-references link only to pages you have installed. Platform detection classifies a site
from its homepage.

**Stage 2's exit gate is met** (2026-07-29): relevance **0.9082 recall@3** over 339 documents
against a ≥ 0.90 target, and search **P95 158 µs** against a 100 ms budget. The relevance half is
met by one query of 207 — read it as *met*, not as comfortable.

**Agent access works** (Stage 3, 2026-07-30). The CLI is complete — `add` (with platform
detection), `pull`, `list`, `search`, `remove`, `status`, `serve`, `mcp`, all with `--json`.
**`tome mcp` is a real MCP stdio server**: Claude Code connects to it and answers questions from
locally indexed pages, verified against the real client. **`tome serve` is the local HTTP API**,
off by default, bearer token on every route including loopback. There is a
[Claude Code plugin](packaging/claude-plugin/) and a [source registry](registry/README.md) with a
live verification job.

**Sync and annotations do not exist at all.** When asked whether something works, check rather
than assume — parts of this repo still describe intent rather than behaviour.

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
| `registry/` | The source registry: ready-made configurations, its own README, and `scripts/verify-registry.sh` |
| `packaging/` | Authored distribution artifacts. **Not `dist/`** — that is Vite's `outDir`, and Vite empties it on every build, which is how the plugin and the cask were deleted for two stages without anyone noticing |
| `packaging/homebrew/Casks/tome.rb` | Cask source of truth, mirrored to `alexnodeland/homebrew-tap` on release. Linted by `scripts/check-cask.sh` |
| `packaging/claude-plugin/` | The Claude Code plugin: manifest, commands, bundled MCP config |
| `scripts/verify-bundle.sh` | The only check that looks at the artifact a user installs: that `Tome.app` ships the CLI, from the same build, with the same version, resolving the same library, and that the cask's zap list covers what it writes |
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
- **The `tome` CLI ships inside the app bundle**, at `Tome.app/Contents/MacOS/tome`, as a Tauri
  `externalBin` sidecar — one build delivers both, which is what makes them resolve the same
  library. There is no second artifact, and no separate `brew install tome` formula.
- **Tome ships unsigned** (ADR-0006), so Gatekeeper blocks first launch and the cask's caveats
  carry `xattr -dr com.apple.quarantine`. Not a bug to fix; a decision to revisit at v1.0.
- **App Sandbox is off** (ADR-0002). Data lives in `~/Library/Application Support/Tome` and
  `~/Library/Caches/Tome`, never `~/.tome`.
- **Sync is an iCloud Drive container with per-device op logs** (ADR-0001), not CloudKit.
- **MCP is stdio.** There is no `mcp.sock`; a Unix socket is not an MCP transport. The server
  speaks the **legacy `2025-11-25` handshake** deliberately — the shipping client does, and has no
  fall-forward against a modern-only server. `tome mcp --http` is still unimplemented, by choice.
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
