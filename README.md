# Tome

**A personal library for technical documentation.** Point it at any documentation site — ReadTheDocs,
rustdoc, mdBook, man pages, or a plain HTML site — and read it offline in a single, well-typeset
place, with fast search across everything, and an MCP server so your coding agent can read your docs
too.

macOS, Apple Silicon. Local-first. Open source.

---

## Status: ingestion, reading, search and agent access all work. Not yet released.

```bash
tome add https://docs.python.org/3/     # detect the platform, fetch, index
tome search "async iterator"            # 158 µs P95 across the library
tome mcp                                # your coding agent reads your docs
```

| Works | Does not exist |
|---|---|
| Ingestion — robots, rate limit, SSRF, scope, asset localization | Bookmarks and annotations |
| The reader — offline, sandboxed, typeset, with outline and history | Cross-device sync (deferred; ADR-0001 designs it) |
| Search — ⌘K across everything, ⌘F in a page, typo tolerance, symbols | A signed, notarized build |
| Man pages, platform detection, per-platform scrapers | A release you can `brew install` |
| The CLI, the MCP server, the local HTTP API, a Claude Code plugin | |

**Measured, not asserted** — every number here has a harness behind it:

| | | |
|---|---|---|
| Search relevance | **0.9082 recall@3** over 339 documents | target ≥ 0.90 |
| Search latency | **158 µs P95** | budget 100 ms |
| Platform detection | **0.9922** accuracy, zero confident errors | 128 homepages |
| Index size | **224 MB** per 100 000 pages | budget 500 MB |

There is no release yet: no signing, no DMG, no tap. If you want to use it today you build it
yourself. **[Build and run](#building-it-yourself)** is below.

| | |
|---|---|
| **How to use it** | [`site/`](site/README.md) — the user documentation: getting started, CLI reference, agent setup |
| **What it is** | [`docs/PRD.md`](docs/PRD.md) — product requirements, architecture, API and CLI surface |
| **How it is being built** | [`docs/plans/18-implementation-plan.md`](docs/plans/18-implementation-plan.md) — stages, gates, and what each one proved |
| **What was measured** | [`docs/spikes/`](docs/spikes/) — spikes that ran, raw output included |
| **What's wrong with the plan** | [`docs/reviews/2026-07-28-plan-review.md`](docs/reviews/2026-07-28-plan-review.md) — a full critical review |
| **What's undecided** | [`docs/decisions/`](docs/decisions/) — open decisions and ADRs |
| **Ready-made sources** | [`registry/`](registry/README.md) — configurations, never content |

---

## Building it yourself

```bash
npm install
npm run tauri dev                       # the app
cargo build --release -p tome-cli       # the `tome` binary
./scripts/check.sh                      # the gate: everything CI would run
```

> **CI carries no information.** The repository is private and GitHub Actions is blocked at the
> account level, so every run fails in seconds without executing a step. Judge by
> [`scripts/check.sh`](scripts/check.sh), never by a pull request's checks. It runs exactly what
> [`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs — change one, change the other.

---

## Installing (once there is a release)

```bash
brew install --cask alexnodeland/tap/tome
xattr -dr com.apple.quarantine /Applications/Tome.app   # see below
```

Tome is **not signed or notarized** — the Apple Developer Program is deferred
([ADR-0006](docs/decisions/0006-unsigned-distribution.md)). macOS Gatekeeper will refuse to open it
on first launch until the quarantine flag is cleared; the command above works on every supported
version, and the cask's caveats give the click-through alternatives. You only do it once per
install.

That friction is a real cost, and it is the main reason to revisit the decision at v1.0.

`brew install --cask` delivers both the app and the `tome` CLI from the same build, which is what
makes them read the same library.

## Why this exists

Documentation is scattered across browser tabs, terminal `man` pages, and IDE hover cards. Each
source looks different, searches differently, and disappears when you close the tab.

Dash and DevDocs solve much of this well — with large, curated catalogs, offline support, and years
of polish. Tome is not trying to beat them at that. It is aimed at two things they do not do:

1. **Ingest arbitrary documentation sites**, including your company's internal docs and that one
   library whose docs nobody has packaged, and give them the same reading experience as everything
   else.
2. **Expose your documentation library to coding agents** over MCP, so an assistant working on your
   code can read the actual docs for the actual version you have pinned.

Everything else — the typography, the search, the bookmarks — is table stakes that has to be good
enough not to be the reason you go back to browser tabs.

---

## Design commitments

These are constraints, not aspirations. Where the plan violated one, the plan was changed.

- **Local-first.** Your documentation and reading history live on your machine. iCloud sync is
  optional and carries bookmarks only, never content.
- **No telemetry. None.** No analytics, no crash reporting, no phone-home, not even opt-in. The
  cost of this is real and stated plainly: there is no usage data, so product direction comes from
  conversations rather than dashboards.
- **Offline means offline.** Images and assets are fetched at sync time and rewritten to local
  paths; opening a page never touches the network. The reader's content-security policy enforces
  this rather than trusting it.
- **A good citizen on the web.** Tome fetches other people's documentation. It obeys `robots.txt`,
  rate-limits itself, identifies itself honestly, caches for one user, links back to the origin,
  and **never redistributes**. The source registry ships configuration files, never content.
- **Programmable.** A local HTTP API and an MCP server, both authenticated, both off by default.

---

## Fetching other people's documentation

Tome's core loop is downloading documentation someone else wrote. The position, in plain language
(evidence and analysis in [`docs/spikes/010-legal-posture.md`](docs/spikes/010-legal-posture.md)):

- **Tome is a personal cache.** It fetches pages you chose, at your request, to your machine —
  the same pages your browser would fetch, kept so they work offline. Nothing is uploaded,
  shared, or served to anyone else.
- **Tome is a polite client.** `robots.txt` is obeyed (and not overridable for registry-shipped
  configurations), the User-Agent names this project, requests are rate-limited per host,
  `Retry-After` is honoured, and revalidation uses conditional requests.
- **Tome redistributes configurations, never content.** The registry is scraper configs — URLs
  and selectors. No documentation text ships with the app or the registry.
- **Every page keeps its provenance.** The reader links to the origin and shows the upstream
  licence where determinable; exports carry both along.
- **Removal on request.** A documentation owner who objects gets their registry entry removed —
  within seven days, without a debate.

---

## Open decisions

Recorded in [`docs/decisions/`](docs/decisions/). The ones that block progress:

| ID | Decision | Status |
|----|----------|--------|
| DEC-001 | Licence | ✅ Dual **MIT OR Apache-2.0** |
| DEC-002 | Bundle identifier | ✅ `com.alexnodeland.tome` |
| DEC-004 | Capacity and scope | ✅ Solo + agent workflows; sync deferred |
| DEC-003 | Apple Developer Program | ✅ **Deferred.** Ships unsigned via own Homebrew tap |
| DEC-005–008 | Product questions (docset import, `watch` behaviour, note format, export targets) | Open, non-blocking |

**How it is being built:** by a solo maintainer directing Fable + Opus agent workflows. That changes
the binding constraint from *time to write code* to *confidence the code is right*, which is why the
[implementation plan](docs/plans/18-implementation-plan.md) puts every verification artifact before
the thing it verifies. Scope is cut to Stages 0–4; cross-device sync is deferred until v1.0 has
shipped and users ask for it.

---

## Repository layout

```
crates/
├── tome-core/                shared library — app, CLI, and MCP server all use it
│   ├── src/paths.rs          the only place a data path is constructed
│   └── corpus/               golden corpora: real pages and their expected output
├── tome-cli/                 the `tome` binary — CLI, MCP server, HTTP API
└── tome-testkit/             test infrastructure (dev-dependency only)
    ├── src/server.rs         fixture HTTP server — serves doc-site fixtures offline
    ├── src/golden.rs         golden-corpus harness — snapshot, diff, review
    └── fixtures/             hand-authored miniature documentation sites
fuzz/                         fuzz targets; its own workspace, needs nightly
src-tauri/                    the desktop app (Tauri owns the shell)
src/                          Svelte frontend
registry/                     ready-made source configurations, and their live verifier
site/                         the documentation site, deployed to GitHub Pages
dist/claude-plugin/           the Claude Code plugin
docs/
├── PRD.md                    product requirements — the authoritative specification
├── plans/                    phase plans (01-05), dependency map, and supporting documents
│   ├── 00-project-overview.md    start here for the plan
│   ├── 01..05-phase-*.md         90 tickets with acceptance criteria
│   ├── 06-dependency-map.md      graph, critical path, parallelization
│   ├── 07-technical-spikes.md    the questions that must be answered first
│   ├── 08..17-*.md               testing, NFRs, CI/CD, risk, security, observability,
│   │                             versioning, design system, support, recovery
├── decisions/                open decisions and architecture decision records
├── reviews/                  point-in-time reviews of the plan
└── spikes/                   results of spikes that have run, raw output included
```

**`tome-core` shared by the app and the CLI is the load-bearing part.** They are separate
processes that must observe the same library on disk; an integration test runs the real `tome`
binary and asserts it resolves the same paths the app links against. See
[ADR-0002](docs/decisions/0002-no-app-sandbox.md) for why that constraint exists and what it
cost.

Each shared fact has exactly one owning document — see the ownership table in
[`docs/plans/00-project-overview.md`](docs/plans/00-project-overview.md). Please link rather than
restate; the plan previously drifted badly because the same table lived in three places.

---

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). The most useful contributions right now:

- **A registry source.** One YAML file and one index entry, in [`registry/`](registry/README.md).
  Four exist against a v1.0 target of thirty, and this is the gap between "the registry works" and
  "onboarding works". The checks tell you if it is right, and
  `./scripts/verify-registry.sh <id>` proves it against the live site.
- **A documentation site that Tome reads badly.** The golden corpus is how content fidelity gets
  fixed, and every entry in it started as a page that came out wrong.
- **Disagreement.** Especially about the architecture, the scope, or whether the product is worth
  building at all. Open a discussion.
- **Answers to the open decisions above.** DEC-006 now has code waiting on it.

Security issues: see [`SECURITY.md`](SECURITY.md) — please do not open a public issue.

---

## Licence

Dual-licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option — the
Rust ecosystem convention. Contributions are dual-licensed on the same terms.
