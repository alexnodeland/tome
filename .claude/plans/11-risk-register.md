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

### RISK-001: Tauri + Swift Integration Complexity

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
Integrating Tauri (Rust) with custom Swift code for native macOS features (menu bar, global shortcuts) may prove more complex than anticipated, potentially requiring architectural changes.

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
- Pure Tauri: Lose menu bar popover, global shortcuts (Medium impact)
- Helper process: Additional complexity, potential latency (Low impact)
- Electron: 2-week delay, different architecture (High impact)

---

### RISK-002: CloudKit Sync Edge Cases

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
CloudKit has undocumented edge cases, rate limits, and conflict scenarios that may cause sync reliability issues.

**Triggers:**
- Sync failures exceed 0.5% threshold
- Data loss during conflict resolution
- Apple changes CloudKit behavior unexpectedly

**Mitigation:**
1. **Proactive:** Complete SPIKE-004 early, document all edge cases
2. **Design:** Conservative conflict resolution (never delete, always merge)
3. **Testing:** Extensive sync testing with multiple devices
4. **Monitoring:** Sync health metrics, failure alerting (local)

**Contingency:**
- Simpler sync: iCloud Drive file-based sync instead of CloudKit
- Optional sync: Make sync fully optional, document local-only workflow

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

## Risk Summary

| ID | Risk | Score | Level | Status |
|----|------|-------|-------|--------|
| RISK-001 | Tauri + Swift Integration | 15 | Critical | Open |
| RISK-002 | CloudKit Sync Edge Cases | 16 | Critical | Open |
| RISK-003 | Scraper Maintenance | 12 | High | Open |
| RISK-004 | Search Performance at Scale | 8 | Medium | Open |
| RISK-005 | Apple Notarization Changes | 8 | Medium | Open |
| RISK-006 | MCP Protocol Instability | 6 | Medium | Open |
| RISK-007 | WebView Rendering | 9 | Medium | Open |
| RISK-008 | Dependency Vulnerabilities | 12 | High | Open |
| RISK-009 | Scope Creep | 12 | High | Open |
| RISK-010 | Single Maintainer | 10 | High | Open |

---

## Risk Review Schedule

| Review Type | Frequency | Participants |
|-------------|-----------|--------------|
| Critical risks | Weekly | Project lead |
| High risks | Bi-weekly | Project lead |
| All risks | Monthly | All stakeholders |
| Post-phase retrospective | Per phase | All stakeholders |

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
