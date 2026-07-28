# ADR-0004: Bundle identifier `com.alexnodeland.tome`

**Status:** Accepted
**Date:** 2026-07-28
**Deciders:** Alex Nodeland
**Resolves:** DEC-002

## Context

`com.example.tome` — a placeholder — was threaded through the notarization script, the Keychain
service name, the iCloud container identifier, and the Homebrew `zap` list. Each of those is a
place where a wrong identifier either breaks signing or silently orphans user data, and the
identifier is effectively permanent once shipped: changing it later means users' Keychain entries,
iCloud container, and preferences all move.

## Decision

**`com.alexnodeland.tome`.**

Derived identifiers, all from a single constant:

| Use | Value |
|---|---|
| Bundle identifier | `com.alexnodeland.tome` |
| iCloud container | `iCloud.com.alexnodeland.tome` |
| Keychain service | `com.alexnodeland.tome` |
| Preferences domain | `com.alexnodeland.tome.plist` |
| Homebrew zap | derived from the above |

## Alternatives considered

**`studio.ournature.tome`.** Reverse-DNS of a domain already in use. Lost on tying a personal
open-source project to a separate brand — awkward if either changes direction independently.

**`dev.tome.app`.** Cleanest product-first identity, and matches the `tome.dev` landing page the
plan assumes. Lost on requiring ownership of a domain that is not currently held; adopting an
identifier based on a domain you do not control is a problem that surfaces at the worst moment.

## Consequences

**Better:** unambiguous, controlled by the maintainer, no external dependency. Signing,
entitlements, and the container identifier all agree from the start rather than being reconciled
during the first release attempt.

**Worse:** ties the identifier to a personal handle. If Tome later moves to an organisation, the
identifier will look slightly dated — which is cosmetic, and much cheaper than a migration.

The landing page domain is a **separate** decision. The bundle identifier does not have to match it,
and deliberately does not depend on it.

## Reversibility

Expensive after the first public release: Keychain entries, the iCloud container, and preferences
are all keyed on it, so a change requires a migration path for each. Cheap right now, which is why
it was settled before any code was written.
