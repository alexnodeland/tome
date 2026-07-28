# ADR-0002: Ship without App Sandbox

**Status:** Accepted
**Date:** 2026-07-28
**Deciders:** plan review

## Context

The plan asserted three things that cannot all be true:

- `09-non-functional-requirements.md`: "Sandboxed: App Sandbox enabled"
- `05-phase-5-polish-launch.md`: an entitlements file that never requested the sandbox entitlement
- every other document: data stored in `~/.tome`

**A sandboxed app cannot write `~/.tome`** — the path is redirected into its container. And the
`tome` CLI installed by Homebrew is not sandboxed, so it would resolve `~/.tome` to the real
location. The app and the CLI would read **different libraries**.

That is not a detail. The CLI, the MCP server, and the GUI sharing one library is the integration
story that Phase 4 exists to deliver.

## Decision

Ship **Developer ID–signed and notarized, hardened runtime enabled, App Sandbox disabled.**

Data lives at `~/Library/Application Support/Tome` (state) and `~/Library/Caches/Tome`
(re-fetchable), reachable identically by the app, the CLI, and the MCP server.

## Alternatives considered

**Sandbox the app, keep the CLI outside it.** Rejected: two libraries, permanently confusing.

**Sandbox everything and ship the CLI inside the app bundle.** Rejected: `brew install` would not
put `tome` on `$PATH` in the usual way, and a sandboxed CLI cannot read documentation directories
the user names on the command line without a file-picker prompt — which defeats scripting.

**Use a shared App Group container.** Rejected: app groups are for code you sign and distribute
together; a Homebrew-installed binary that users may build themselves does not fit, and it does not
solve the file-picker problem.

**Ship on the Mac App Store instead.** Rejected: the App Store forbids the CLI, the local HTTP
server, and arbitrary network fetching of user-specified hosts. It would be a different product.

## Consequences

**Better:** one library for every entry point. No container-path translation. Local documentation
directories readable without a picker prompt every launch.

**Worse — and worth stating plainly:** we give up a real layer of defence in depth. A compromise of
the app process has the user's full file access rather than a container's.

Compensating controls, all now specified:

- Hardened runtime with `allow-jit` as the only exception. The original entitlements requested
  `allow-unsigned-executable-memory` and `disable-library-validation` — both materially weakening
  it, neither needed by a Rust + Tauri app.
- Untrusted documentation rendered in a script-disabled iframe under a strict CSP.
- Sanitization at ingest, once, rather than trusting content on every view.
- No network server by default; when enabled, loopback-bound and token-authenticated.
- Path validation confined to the data directory, symlink escapes rejected.

## Reversibility

Expensive. Enabling the sandbox later means either giving up the shared library — a product change,
not a build-flag change — or moving to a container path and migrating every existing user's data. If
a Mac App Store build is ever wanted, it is a separate target with a reduced feature set, not a
retrofit.
