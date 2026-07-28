# Tome — Plan Review

**Date:** 2026-07-28
**Scope:** the PRD and all 18 planning documents (~13,600 lines). No code exists.
**Method:** full read of every document, cross-checking claims between them and against the
platforms and protocols they depend on.

---

## Verdict

This is a genuinely strong planning set. It is more thorough than most funded products get: 87
tickets with acceptance criteria and dependency graphs, a risk register with a scoring framework,
a testing strategy, NFRs, CI/CD, security, observability, versioning, design system, support,
and disaster recovery. Somebody thought hard about this.

It also has three categories of problem, and they are not equally important:

1. **A few decisions that are wrong and would waste months.** The stack contradicts itself, one
   security posture is incompatible with the file layout, the API is open to any web page, MCP uses
   a transport that does not exist, and annotations are designed to silently corrupt.
2. **A pervasive consistency failure.** The same fact is stated in three documents with three
   different values — the critical path, the memory targets, the data directory, the keyboard
   shortcuts, the API base path, the entitlements. This is what happens when documents are
   generated in parallel rather than derived from each other, and it compounds: every future edit
   has to guess which copy is authoritative.
3. **A planning-fiction problem.** The plan describes a 2.5-person team, weekly risk reviews,
   quarterly recovery drills, and a 90% coverage gate — for what the risk register elsewhere calls
   a single-maintainer project. Process nobody performs is worse than lighter process that happens,
   because it makes the plan look managed when it is not.

Underneath all of it is one structural issue worth stating on its own: **the plan is written as
though the spikes had already passed.** SPIKE-001 asks whether the shell architecture is feasible;
23 Phase-1 tickets assume it is. SPIKE-008 asks what MCP requires; the MCP server was specified
against an invented transport. Spikes that do not gate anything are documentation, not risk
reduction.

**The single most consequential finding is not technical.** Summing the plan's own estimates gives
~381 person-days against a 30-week calendar — about 2.5 full-time engineers. The dependency map
allocates work to "Developer 1/2/3"; the risk register describes one maintainer. Until that is
resolved, every date in the plan is fiction, and the right response is to **cut scope, not extend
the calendar**.

---

## What is genuinely good

Worth saying explicitly, because the rest of this document is criticism:

- **The product thesis is sharp.** "Point it at any docs site" plus "expose your docs to coding
  agents" is a real gap that Dash and DevDocs do not fill.
- **Local-first with no telemetry is a coherent, defensible position** that is followed through
  almost everywhere.
- **Ticket quality is high.** Acceptance criteria are concrete and mostly testable. Dependencies
  are real dependencies, not decoration.
- **The risk framework** (impact × probability, triggers, mitigation, contingency) is properly
  built. Its problem is what is missing from it, not how it works.
- **The spike list is the right instinct** — nine well-chosen questions with time budgets and
  fallbacks. The failure is procedural: none gates anything.
- **The rollback and recovery document exists at all**, which is rare, and its instinct to test
  recovery procedures is correct.

---

## Findings

Severity is about consequence if unaddressed, not effort to fix.

### Critical

#### C1 — The architecture describes three overlapping shells

`Native Shell: Swift + AppKit`, `UI: Svelte`, `Doc Rendering: WKWebView`, and `IPC: Tauri` were
listed as four peer layers. They are not peers. **Tauri *is* the native shell on macOS** — it owns
the process, the window, and a `WKWebView`. So the table describes a Swift shell that does not need
to exist, and "Svelte UI" and "WKWebView rendering" are the same web context unless a second
webview is deliberately created — which nothing said.

This matters because it propagates: SPIKE-001 exists to de-risk a Swift↔Rust↔JS boundary that only
two Phase-5 features actually need.

**Applied:** stack table rewritten around Tauri as the shell; the reader is a sandboxed `<iframe>`
inside the primary webview (which also isolates untrusted HTML from the IPC bridge); Swift demoted
to an optional Phase-5 plugin for `NSStatusItem` and the global hotkey, with pure-Tauri fallbacks.
Architecture diagram redrawn.

#### C2 — App Sandbox is incompatible with the design, and the plan required both

The NFR document required "App Sandbox enabled". The Phase-5 entitlements file never requested it.
Every path in the plan was `~/.tome`, which **a sandboxed app cannot write** — the path is
redirected into its container.

The consequence is the one that matters: the CLI installed by Homebrew is not sandboxed. The app
would be. **They would read different libraries.** The CLI, the MCP server, and the GUI sharing one
library is the entire integration story, and sandboxing silently breaks it.

**Applied:** Developer ID + notarization + hardened runtime, App Sandbox **off** (it is only
mandatory for the Mac App Store, and distribution is DMG + Homebrew). The compensating controls are
stated rather than glossed. Data moved to `~/Library/Application Support/Tome` (state) and
`~/Library/Caches/Tome` (re-fetchable), with `$TOME_HOME` as an override. NFR, security doc, Phase 5
entitlements, rollback paths, and the Homebrew zap list all reconciled.

#### C3 — The local HTTP API was open to every web page the user visits

Three decisions combined into a serious hole:

- `CorsLayer::permissive()` in P4-009
- auth middleware that returned early for `addr.ip().is_loopback()`
- `POST /api/sources`, which makes Tome fetch an arbitrary URL

**Loopback is not a trust boundary on a desktop.** Any page the user has open can `fetch()`
`http://localhost:7431`; with permissive CORS it can read the response too. So any website could
enumerate the user's library, read their bookmarks and reading history, and use Tome as an **SSRF
proxy into their private network** — reading the result back through `GET /pages`. The auth bypass
exempted precisely the attacker that matters.

The security document meanwhile specified "No CORS in default mode". The code would have won.

**Applied:** server off by default; bearer token required on **every** request including loopback;
no CORS headers by default with an explicit allowlist opt-in (`*` rejected); `Host`/`Origin`
validation to defeat DNS rebinding; constant-time token comparison; SSRF validation before any
fetch; `/status` the only unauthenticated route. P4-012 raised from Medium to Critical.

#### C4 — The SSRF filter did not work, and part of it was unreachable

`validate_source_url` matched hostname *strings*. It missed `172.16/12`, `169.254/16` (cloud
metadata at `169.254.169.254`), all of IPv6 (`[::1]`, `fc00::/7`), `0.0.0.0`, and alternate
encodings (`2130706433`, `0x7f.1`, `127.1`). It did nothing about DNS rebinding. And its `file://`
check was **dead code** — the scheme check above it already returned `Err`.

**Applied:** resolve first, then judge every resolved address; full IPv4 and IPv6 predicate
including IPv4-mapped unwrapping; pin the connection to the validated address (or the check-then-
connect race reopens the hole); re-validate on every redirect hop. Test vector list added to the
security checklist and to P1-008's success metrics.

#### C5 — MCP used a transport that does not exist

The server was specified on a Unix socket at `~/.tome/mcp.sock`. **MCP defines stdio and Streamable
HTTP.** No client — including Claude Code, the headline integration — can connect to a bare socket.
The flagship feature of Phase 4 could not have worked with its flagship client. Compounding it, the
supported-versions list contained `"2024-09-01"`, which is not a real protocol version, and pinned
a 2026 product to 2024 revisions.

**Applied:** stdio default with `tome mcp`, optional Streamable HTTP behind the same auth; the
consequences of stdio spelled out (spawned per client, no exclusive index lock, **nothing on stdout
but protocol messages**, clean exit on EOF — the original loop had no EOF handling and would have
orphaned processes); version list made a maintained build-time constant driven by SPIKE-008;
SPIKE-008 promoted P2 → P0. Write-capable tools disabled by default, because documentation Tome
ingests is untrusted text that agents will read.

#### C6 — Annotations were designed to silently corrupt

Highlights were stored as raw character offsets. Documentation is re-fetched on a schedule, so any
upstream edit above a highlight shifts every offset below it — and the highlight reappears attached
to *different text*. This is worse than deleting it, because the user is not told; they return to a
highlight marking unrelated words and stop trusting every other annotation.

Probability is not low. Documentation changes constantly. This is the normal case.

**Applied:** W3C Web Annotation anchoring — quote + prefix/suffix as the anchor, offsets as a hint
only; re-anchor pass on content-hash change; explicit `exact`/`approximate`/`orphaned` states;
orphaned annotations keep their text and note and surface in a "needs attention" view, never
deleted. Property tests specified. Added as RISK-014 (16, Critical). The sample's
`range.surroundContents()` was also fixed — it throws on any selection partially covering a node,
i.e. most real highlights.

#### C7 — The bookmark uniqueness key guaranteed duplicates

`UNIQUE(source_id, page_path, device_id)`. Bookmarking the same page on a laptop and a desktop
produces **two rows**, and no merge logic downstream can collapse them — by definition they are
distinct records. Sync would faithfully replicate both. A sync key must identify the thing, not the
writer.

**Applied:** `UNIQUE(source_id, page_path)`; `device_id` → `last_writer`, metadata only. Also split
annotations out of bookmarks (you can highlight a page you never bookmarked, and deleting a bookmark
was cascade-deleting annotations), replaced `sync_status` with a sync-state table, and replaced
`collection_id: Option<Uuid>` with a list — the features list promised multi-collection membership
that a nullable id cannot express.

#### C8 — There was no legal risk at all

The risk register had ten risks and none of them was legal. For a product whose core loop is
automated retrieval of third-party copyrighted content, that is a striking omission — and
`robots.txt` compliance was specified as **"optional, configurable"**, which is the most likely way
the risk actually materializes.

**Applied:** RISK-011 (16, Critical), separating the three distinct exposures (crawl behaviour,
local caching, redistribution) because conflating them is how this gets handled badly. `robots.txt`
obedience is now a non-overridable default for bundled configs. Honest User-Agent, conditional
requests, `Retry-After`, rate caps. Attribution enforced in the reader. The **registry ships
configurations, never content** — a structural mitigation rather than a policy one. SPIKE-010 added
to establish the position before it shapes the product.

---

### High

#### H1 — The estimates do not survive arithmetic

| Phase | Effort | Calendar | Implied FTE |
|-------|--------|----------|-------------|
| P1 | 94.5 pd | 8 wk | 2.4 |
| P2 | 79.5 pd | 6 wk | 2.7 |
| P3 + P4 (parallel) | 139.5 pd | 6 wk | 4.7 |
| P5 | 54.5 pd | 4 wk | 2.7 |
| **Total** | **~368 pd** | **30 wk** | **~2.5** |

Plus 16 days of spikes, and nothing for review, design iteration, or dependency breakage. The
critical path is ~88 working days (~18 weeks), so **sequencing is not the constraint — capacity
is.** Solo at full time this is ~77 weeks; at side-project pace, four-plus years, by which point
several scrapers will have rotted.

**Applied:** effort table published in the PRD and overview; every phase header annotated; DEC-004
raised as a phase gate; RISK-012 added. Recommendation recorded: **cut to P1 + P2 + the MCP half of
P4** (~55% of effort, both differentiated features retained), and drop Phase 3 — 68 person-days,
the highest-scoring risk, and the least painful loss since bookmarks still work locally. A solo
sequencing plan was added to the dependency map: get one real docs site readable end-to-end in ~3
weeks, before any UI polish, to validate the riskiest assumption early.

#### H2 — The critical path was stated three ways, all different

The overview's chain, the dependency map's "Primary Critical Path", and the dependency map's
23-ticket table named different tickets. The overview's chain included P1-015, P4-005, P5-007 and
P5-014, none of which appear in the 23-row table; the map's chain omitted P1-004, P1-008, P1-012
and P1-013, all of which do.

**Applied:** derived one path from the dependency graph weighted by effort — 15 tickets, ~88 days —
with the release-gate chain (`P5-010→011→012`) called out separately, and the near-critical MCP
chain flagged as 3.5 days from becoming critical. All three locations now agree, with the overview
named as owner.

#### H3 — Three incompatible sync mechanisms

CloudKit `CKRecord`s in a custom zone (Phase 3), a symlinked `~/.tome/icloud/*.json` (PRD), and
`~/Library/Mobile Documents/.../*.json` (rollback). Only one can be built. Additionally, CloudKit is
Swift/Obj-C only while the core is Rust, and the CLI — which runs outside the app — must sync too.

**Applied:** one mechanism, the iCloud Drive ubiquity container. This is not a new idea: it is the
contingency **the risk register already recorded** for RISK-002, promoted to primary because the
constraints favour it. Design specified concretely: per-device append-only op logs (so two devices
never write the same file and iCloud's conflict machinery is never invoked), Lamport-ordered
convergence, add-wins sets, tombstones, idempotent replay. P3-010/011/012 rewritten; P3-015 shrank
from M to S because the op log *is* the offline queue — a second queue on top of an append-only log
is a second source of truth that can disagree with the first.

#### H4 — Most success metrics were unmeasurable by construction

"80% of users complete onboarding", "sync reliability > 99.5%", "crash-free sessions > 99.9%",
"cache hit rate > 80%", "< 5 user-reported critical bugs in the first month", "relevant result in
top 3 for 90% of queries". Tome collects **no telemetry** — there is no mechanism by which any
percentage-of-users metric could ever be observed. Several also lack a denominator.

The last one is the interesting case: it is a real quality bar with no eval set behind it.

**Applied:** metrics split into lab metrics (CI, against owned corpora), public signals (GitHub,
Homebrew), and explicitly-not-measured. Unmeasurable targets removed or restated. **Two tickets
added — P2-019 (relevance eval set + harness) and P2-020 (detection corpus)** — because tuning
ranking without an eval set is guesswork and search quality regresses invisibly. Onboarding
completion replaced with moderated testing on five users, which measures the same thing and is
possible.

#### H5 — The offline guarantee was false

"Works offline" is a headline claim. The ingestion pipeline fetched **HTML only** — no images, no
diagrams, no SVGs. Every page with a figure would have broken on a plane, and the reader would have
issued live requests to third-party hosts on every page view, leaking reading activity and
contradicting the document's own CSP.

**Applied:** P1-023 added (L, Critical) — asset collection, fetch under the same etiquette rules,
content-addressed storage with dedup, sanitization of SVG, size caps, reference rewriting,
placeholder on failure (never a live remote reference), GC on re-sync. CSP `img-src` tightened to
`'self' data:` so the browser enforces the guarantee. Phase 1 exit criteria now include rendering
with networking disabled — the original criteria never checked the product's headline claim.

#### H6 — The E2E test strategy could not run

Playwright specs were written to drive the app, and CI ran `playwright install webkit`. **Tauri
automation goes through `tauri-driver`, which supports Linux and Windows only** — macOS `WKWebView`
has no WebDriver implementation. The CI job installed *Playwright's own* WebKit build, so every
"E2E" test would have exercised a stock browser, not Tome, and **reported green**. On a macOS-only
product, there was no platform on which this tier could ever have run.

Jest had the same shape of problem: `preset: 'ts-jest'` cannot compile `.svelte`.

**Applied:** three tiers that do run — Vitest + Testing Library against real components with a
stubbed IPC seam (the workhorse), Rust integration tests for the full backend pipeline, and a thin
XCUITest smoke suite for what genuinely needs a real bundle. Jest → Vitest. Wall-clock assertions
removed from UI tests (they flake and teach people to ignore red builds); latency lives in
`criterion` benchmarks. Property and fuzz testing added for the parser, sanitizer, sync convergence,
and re-anchoring — the components where example-based tests miss the bugs that matter.

#### H7 — CI claimed mitigations it did not implement

RISK-008 and the security document both list `cargo audit`, `npm audit`, and Dependabot as active
CI controls. **The workflows contained none of them.** A mitigation recorded in a risk register but
absent from CI is worse than an acknowledged gap, because the risk reads as handled.

Also: no `permissions:` blocks (so the default write-scoped token was available to every third-party
action in a workflow holding signing secrets), no action pinning, and every job on `macos-14` at 10×
the Linux billing rate for jobs needing no macOS.

**Applied:** `audit` job added (cargo audit, cargo deny, npm audit, gitleaks) as a required check;
least-privilege `permissions` blocks; SHA-pinning rule with `github-actions` added to Dependabot;
lint and JS tests moved to Linux; certificate cleanup and keychain teardown; the duplicate
`cargo update` cron superseded by Dependabot with the conflict explained.

#### H8 — The release procedure could not execute

It ended in `git push origin main --tags` — a direct push to a branch this same document protects
with required reviews, required status checks, and linear history. GitHub rejects it. The hotfix
procedure had the same defect.

**Applied:** version bumps go through a PR; tags are signed and applied to the merge commit on
main; the release workflow verifies the tag is an ancestor of main and fails otherwise. Clean-machine
`spctl` verification added before publishing — a build that is signed but not stapled passes on the
build machine and fails for every user, which is the most common way a macOS release goes wrong.

#### H9 — Homebrew distribution assumed something a third party controls

P5-012 was **Critical** priority and depended on submission to `homebrew-cask`, which has notability
requirements a brand-new project does not meet. Critical priority on an action someone else will
decline is a scheduling trap.

**Applied:** own tap on day one, homebrew-cask as a tracked post-launch follow-up; priority lowered
to High; zap list corrected to the actual paths (the original listed three roots no version ever
used simultaneously).

#### H10 — `com.example.tome` was threaded through the release pipeline

A placeholder bundle identifier appears in notarization, the Keychain service name, the iCloud
container, and the Homebrew zap list. Any of these being wrong at release breaks signing, or
silently orphans user data.

**Applied:** raised as DEC-002, blocking P5-010; parameterized in the scripts; added to the
pre-launch checklist.

---

### Medium — code and specification defects

These were found by reading the samples as if they had to compile and run. They are listed
compactly; each is fixed in place with a comment explaining why.

| # | Where | Defect |
|---|-------|--------|
| M1 | security | `&query[..query.len().min(1000)]` **panics** on multi-byte input — and search text comes from the user, the API, and MCP clients |
| M2 | security | `validate_page_path` had three bugs: literal `~` in `PathBuf` (never expanded), base never canonicalized so the containment check could never pass, and `canonicalize()` fails for paths that do not exist yet |
| M3 | several | Literal `~` passed to `tracing_appender`, `notify::watch`, and the MCP socket path — none expands tilde; each would create a directory named `~` |
| M4 | rollback | `PRAGMA foreign_key_check` returns **rows, not a scalar** — `query_scalar().fetch_one()` errors on a *clean* database, so the integrity check reported every healthy database as broken |
| M5 | versioning | Down migration drops a column that `idx_bookmarks_sync` indexes — SQLite refuses; discovered only during an actual rollback, i.e. during an incident |
| M6 | P4-010 | `total: results.len()` after moving `results` (won't compile), `total` semantically wrong under a limit, and `query_time_ms: 0 // TODO` shipped in the published API contract |
| M7 | P4-018 | `.elapsed()` on `Option<DateTime>`; `split_once(':').unwrap()` panics on user-edited YAML; 60-second registry polling ≈ 43k requests/day for 30 watched crates, contradicting "no background network activity without user action" |
| M8 | P3-007 | `range.surroundContents()` throws whenever a selection partially covers a node — i.e. most real highlights |
| M9 | P3-011 | Swift `as!` and `!` on records arriving from another device — one malformed record becomes a launch crash loop; `_ => unimplemented!()` reachable from remote data |
| M10 | P2-009 | "Edit distance 2–3 for longer words" — Tantivy's automaton maxes at 2; `field` undefined in the sample; `len()` counts bytes not chars |
| M11 | observability | `Backtrace::capture()` returns `Disabled` unless `RUST_BACKTRACE` is set, which it is not for a double-clicked app — crash reports would ship **empty**; `fs::write` to a fixed path overwrites the first (most informative) crash; `dirs::data_dir()?` is `Option` in a `Result` fn; `_guard` dropped immediately so the non-blocking log writer never flushes |
| M12 | observability | `NSUserNotification` was removed from modern SDKs — would not compile |
| M13 | P5-010 | `codesign --deep` is documented by Apple as unsuitable for signing; entitlements included `allow-unsigned-executable-memory` and `disable-library-validation`, both materially weakening the hardened runtime and neither needed |
| M14 | design system | `--text-base: 17px` declared but never applied, so `rem` resolved against 16px — **every commented pixel value was wrong** and the reader would render a point small |
| M15 | design system | Two token pairs **failed the project's own 4.5:1 contrast rule**: `#8E8E93` on `#FAFAFA` (3.1:1) and `#6E6E73` on `#1C1C1E` (3.4:1) |
| M16 | design system | Dark `--color-code-bg` identical to `--color-bg-secondary` (code blocks invisible on panels); light `#F5F5F7` on `#FAFAFA` effectively invisible |
| M17 | design system | Only `prefers-color-scheme`, but P5-007 offers explicit light/dark/system — CSS could not honour the setting |
| M18 | design system | `role="button"` with no keydown handler; modal with no focus trap, Escape, `aria-modal`, or focus restoration; `Icon` always `aria-hidden` so icon-only buttons had no accessible name; `×` as a close label (read as "multiplication sign") |
| M19 | security | Sanitizer allowlist stripped **`id`** — silently disabling every heading anchor, the TOC sidebar, and every `#fragment` cross-reference — plus `img`, `br`, `hr`, `sup`/`sub`, `dl`/`dt`/`dd`, and `alt` |
| M20 | security | CSP allowed `img-src https:`, permitting the tracking pixels the same document lists as a threat two paragraphs above |
| M21 | security | Silent `http:`→`https:` upgrade with `.ok()` discarding the failure — not a control, and it hides an insecure configuration from the user |
| M22 | NFR/spikes | Four different idle-memory targets (200 MB / 500 MB / 100 MB / 50 MB) and a 10× disagreement on index size (50 MB vs 5 MB per 1k pages) |
| M23 | versioning | `/api/v1/` here but unversioned `/api/` in four other documents; `schema_version` introduced here but absent from the authoritative schema and the parser; `GET` with a JSON request body |
| M24 | versioning | MCP `"2024-09-01"` is not a real protocol version |
| M25 | P1-022 | Example config used `strategy: weekly`, **invalid against the schema** (`weekly` is a `schedule` value) — the shipped example was wrong in the way users will most commonly get it wrong |
| M26 | testing | Broken link to `09-cicd-devops.md` (the file is `10-`); global 90% coverage gate contradicting the per-module table that permits 80–85% |
| M27 | CLI | `tome sync`, `tome import`, `tome debug …`, `tome rebuild-index` used across three documents, none in the CLI specification; error messages named commands that do not exist |
| M28 | support | "No SLAs" alongside P0 "Immediate" / P1 "< 4 hours" in the recovery document |
| M29 | several | Dependency cadence stated three ways (weekly / weekly-via-cron / monthly), with two mechanisms that would produce competing PRs |
| M30 | P5-004 | "Error telemetry (opt-in)" against "zero data collection or phone-home" and "no external crash reporting" |
| M31 | PRD | Competitive table factually wrong: DevDocs *does* work offline, Dash is a one-time purchase, Zeal supports more platforms than Tome will |
| M32 | shortcuts | `Cmd+H` (Hide Application) used for highlight; `Cmd+P` (Print) for go-to-page; single-letter reading keys unscoped, so typing in a filter box would scroll the document; four drifting shortcut tables |

---

### Low — but worth doing

- **Repo hygiene.** `README` had no extension, so GitHub rendered it as plain text. It was also a
  PRD, not a README. No `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, or
  `.gitignore` — several of which other documents reference as existing.
- **Planning docs lived in `.claude/plans/`.** A tool-specific dotfile directory is invisible in the
  GitHub UI and excluded by most documentation tooling — the wrong home for the primary
  specification. The CI document's own CODEOWNERS table already pointed at `/docs/`.
- **Stale metadata.** PRD dated January 2026, status "Draft"; overview status "Planning Complete";
  changelog ending 2026-01-25. Six months on with nothing built, "Planning Complete" is misleading
  in a way that matters — it reads as ready-to-implement.
- **No ADR location** despite spikes listing "Architecture decision record" as an output.
- **Spike ownership.** Every spike is unowned, undated, and "Not Started". SPIKE-009 was marked P2
  ("before the relevant phase") but due "During P1", contradicting its own priority definition.
- **Process weight.** Weekly critical-risk reviews, bi-weekly high-risk reviews, monthly all-risk
  reviews "with all stakeholders", quarterly recovery drills, quarterly performance checks — for a
  project with one maintainer and no stakeholders.

---

## Things I deliberately did not decide

These are the owner's calls. They are recorded in [`docs/decisions/`](../decisions/README.md) rather
than resolved here.

| ID | Decision | Why it is yours |
|----|----------|-----------------|
| DEC-001 | Licence (MIT vs Apache-2.0) | A licence is a commitment, not a default. The plan asserted MIT in marketing copy and "TBD" in the NFRs; **nothing should be published until this is settled.** |
| DEC-002 | Bundle identifier and domain | Depends on a domain you own |
| DEC-003 | Funding | The Apple Developer Program at $99/yr is mandatory for notarization *and* iCloud. Not a task — a standing cost |
| DEC-004 | Team size | **The most consequential open question in the project.** Everything downstream changes |
| DEC-005 | Docset import priority | Importing Dash docsets may be the cheapest possible answer to cold start; worth reconsidering its v1.2 slot |
| DEC-006–008 | `watch` behaviour, note format, export targets | Product taste |

Two architectural forks I *did* resolve, because leaving three contradictory answers in place was
worse than picking, and in both cases the plan's own documents pointed at the answer:

- **Sync mechanism → iCloud Drive container.** This is the contingency RISK-002 already recorded,
  and the Rust core plus out-of-process CLI make it the better primary. Reversible: P3-010 states
  the tradeoff and CloudKit remains the documented fallback.
- **App Sandbox → off.** Forced by the CLI/app shared-library requirement and by DMG + Homebrew
  distribution. Reversible only by giving up the shared library, which would be a product change.

---

## Recommended next five actions

1. **Answer DEC-004.** Everything else is downstream of it. If solo, re-cut to P1 + P2 + MCP now,
   before any code is written against a plan that assumes 2.5 engineers.
2. **Run SPIKE-001, 002, 003, 008 and 010.** Three days each at most. They gate 90 tickets.
3. **Choose a licence and commit `LICENSE`.** One decision, five minutes, currently blocking any
   honest public commit.
4. **Build one vertical slice before anything else** — one real documentation site fetched,
   normalized, sanitized, rendered, offline. Three weeks, and it validates the riskiest assumption
   in the product (that normalization across arbitrary sites is tractable) far earlier than the
   current Phase 1 ordering does.
5. **Build the relevance eval set early (P2-019).** Not because search work is imminent, but because
   without it there is no way to know whether search is getting better or worse — and that is the
   feature everything else exists to serve.

---

## What changed in this sweep

Applied across the PRD and all 18 planning documents:

- **Structure:** `README` → `docs/PRD.md`; `.claude/plans/` → `docs/plans/`; new `README.md`,
  `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `CLAUDE.md`, `.gitignore`,
  `docs/decisions/`, `.github/` templates and CODEOWNERS.
- **All eight Critical and ten High findings** applied as described above.
- **All 32 Medium defects** fixed in place, each with a comment explaining the failure mode — the
  comments are deliberate, so the same mistake is not reintroduced.
- **Single-ownership table** added to the overview, naming which document owns each shared fact.
  Duplicated tables replaced with links. This is the change most likely to prevent the plan drifting
  apart again.
- **Three tickets added:** P1-023 (asset localization), P2-019 (relevance eval), P2-020 (detection
  corpus). Ticket count 87 → 90.
- **Two spikes added:** SPIKE-010 (legal posture), SPIKE-011 (sanitizer vs real docs).
- **Four risks added:** RISK-011 legal, RISK-012 capacity, RISK-013 cold start, RISK-014 annotation
  drift.
- **Eight decisions recorded** as explicitly open rather than silently assumed.

Nothing in this sweep changes the product's ambition. It removes the parts of the plan that were not
true.
