# Support & Maintenance Plan

This document outlines the post-launch support model and maintenance practices for Tome.

---

## Support Model

### Support Channels

| Channel | Purpose | Response Time |
|---------|---------|---------------|
| **GitHub Issues** | Bug reports, feature requests | Best effort |
| **GitHub Discussions** | Q&A, community help | Community-driven |
| **Documentation** | Self-service help | Always available |

### No Dedicated Support

As an open-source project:
- No SLAs or guaranteed response times
- No paid support tiers
- Community-driven support via GitHub Discussions
- Maintainer(s) triage issues as time permits

---

## Issue Management

### Issue Templates

**Bug Report:**
```markdown
---
name: Bug Report
about: Report a bug in Tome
---

## Description
[Clear description of the bug]

## Steps to Reproduce
1.
2.
3.

## Expected Behavior
[What should happen]

## Actual Behavior
[What actually happens]

## Environment
- Tome version:
- macOS version:
- Chip: M1 / M2 / M3 / M4

## Logs
[Attach relevant logs from ~/.tome/logs/ if applicable]

## Screenshots
[If applicable]
```

**Feature Request:**
```markdown
---
name: Feature Request
about: Suggest a feature for Tome
---

## Problem
[What problem does this solve?]

## Proposed Solution
[How would this feature work?]

## Alternatives Considered
[Other approaches you've thought about]

## Additional Context
[Any other relevant information]
```

### Issue Labels

| Label | Description |
|-------|-------------|
| `bug` | Something isn't working |
| `enhancement` | New feature or improvement |
| `documentation` | Documentation updates |
| `good first issue` | Good for newcomers |
| `help wanted` | Extra attention needed |
| `question` | Further information requested |
| `duplicate` | Already reported |
| `wontfix` | Will not be addressed |
| `P0-critical` | Critical bug, blocks usage |
| `P1-high` | High priority |
| `P2-medium` | Medium priority |
| `P3-low` | Low priority |

### Issue Triage Process

```
New Issue
    │
    ▼
┌─────────────────┐
│ Valid issue?    │──No──► Close with explanation
└────────┬────────┘
         │ Yes
         ▼
┌─────────────────┐
│ Duplicate?      │──Yes─► Link to original, close
└────────┬────────┘
         │ No
         ▼
┌─────────────────┐
│ Add labels:     │
│ - Type          │
│ - Priority      │
│ - Component     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Needs more info?│──Yes─► Request info, add label
└────────┬────────┘
         │ No
         ▼
     Ready for work
```

---

## Release Management

### Release Types

| Type | Versioning | Trigger | Examples |
|------|------------|---------|----------|
| **Major** | X.0.0 | Breaking changes | v2.0.0 |
| **Minor** | x.Y.0 | New features | v1.1.0 |
| **Patch** | x.y.Z | Bug fixes | v1.0.1 |
| **Hotfix** | x.y.Z | Critical fixes | v1.0.2 |

### Release Cadence

- **No fixed schedule** - Release when ready
- **Minor releases** - As features complete
- **Patch releases** - As bugs are fixed
- **Hotfixes** - Within 24-48 hours of critical bugs

### Release Process

```bash
# 1. Ensure main is stable
git checkout main
git pull
cargo test && npm test

# 2. Update version numbers
# - Cargo.toml
# - package.json
# - tauri.conf.json

# 3. Update CHANGELOG.md
# 4. Commit
git add -A
git commit -m "chore: release v1.1.0"

# 5. Tag
git tag v1.1.0
git push origin main --tags

# 6. CI builds, signs, notarizes, publishes
# 7. Write release notes on GitHub
# 8. Announce (if significant)
```

### Release Notes Template

```markdown
# Tome v1.1.0

## What's New

### Features
- **New feature** - Description (#123)

### Improvements
- Improvement description (#124)

### Bug Fixes
- Fixed bug description (#125)

## Breaking Changes
None

## Upgrade Notes
No action required. Simply download the new version.

## Download
- [Tome-1.1.0.dmg](link)
- Homebrew: `brew upgrade --cask tome`

## Checksums
```
SHA256: abc123...
```
```

---

## Maintenance Tasks

### Regular Maintenance

| Task | Frequency | Description |
|------|-----------|-------------|
| Dependency updates | Weekly | Review Dependabot PRs |
| Security audit | Monthly | Run `cargo audit`, `npm audit` |
| Issue triage | Weekly | Review and label new issues |
| Documentation review | Monthly | Update outdated docs |
| Performance check | Quarterly | Run benchmarks, profile |

### Dependency Updates

```yaml
# Dependabot config (.github/dependabot.yml)
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/src-tauri"
    schedule:
      interval: "weekly"
    reviewers:
      - "maintainer"

  - package-ecosystem: "npm"
    directory: "/"
    schedule:
      interval: "weekly"
    reviewers:
      - "maintainer"
```

**Update policy:**
- Patch versions: Auto-merge if CI passes
- Minor versions: Review changelog, merge if safe
- Major versions: Evaluate breaking changes, test thoroughly

### Security Updates

When a CVE is reported:

1. **Assess severity** - Critical/High = immediate action
2. **Check if affected** - Is the vulnerable code path used?
3. **Update dependency** - Bump to fixed version
4. **Release patch** - If critical, release same day
5. **Document** - Note in changelog

---

## Monitoring

### What to Monitor

Since Tome has no telemetry, monitoring is external:

| Metric | Source | Action |
|--------|--------|--------|
| GitHub stars/forks | GitHub | Community growth indicator |
| Issue volume | GitHub | Support load indicator |
| Download counts | GitHub Releases | Adoption indicator |
| Homebrew installs | Homebrew analytics | Adoption indicator |
| CVEs in deps | GitHub Security | Security response |

### Health Indicators

**Healthy project:**
- Issues triaged within 1 week
- Critical bugs fixed within 1 week
- Regular releases (at least quarterly)
- Dependencies reasonably up to date

**Warning signs:**
- Issue backlog growing
- No releases for 6+ months
- Many outdated dependencies
- Unaddressed security issues

---

## Documentation Maintenance

### Documentation Types

| Type | Location | Update Trigger |
|------|----------|----------------|
| User docs | `/docs/` | Feature changes |
| API docs | `/docs/api/` | API changes |
| README | `/README.md` | Project changes |
| Changelog | `/CHANGELOG.md` | Every release |
| Code comments | Source files | Code changes |

### Documentation Review Checklist

- [ ] Screenshots are current
- [ ] CLI examples work
- [ ] API examples work
- [ ] Links are not broken
- [ ] Version numbers are correct
- [ ] No references to removed features

---

## End of Life Planning

### Version Support

| Version | Status | Support Until |
|---------|--------|---------------|
| v1.x | Current | Until v2.0 + 6 months |
| v0.x | Deprecated | No support |

### EOL Process

When a major version reaches end of life:

1. **Announce** - 3 months before EOL
2. **Document** - Migration guide to new version
3. **Final patch** - Security fixes only
4. **Archive** - Mark branch as archived
5. **Redirect** - Point users to new version

### Project Sunset (if necessary)

If the project is no longer maintained:

1. **Announce** - Clear notice in README and releases
2. **Archive** - Archive GitHub repository
3. **Document** - Final state and alternatives
4. **Handoff** - Offer to transfer to new maintainer

---

## Community Contributions

### Contribution Guidelines

```markdown
# Contributing to Tome

## Getting Started
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Write/update tests
5. Submit a pull request

## Code Standards
- Run `cargo fmt` and `npm run format`
- Run `cargo clippy` and `npm run lint`
- Ensure all tests pass
- Add tests for new functionality

## Pull Request Process
1. Update documentation if needed
2. Add entry to CHANGELOG.md
3. Request review from maintainer
4. Address review feedback
5. Squash commits before merge
```

### Recognition

- Contributors listed in README
- Release notes credit contributors
- Good first issues for newcomers

---

## Emergency Procedures

### Critical Bug Response

```
Critical Bug Reported
        │
        ▼
┌───────────────────┐
│ Verify bug        │
│ - Reproduce       │
│ - Assess impact   │
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│ Notify users      │
│ - GitHub issue    │
│ - Add warning     │
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│ Develop fix       │
│ - Hotfix branch   │
│ - Minimal change  │
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│ Test thoroughly   │
│ - Unit tests      │
│ - Manual testing  │
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│ Release hotfix    │
│ - Tag             │
│ - Build/sign      │
│ - Publish         │
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│ Post-mortem       │
│ - Root cause      │
│ - Prevention      │
└───────────────────┘
```

### Data Loss Scenario

If a bug causes data loss:

1. **Stop the bleeding** - Identify and warn users
2. **Release fix** - Prevent further data loss
3. **Recovery guidance** - Help users recover if possible
4. **Root cause analysis** - Understand how it happened
5. **Prevention** - Add safeguards to prevent recurrence
