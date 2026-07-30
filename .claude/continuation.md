# Continuation — Stage 3 is complete; Stage 4 is next

**Written:** 2026-07-30, after S3-8 landed. **Rewrite or delete this file when Stage 4 starts.**

In-flight state only: what is done, what is decided, what to do next. It deliberately carries **no**
durable knowledge — mistakes and invariants live in [`.claude/traps.md`](traps.md), which does not go
stale. Do not let this file grow a "traps" section; an earlier one was deleted for exactly that.

---

## Stage 3 is done

| | Ticket | Landed as |
|---|---|---|
| SPIKE-008 ✅ | MCP protocol, against the real client | [`docs/spikes/008-mcp-protocol.md`](../docs/spikes/008-mcp-protocol.md) |
| S3-1 ✅ | CLI: `add`, `remove`, full `--json` | `add.rs`, `remove.rs`, `tests/cli_commands.rs` |
| S3-2 ✅ | MCP stdio server | `mcp.rs` |
| S3-3 ✅ | MCP tools | `mcp_tools.rs` — five, all read-only |
| S3-4 ✅ | Truncation + `section` | in `mcp_tools.rs` |
| S3-5 ✅ | HTTP API + auth | `serve.rs`, `token.rs`, `tests/api_http.rs` |
| S3-6 ✅ | Claude Code plugin | [`packaging/claude-plugin/`](../packaging/claude-plugin/) |
| S3-7 ✅ | Sync strategies | `tome-core/src/sync.rs`, `tome pull --all --due` |
| S3-8 ✅ | Source registry + verification | [`registry/`](../registry/README.md) |

**The exit gate is met.** Claude Code 2.1.220 connected over stdio to a library pulled from the
fixture site, called `tome_search` and `tome_get_page`, and answered by quoting the page.

## What Stage 3 did not build

Not omissions to discover later — each is marked `[~]` in its ticket with the reason:

- **`tome mcp --http`** (Streamable HTTP). No client Tome targets needs it — Claude Code spawns
  the process — and an HTTP MCP endpoint has exactly the browser-reachability problem `serve`
  spends its whole middleware stack on. It lands when a client that cannot spawn processes does.
- **`tome_bookmark` and the `/api/v1/bookmarks` routes.** There is no bookmark model until Phase
  3. Absent, not stubbed.
- **`watch` sync.** **DEC-006 is open and is the owner's call.** `sync::due` returns
  `WatchUndecided`, which does not fetch, and a test fails if someone implements it instead of
  deciding it.
- **A background sync loop.** `--due` is invoked, not scheduled. The daemon (and the cancellation
  and concurrency cap it would need) belongs with the app's launch path.
- **A `language` filter on `tome_lookup_symbol`.** Symbols come from headings and carry no
  language; a parameter that is accepted and ignored is worse than its absence.
- **Timeouts on tool calls.** Every tool call is a synchronous local read with no network. The
  hang a timeout guards against has no source here.

## What is weakest in what *was* built

- **The registry has four sources against a v1.0 target of thirty.** The machinery is done and
  measured; the content is a weekend per ten. This is the gap between "the registry works" and
  "onboarding works".
- **The rate limiter is one global fixed window**, not per-token — because there is one token.
  It becomes wrong the moment a second credential exists.
- **`pull` is sequential**, which caps concurrency at one by accident rather than by design. A
  background scheduler makes that a real decision.
- **The API's `total_hits` saturates at 1000** and says so via `total_capped`. Honest, but a
  library larger than that reports a ceiling rather than a count.
- **Nothing has been run against a large real library.** Every end-to-end test uses the 4-page
  fixture site; the registry verification caps at 25 pages. Behaviour at 100k pages is SPIKE-003's
  measurement of Tantivy, not of these surfaces.

---

## Stage 4

Read [`docs/plans/18-implementation-plan.md`](../docs/plans/18-implementation-plan.md) for the
stage's own entry gate, ticket list and ordering — it is the execution plan and it owns that.

**S4-9 is the one that matters and is easy to get wrong**: the cask symlinks
`Tome.app/Contents/MacOS/tome` onto `PATH`, so a release must deliver the app *and* the CLI from
the same build. Everything in Stage 3 assumes they resolve the same library.

Three things from this stage that should carry into the next:

**Ask the tool, not the specification.** SPIKE-008 found that the current MCP revision is not what
the shipping client speaks. `claude plugin validate` found that P4-017's manifest format never
existed. The registry verifier found that `nodejs.org/docs/` is `Disallow`ed. All three were
written confidently and all three were wrong; each took one command to disprove.

**Verify the security property in the medium where it fails.** The CORS design reads as correct in
Rust. It was worth driving a real browser at it from a hostile origin — that is the check that
distinguishes "no CORS headers" from "cannot have effects", and the plan says as much.

**A check that edits its subject checks nothing.** The registry verifier's first version `sed`-ed
the config it was verifying, which both tested a file no user runs and silently failed to cap
three of the four source types.

## How to run the gates

```bash
./scripts/check.sh                      # everything, including the app bundle
./scripts/check.sh --fast               # everything except the bundle

cargo test -p tome-core --test relevance -- --nocapture          # search quality
cargo test -p tome-core --test detection -- --nocapture          # platform detection
cargo test -p tome-core --test registry                          # registry, offline
cargo test -p tome-core --test search_bench --release -- --nocapture   # latency

./scripts/verify-registry.sh            # registry, LIVE — deliberately not in check.sh
TOME_VERIFY_UPDATE=1 ./scripts/verify-registry.sh   # …and write back `verified:` dates
```

The MCP handshake against real Claude Code cannot run in the suite (it needs a logged-in client).
The command shape is in the SPIKE-008 write-up; re-run it by hand when the protocol handler
changes.

Update modes — `TOME_UPDATE_GOLDEN`, `TOME_UPDATE_BASELINE`,
`TOME_UPDATE_DETECTION_BASELINE` — all **fail the run they change anything in**, on purpose. The
passing run is the one after the diff has been read.

## Still open — do not decide alone

- **`two-face`** for TypeScript/TOML syntax highlighting. A licence decision, not a technical one.
- **DEC-005** docset import priority · **DEC-006** `watch` fetch vs notify · **DEC-007** note
  format · **DEC-008** export targets. DEC-006 now has code waiting on it (`sync::due` returns
  `WatchUndecided`); the rest remain non-blocking.
- **PR #10 (TypeScript 7)** — left open deliberately as a reminder. `npm ci` fails outright:
  `svelte-check@4.7.4` peers on `typescript@^5 || ^6`. Re-check when svelte-check and
  typescript-eslint support TS 7.
- **Going public + Actions billing.** Until then CI carries no information: every run fails in ~2 s
  without executing a step. **Judge by `./scripts/check.sh`**, never by a PR's checks. This also
  means the registry's scheduled verification job has nowhere to run yet — it is a script, invoked
  by hand.
- **Playwright E2E.** `main` still has none; the frontend is Vitest-only. Stage 4 hardening's.
- **Pruning after a clean crawl.** Agreed 2026-07-29, recorded on `Database::delete_page`, still
  unimplemented. Needs a test that a capped or errored crawl deletes nothing.

## Environment

Rust 1.96.1 · Node 26.3.0 · npm 11.16.0 · tauri-cli 2.5.0 · macOS 26.5 · arm64.
`cargo-deny` and `mandoc` present; `cargo-audit`, `cargo-fuzz`, and nightly are **not** (the gate
says so). Bash is 3.2 — no `mapfile`. 344 workspace Rust tests + 136 Vitest.
`npm audit` hits the live registry and fails the gate when npmjs.org is down — that is an outage,
not a finding.
