# Risk Register

This document tracks identified risks, their assessment, and mitigation strategies.

---

## Risk Assessment Framework

### Impact Scale

| Level | Score | Description |
|-------|-------|-------------|
| **Critical** | 5 | Project failure, major rewrite required |
| **High** | 4 | Significant delay, feature removal |
| **Medium** | 3 | Moderate delay, workaround needed |
| **Low** | 2 | Minor delay, cosmetic impact |
| **Minimal** | 1 | Negligible impact |

### Probability Scale

| Level | Score | Description |
|-------|-------|-------------|
| **Almost Certain** | 5 | > 80% chance |
| **Likely** | 4 | 60-80% chance |
| **Possible** | 3 | 40-60% chance |
| **Unlikely** | 2 | 20-40% chance |
| **Rare** | 1 | < 20% chance |

### Risk Score

**Risk Score = Impact × Probability**

| Score Range | Risk Level | Action |
|-------------|------------|--------|
| 15-25 | **Critical** | Immediate mitigation required |
| 10-14 | **High** | Active mitigation, regular review |
| 5-9 | **Medium** | Mitigation plan, periodic review |
| 1-4 | **Low** | Monitor, accept |

---

## Active Risks

### RISK-001: Shell architecture unvalidated (Tauri + Swift)

| Attribute | Value |
|-----------|-------|
| **Category** | Technical |
| **Phase** | P1 |
| **Impact** | 5 (Critical) |
| **Probability** | 3 (Possible) |
| **Score** | 15 (Critical) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
The entire plan — 23 Phase-1 tickets — is written against a shell architecture that no spike has
validated. The specific concern is integrating Tauri (Rust) with custom Swift for native macOS
features (menu-bar popover, global shortcut).

**The exposure is smaller than it was**, because the architecture was corrected: Tauri *is* the
shell, and Swift is now an optional Phase-5 plugin for two non-critical features rather than a
foundational layer (see [PRD § Stack](../PRD.md#stack)). What remains is that Phase 1 still rests
on unmeasured assumptions about the Tauri/WebView bridge.

**Triggers:**
- SPIKE-001 reveals fundamental incompatibilities
- IPC latency exceeds acceptable thresholds
- Memory management issues between Rust and Swift

**Mitigation:**
1. **Proactive:** Complete SPIKE-001 before Phase 1 commits
2. **Alternative:** Pure Tauri implementation with limited native features
3. **Fallback:** Separate Swift helper process communicating via IPC
4. **Contingency:** Evaluate Electron as last resort (significant pivot)

**Contingency Impact:**
- Pure Tauri: lose the menu-bar popover (a plain menu still works) and use Tauri's global-shortcut
  plugin. **Low impact, and paid in Phase 5 rather than Phase 1** — which is the point of demoting
  Swift out of the foundation.
- Helper process: additional complexity, potential latency (Low impact)
- Electron: significant pivot; only if the WebView bridge itself proves unworkable (High impact)

---

### RISK-002: Sync correctness and data loss

| Attribute | Value |
|-----------|-------|
| **Category** | Technical |
| **Phase** | P3 |
| **Impact** | 4 (High) |
| **Probability** | 4 (Likely) |
| **Score** | 16 (Critical) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Cross-device sync silently loses or duplicates user-authored data. This is the worst failure mode
in the product: a bookmark that vanishes is noticed late and never forgiven.

**The design has changed since this risk was written.** v1 uses an iCloud Drive ubiquity container
with per-device append-only logs rather than CloudKit — see
[PRD § iCloud Sync Architecture](../PRD.md#icloud-sync-architecture). That was this risk's own
recorded contingency, promoted to the primary plan because the core is Rust and CloudKit is not.
The risk score does not drop: the failure modes change rather than disappear.

**Triggers:**
- Any reproducible lost write in the fault-injection suite
- Duplicate records appearing after a two-device sync (the original schema guaranteed this — it
  keyed bookmarks on `device_id`)
- iCloud evicting a log file and a read returning empty rather than downloading
- Convergence property tests failing on permuted op sets

**Mitigation:**
1. **Proactive:** SPIKE-004 measures the container's real behaviour before P3-010 commits
2. **Design:** per-device directories (no file-level conflicts possible); tombstones, never
   physical deletes; add-wins sets; concurrent note edits preserved, never overwritten
3. **Testing:** property-based convergence tests over permuted op sets; crash injection at every
   await point; two real Macs before release
4. **Bounded blast radius:** sync failure never blocks a local action

**Contingency:**
- **Ship v1.0 with no sync at all.** This is now the recommended cut if the project is solo
  (DEC-004) — 68 person-days saved and the highest risk in the register retired
- Fall back to CloudKit, accepting a Swift boundary
- Export/import as a manual path between machines

---

### RISK-003: Platform Scraper Maintenance Burden

| Attribute | Value |
|-----------|-------|
| **Category** | Operational |
| **Phase** | P2+ |
| **Impact** | 3 (Medium) |
| **Probability** | 4 (Likely) |
| **Score** | 12 (High) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Documentation platforms (Sphinx, rustdoc, etc.) evolve their output formats, requiring ongoing scraper maintenance.

**Triggers:**
- Sphinx 8.x changes output format
- rustdoc changes search-index.js structure
- New popular platforms emerge requiring scrapers

**Mitigation:**
1. **Architecture:** Modular scraper design, easy to update individually
2. **Graceful fallback:** All scrapers fall back to generic scraper
3. **Version detection:** Auto-detect platform version, adjust parsing
4. **Community:** Open-source scrapers for community contributions

**Contingency:**
- Rely more heavily on generic scraper
- Defer new platform support to community PRs

---

### RISK-004: Search Performance at Scale

| Attribute | Value |
|-----------|-------|
| **Category** | Performance |
| **Phase** | P2 |
| **Impact** | 4 (High) |
| **Probability** | 2 (Unlikely) |
| **Score** | 8 (Medium) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Tantivy performance may degrade with very large documentation sets (100K+ pages), affecting user experience.

**Triggers:**
- Search latency exceeds 100ms P95
- Memory usage during search exceeds 1GB
- Index corruption under load

**Mitigation:**
1. **Proactive:** Complete SPIKE-003, establish scaling limits
2. **Architecture:** Index sharding by source if needed
3. **Optimization:** Lazy loading, segment management
4. **Documentation:** Publish recommended limits

**Contingency:**
- Implement index sharding
- Offer "archive" mode for rarely-used sources (not indexed)

---

### RISK-005: Apple Notarization Changes

| Attribute | Value |
|-----------|-------|
| **Category** | Distribution |
| **Phase** | P5 |
| **Impact** | 4 (High) |
| **Probability** | 2 (Unlikely) |
| **Score** | 8 (Medium) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Apple may change notarization requirements, breaking the release pipeline.

**Triggers:**
- New macOS version with stricter requirements
- Changes to notarytool API
- New entitlement requirements

**Mitigation:**
1. **Monitoring:** Track Apple developer announcements
2. **Testing:** Test on beta macOS versions
3. **Automation:** Modular release pipeline, easy to update
4. **Timeline:** Build releases before major macOS launches

**Contingency:**
- Manual notarization process temporarily
- Delay releases until pipeline updated

---

### RISK-006: MCP Protocol Instability

| Attribute | Value |
|-----------|-------|
| **Category** | Technical |
| **Phase** | P4 |
| **Impact** | 2 (Low) |
| **Probability** | 3 (Possible) |
| **Score** | 6 (Medium) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
MCP (Model Context Protocol) is relatively new; the protocol may evolve, requiring updates to Tome's implementation.

**Triggers:**
- Breaking protocol changes
- Claude Code updates MCP client incompatibly
- New required features

**Mitigation:**
1. **Proactive:** SPIKE-008 to understand protocol deeply
2. **Versioning:** Implement protocol version negotiation
3. **Monitoring:** Track MCP specification updates
4. **Abstraction:** Abstract protocol layer for easy updates

**Contingency:**
- MCP is optional; HTTP API remains primary integration
- Version-specific protocol handlers

---

### RISK-007: WebView Rendering Inconsistencies

| Attribute | Value |
|-----------|-------|
| **Category** | Technical |
| **Phase** | P1 |
| **Impact** | 3 (Medium) |
| **Probability** | 3 (Possible) |
| **Score** | 9 (Medium) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
WKWebView may render documentation inconsistently, especially complex layouts, tables, or code blocks.

**Triggers:**
- Specific documentation sites render poorly
- Code syntax highlighting breaks
- Tables overflow or break layout

**Mitigation:**
1. **Normalization:** Strong HTML normalization pipeline
2. **CSS reset:** Aggressive default styling
3. **Testing:** Test with diverse documentation sources
4. **Fallback:** "View original" option for problematic pages

**Contingency:**
- More aggressive content simplification
- User-configurable styling overrides

---

### RISK-008: Dependency Vulnerabilities

| Attribute | Value |
|-----------|-------|
| **Category** | Security |
| **Phase** | All |
| **Impact** | 4 (High) |
| **Probability** | 3 (Possible) |
| **Score** | 12 (High) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Third-party dependencies may have security vulnerabilities discovered after release.

**Triggers:**
- CVE published for a dependency
- Dependabot/advisory alert
- Security audit findings

**Mitigation:**
1. **Automation:** Dependabot for automated updates
2. **Scanning:** cargo audit, npm audit in CI
3. **Policy:** No dependencies with known high/critical CVEs
4. **Review:** Monthly dependency review

**Contingency:**
- Emergency patch release process
- Temporary feature disable if dependency is deeply embedded

---

### RISK-009: Scope Creep

| Attribute | Value |
|-----------|-------|
| **Category** | Project |
| **Phase** | All |
| **Impact** | 3 (Medium) |
| **Probability** | 4 (Likely) |
| **Score** | 12 (High) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Feature requests and "nice to have" improvements may expand scope, delaying v1.0 release.

**Triggers:**
- Adding features not in original spec
- Perfectionism on non-critical features
- Responding to every user request

**Mitigation:**
1. **Discipline:** Strict adherence to phase deliverables
2. **Backlog:** New ideas go to v1.x backlog, not v1.0
3. **Review:** Regular scope check against original PRD
4. **MVP mindset:** Ship working software, iterate

**Contingency:**
- Cut non-critical features from current phase
- Extend timeline with stakeholder agreement

---

### RISK-010: Single Maintainer Bus Factor

| Attribute | Value |
|-----------|-------|
| **Category** | Operational |
| **Phase** | All |
| **Impact** | 5 (Critical) |
| **Probability** | 2 (Unlikely) |
| **Score** | 10 (High) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
With limited contributors, project continuity depends on key individuals.

**Triggers:**
- Maintainer unavailability
- Loss of context/knowledge
- Burnout

**Mitigation:**
1. **Documentation:** Comprehensive docs (this planning work!)
2. **Architecture:** Clean, understandable codebase
3. **Open source:** Enable community contributions
4. **Automation:** Reduce manual maintenance burden

**Contingency:**
- Project handoff documentation
- Community fork if original maintainer unavailable

---

### RISK-011: Legal / ToS exposure from scraping third-party documentation

| Attribute | Value |
|-----------|-------|
| **Category** | Legal |
| **Phase** | All |
| **Impact** | 4 (High) |
| **Probability** | 4 (Likely) |
| **Score** | 16 (Critical) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Tome's entire value proposition is fetching, transforming, caching, and displaying documentation
that other people wrote and own. **The original risk register contained no legal risk at all** —
which is remarkable for a product whose core loop is automated retrieval of third-party
copyrighted content. Exposure comes in three distinguishable flavours, and conflating them is how
this gets handled badly:

1. **Crawling behaviour.** Ignoring `robots.txt` or hammering a host. Purely technical, entirely
   within our control, and the original plan made `robots.txt` compliance *optional* — the single
   most likely way this risk actually materializes.
2. **Local caching.** A user's machine storing pages they could have visited. The strongest
   position, and close to what every browser cache already does.
3. **Redistribution.** Shipping content, or a service that serves content. Tome does **not** do
   this, and must never start — the registry ships *configurations*, not documentation.

**Triggers:**
- A documentation host blocks Tome's User-Agent or an IP range
- A takedown request, or a complaint on a registry PR
- A host's ToS explicitly forbidding automated access is found among the registry sources
- Anyone proposing to host scraped content "for convenience"

**Mitigation:**
1. **SPIKE-010** establishes the position before Phase 1 shapes the product around it
2. `robots.txt` obeyed by default and non-optional for bundled configurations; honest User-Agent;
   conservative rate limits; `Retry-After` honoured
3. Attribution enforced in the reader: every page links to its origin and shows the upstream
   licence where determinable
4. **The registry contains configuration only** — this is a structural mitigation, not a policy
   one, and it is why the registry was designed that way
5. A published takedown process, and a per-host opt-out list shipped with the app
6. Prior art: Dash, DevDocs, and Zeal have all operated in this space publicly for years

**Contingency:**
- Remove a source from the registry on request, same day; users may still add it manually on their
  own machine, which is a materially different act
- If a major host objects to the tool as such, engage directly — being blockable by User-Agent is
  precisely what makes good-faith negotiation possible

---

### RISK-012: Capacity — the plan requires a team the project does not have

| Attribute | Value |
|-----------|-------|
| **Category** | Project |
| **Phase** | All |
| **Impact** | 5 (Critical) |
| **Probability** | 3 (Possible) |
| **Score** | 15 (Critical) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
The plan is ~381 person-days against a 30-week calendar, which implies ~2.5 FTE. `06-dependency-map.md`
allocates work to three named developers; RISK-010 in this same register describes a single-maintainer
bus factor. **Both cannot be true**, and the plan has been written as though the first is, while
the risk register assumes the second. Solo, this is ~77 weeks full-time — and roughly four years
at side-project pace, by which time several platform scrapers will have rotted (RISK-003).

This is the risk that most changes the shape of everything else, and it was the only one not
recorded.

**Triggers:**
- Phase 1 exceeding 12 weeks
- Spikes not run because there is no time
- Scrapers breaking faster than they are fixed

**Mitigation:**
1. **Answer DEC-004 before writing code.** Everything else follows from it
2. If solo: cut to P1 + P2 + the MCP half of P4 (~55 % of effort, both differentiated features)
3. Sequence for early feedback: one real docs site readable end-to-end in ~3 weeks, before UI work
4. Prefer boring, well-maintained dependencies over building; every custom component is
   maintenance forever

**Contingency:**
- Cut Phase 3 (sync) entirely — retires RISK-002 as well
- Ship v0.x publicly and let real usage prioritize the rest

---

### RISK-013: Cold start — an empty library is not a product

| Attribute | Value |
|-----------|-------|
| **Category** | Product |
| **Phase** | P5 |
| **Impact** | 4 (High) |
| **Probability** | 3 (Possible) |
| **Score** | 12 (High) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Dash and DevDocs win on the curated catalog: install, and the docs are there. Tome as originally
specified asks the user to hand-author YAML before it does anything useful. First-run friction is
where most of these products lose people, and no ticket addressed it.

**Mitigation:**
1. Source registry as a **v1.0 requirement** (PRD § Source Registry), 30 verified sources at launch
2. Onboarding installs the first source in one click, from the registry
3. Registry configs CI-verified against live sites, so they work when a new user tries them
4. DEC-005: consider promoting Dash docset import — the cheapest possible catalog

**Contingency:**
- Ship fewer registry entries but cover the top ten languages properly

---

### RISK-014: Annotation loss through content drift

| Attribute | Value |
|-----------|-------|
| **Category** | Technical |
| **Phase** | P3 |
| **Impact** | 4 (High) |
| **Probability** | 4 (Likely) |
| **Score** | 16 (Critical) |
| **Owner** | TBD |
| **Status** | Open |

**Description:**
Documentation is re-fetched on a schedule. Highlights stored as character offsets — as originally
specified — shift silently whenever upstream text changes above them, so annotations end up
attached to the wrong words. **This is worse than deleting them**, because the user is not told:
they return to a highlight that now marks unrelated text and lose trust in every other annotation.
Probability is "Likely" because upstream documentation changes constantly; it is not an edge case,
it is the normal case.

**Mitigation:**
1. Quote + prefix/suffix anchoring, offsets as hints only (PRD § Annotation anchoring)
2. Re-anchor pass on every content-hash change
3. Explicit `approximate` and `orphaned` states, surfaced to the user; never silent
4. Property tests: random content edits must leave every annotation resolved or explicitly orphaned
5. Never delete an annotation automatically

**Contingency:**
- Pin annotated pages to the content version they were annotated against, and offer a diff

---

## Risk Summary

| ID | Risk | Score | Level | Status |
|----|------|-------|-------|--------|
| RISK-011 | Legal / ToS exposure from scraping | 16 | Critical | Open |
| RISK-002 | Sync correctness and data loss | 16 | Critical | Open |
| RISK-014 | Annotation loss through content drift | 16 | Critical | Open |
| RISK-001 | Shell architecture unvalidated | 15 | Critical | Open |
| RISK-012 | Capacity vs. plan size | 15 | Critical | Open |
| RISK-003 | Scraper Maintenance | 12 | High | Open |
| RISK-008 | Dependency Vulnerabilities | 12 | High | Open |
| RISK-009 | Scope Creep | 12 | High | Open |
| RISK-013 | Cold start / empty library | 12 | High | Open |
| RISK-010 | Single Maintainer | 10 | High | Open |
| RISK-007 | WebView Rendering | 9 | Medium | Open |
| RISK-004 | Search Performance at Scale | 8 | Medium | Open |
| RISK-005 | Apple Notarization Changes | 8 | Medium | Open |
| RISK-006 | MCP Protocol Instability | 6 | Medium | Open |

**Every owner is TBD.** For a solo project that is honest but not useful: an unowned risk is an
unmanaged one. At minimum, mark which risks are *accepted* rather than leaving all fourteen open.

---

## Risk Review Schedule

| Review Type | Frequency | Participants |
|-------------|-----------|--------------|
| Critical risks | At each phase gate, and when a trigger fires | Project lead |
| All risks | Monthly | Project lead |
| Post-phase retrospective | Per phase | Project lead |

> **Rewritten to be survivable.** The original schedule specified weekly critical-risk reviews,
> bi-weekly high-risk reviews, monthly all-risk reviews with "all stakeholders", plus per-phase
> retrospectives — a standing meeting load for a project that RISK-010 says has one maintainer and
> no stakeholders. Process nobody performs is worse than lighter process that actually happens:
> it makes the register look maintained when it is stale. Trigger-driven review is the part that
> earns its keep, which is why every risk above lists explicit triggers.

---

## Closed Risks

*No closed risks yet.*

---

## Risk Template

When adding new risks:

```markdown
### RISK-XXX: [Title]

| Attribute | Value |
|-----------|-------|
| **Category** | [Technical/Operational/Security/Project/Distribution] |
| **Phase** | [Phase affected] |
| **Impact** | [1-5] ([Level]) |
| **Probability** | [1-5] ([Level]) |
| **Score** | [N] ([Level]) |
| **Owner** | [Name/TBD] |
| **Status** | [Open/Mitigating/Closed] |

**Description:**
[What could go wrong]

**Triggers:**
- [How we'll know the risk is materializing]

**Mitigation:**
1. [Prevention/reduction strategies]

**Contingency:**
[What to do if risk materializes]
```
