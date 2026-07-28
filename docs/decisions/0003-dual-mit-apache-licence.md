# ADR-0003: Dual-licence under MIT OR Apache-2.0

**Status:** Accepted
**Date:** 2026-07-28
**Deciders:** Alex Nodeland
**Resolves:** DEC-001

## Context

No `LICENSE` file existed, while the Phase-5 landing-page copy asserted MIT and the NFR document
said "MIT or Apache 2.0 (TBD)". Without a licence, no licence is granted — the repository was
technically un-usable and un-contributable-to, and the registry and community-contribution plans
depended on resolving it.

## Decision

Dual-licence under **MIT OR Apache-2.0**, at the user's option. `LICENSE-MIT`, `LICENSE-APACHE`, and
a `LICENSE` pointer are committed. Contributions are dual-licensed on the same terms, under the DCO.

## Alternatives considered

**MIT alone.** Shortest, most familiar, and what most tools in this space use (DevDocs, Zeal). Lost
on the absence of an explicit patent grant, which matters slightly more for a project that may
attract corporate contributors.

**Apache-2.0 alone.** Explicit patent grant and contributor terms. Lost on being more friction than
some contributors want for a desktop app, and on being less common in this niche.

**Closed / no licence.** Rejected: incompatible with the source registry, community scraper
contributions, and the open-source positioning throughout the plan.

## Consequences

**Better:** maximum downstream compatibility; standard and immediately recognisable to Rust
contributors; the patent grant is available to anyone who wants it.

**Worse:** two files instead of one, and a slightly longer contribution note. Negligible.

Dependency licences must stay compatible with both — `cargo deny` enforces this in CI, which is
also why a licence allowlist is part of the S0 CI job rather than an afterthought.

## Reversibility

Very low cost to add a licence later; effectively irreversible to *remove* one once third parties
have relied on it. Since both options here are permissive, nothing downstream is constrained.
