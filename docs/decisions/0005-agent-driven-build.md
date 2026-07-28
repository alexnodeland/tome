# ADR-0005: Build with agent workflows; cut scope accordingly

**Status:** Accepted
**Date:** 2026-07-28
**Deciders:** Alex Nodeland
**Resolves:** DEC-004

## Context

The plan totals ~381 person-days against a 30-week calendar, implying ~2.5 full-time engineers.
`06-dependency-map.md` allocated work to three developers; RISK-010 described a single maintainer.
Both could not be true, and every date in the plan depended on which.

The actual answer is neither: **one maintainer directing Fable and Opus agent workflows.**

That is not simply "a fast solo developer". It changes which constraint binds. Agent-written code
arrives fast, confident, idiomatic, and well-formatted whether or not it is correct — which
neutralises most of the heuristics a reviewer normally uses to judge code at a glance. The scarce
resource stops being time-to-write and becomes **confidence that what was written is right**.

## Decision

1. **Build via agent workflows**, with model routing by task shape: Fable for work that is exactly
   specified and loudly verified, Opus for design, security-critical modules, concurrency, and all
   adversarial verification.
2. **Verification artifacts are built before the code they verify**, by a different task. This is
   the central rule; everything else follows from it.
3. **Cut scope to Stages 0–4** of [`18-implementation-plan.md`](../plans/18-implementation-plan.md).
   Cross-device sync (Phase 3 of the original plan) is deferred.
4. **Re-sequence from horizontal to vertical.** A working slice — one real docs site fetched,
   normalized, rendered offline — comes before breadth, because it answers the riskiest product
   question first.

## Alternatives considered

**Keep the full 90-ticket scope.** Tempting, because agents make breadth cheap. Rejected: they make
breadth cheap and *depth* no cheaper. Sync convergence, the sanitizer, and the SSRF filter are
exactly as hard as before; more scope in those areas is more verification burden on one person.

**Keep the original horizontal phase order.** Rejected: it defers the answer to "can arbitrary docs
sites be normalized well?" until week 8, and that answer determines whether the product is worth
building.

**Hire or recruit contributors.** Not available now. The plan stays legible to a team if that
changes — the interface-freeze structure is exactly what makes work parallelisable across people
too.

## Consequences

**Better:** platform breadth (scrapers, UI components, CLI subcommands) becomes genuinely cheap via
fan-out behind frozen interfaces. Test and fixture authoring — historically the first thing a solo
project skips — becomes affordable, which is why the verification-first rule is realistic rather
than aspirational.

**Worse, and worth naming:**

- **Plausible-but-wrong output is the dominant risk**, and it is not one the original risk register
  contemplated. Mitigated by verification-first, adversarial passes, and diff-size checks.
- **Reviewer fatigue is a real failure mode.** The tenth large diff of the day gets less scrutiny
  than the first. Mitigated by keeping tickets small and automating everything checkable.
- **Agents are confidently wrong about external systems.** The original plan's Unix-socket MCP
  transport is what that looks like in practice. Spikes must execute against the real system and
  paste real output; a recalled API shape is never acceptable evidence.

## Reversibility

Fully reversible. The stage structure, frozen interfaces, and verification artifacts are all
independently valuable — if this becomes a team project, the same structure parallelises across
people. Deferred sync is specified and does not decay while waiting.
