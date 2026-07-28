# ADR-0001: Sync via an iCloud Drive container, not CloudKit

**Status:** Accepted
**Date:** 2026-07-28
**Deciders:** plan review

## Context

Bookmarks, annotations, collections and reading positions should follow a user between their Macs.
Documentation content and search indexes are explicitly not synced — they are large and easily
re-fetched.

The plan described **three incompatible mechanisms** in three documents: CloudKit `CKRecord`s in a
custom zone (Phase 3), a symlinked `~/.tome/icloud/*.json` (PRD), and JSON files under
`~/Library/Mobile Documents/…` (rollback plan). Only one can be built.

Two constraints were not weighed when CloudKit was chosen:

1. **The core engine is Rust.** CloudKit is Swift/Objective-C only. Adopting it puts the most
   state-heavy, hardest-to-debug component in the product across a language boundary from the
   database it mutates.
2. **The CLI and MCP server run outside the app process** and must see the same library. A CloudKit
   engine living in the app cannot serve them without inventing an IPC protocol.

RISK-002 scores CloudKit sync 16 (Critical) on undocumented edge cases, quotas, and conflict
handling — and its recorded contingency was already "iCloud Drive file-based sync".

## Decision

Sync through an **iCloud Drive ubiquity container**, using **per-device append-only operation logs**
replayed into local SQLite.

Each device writes only its own subdirectory, so two devices never write the same file and iCloud's
file-conflict machinery is never invoked. Convergence is deterministic: Lamport counter, then wall
clock, then device id; add-wins for set-valued fields; tombstones for deletes.

## Alternatives considered

**CloudKit.** Server-authoritative, well-documented conflict semantics, Apple-maintained. Lost on
the Swift boundary, on being unreachable from the out-of-process CLI, and on the size of its failure
surface for a single-user, low-write workload.

**A self-hosted sync server.** Full control, cross-platform later. Lost on contradicting local-first
and on the standing operational cost for a project whose funding is undecided (DEC-003).

**No sync at all.** Genuinely attractive — it is the recommended cut if the project stays solo
(DEC-004). Not chosen as the *design*, because if sync is built it should be built this way; the
decision to build it at all is DEC-004's.

**Naive file sync (one shared JSON per entity type).** Simplest, and wrong: concurrent writes
produce iCloud conflict copies that silently diverge. Per-device directories exist specifically to
avoid this.

## Consequences

**Better:** works from Rust; the CLI syncs; offline is the default path rather than a special case,
so no separate queue is needed (P3-015 shrank from M to S); failure modes are ours and therefore
testable — the design is verifiable by property tests over permuted operation sets.

**Worse:** we own convergence correctness, which Apple would otherwise have owned. Log growth needs
compaction. iCloud file eviction is a real hazard — a placeholder read without requesting download
returns nothing, which is the most likely cause of an apparent data loss. SPIKE-004 was rewritten to
measure exactly this before P3-010 commits.

## Reversibility

Moderate. The operation-log format is the abstraction; a CloudKit backend could emit and consume the
same operations. The cost of switching would be the Swift boundary, not a data migration —
`tome export` / `tome import` provides an escape hatch either way.
