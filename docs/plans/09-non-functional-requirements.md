# Non-Functional Requirements

This document defines the quality attributes and constraints for Tome beyond functional features.

---

## Platform Requirements

### Supported Systems

| Requirement | Specification |
|-------------|---------------|
| **macOS Version** | macOS 12 (Monterey) and later |
| **Architecture** | Apple Silicon (M1, M2, M3, M4) only |
| **Intel Support** | Not supported |

### Rationale

- Simplifies testing matrix and reduces maintenance burden for a small team
- Intel Macs are being phased out; new users predominantly on Apple Silicon
- Native ARM64 binaries, no universal-binary build complexity

> **Two honest caveats.**
>
> "Unified memory architecture beneficial for search indexing" was listed as a rationale. It is
> not one: Tantivy is memory-mapped and I/O-bound, and nothing in the design exploits UMA. Building
> a universal binary would cost a build-matrix entry, not an architecture. **The real reason is
> scope control**, which is a good reason — it just shouldn't be dressed up as a technical one.
>
> This narrows the addressable market more than any competitor's. The PRD's original competitive
> table criticized Zeal for "limited platform support" while Zeal runs on three platforms and Tome
> runs on one, on one CPU family. That framing has been corrected; the decision stands, but it is
> a **cost**, not an advantage.

---

## Performance Requirements

### Startup Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Cold start to usable | < 500ms | Time from launch to library visible |
| Warm start | < 200ms | Subsequent launches (cached) |
| Time to first search | < 1s | From launch to search results |

### Search Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Simple query latency | < 50ms | P50, 10K pages indexed |
| Complex query latency | < 100ms | P95, 10K pages indexed |
| Fuzzy query latency | < 150ms | P95, with typo correction |
| Results rendering | < 50ms | Time to display results |

### Indexing Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Full index build | < 30s | 1,000 pages |
| Incremental update | < 5s | 100 changed pages |
| Index size on disk | < 50MB | Per 1,000 pages |

> **Note the tension with SPIKE-003**, whose success criterion is "index size < 500 MB for 100K
> pages" — i.e. 5 MB per 1,000, a tenfold difference from the line above. One of these is wrong,
> and until the spike runs nobody knows which. Treat 50 MB/1K as the *ceiling* and 5 MB/1K as the
> *hope*; SPIKE-003 replaces both with a measurement.

### Memory Usage

**This document is the single owner of performance targets.** The plan previously stated idle
memory four different ways — `< 200MB` (PRD), `< 500MB at 100K pages` (here), `< 100MB for 10,000
pages` (P2-001), and `< 50MB with the index not loaded` (SPIKE-003) — with no statement of what
"idle" includes. The numbers below are reconciled and each one names its scenario.

| Metric | Target | Scenario |
|--------|--------|----------|
| Idle, baseline | < 200 MB | App launched, 10 sources / ~5K pages, index open, no activity |
| Idle, index not opened | < 80 MB | Immediately post-launch, before the first search |
| Active browsing | < 400 MB | Reading, page cache warm |
| Peak during index | < 1 GB | Full reindex of 100K pages |
| Idle, large library | < 500 MB | 100 sources / 100K pages, index open |

All are measured by the instrumented launch test, on the reference machine, against the fixture
library — not by reading Activity Monitor by hand, which was the original "measurement method" and
is not reproducible.

### Network Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Scrape rate | 5 req/sec | Default, configurable |
| Page fetch timeout | 30s | Per page |
| Crawl timeout | 10 min | Per source |

---

## Reliability Requirements

### Availability

| Metric | Target | How it is actually verified |
|--------|--------|------------------------------|
| Panics in the parse/sanitize pipeline | 0 | `cargo-fuzz` over the real-page corpus |
| Data corruption incidents | 0 | Crash-injection tests at every await point; `PRAGMA integrity_check` in the recovery suite |
| Sync convergence | 100% | `proptest` over permuted op sets across simulated devices |
| Lost writes under fault injection | 0 | Kill-and-restart harness |

> **"Crash-free sessions > 99.9%" and "sync success rate > 99.5%" were removed.** Both are
> percentages *of production events*, and Tome collects no telemetry — there is no mechanism by
> which either could ever be observed. Neither defines its denominator either ("a session"? "an
> operation"?). The replacements above are absolute, testable properties that can fail a build.

### Data Integrity

- **No data loss**: Bookmarks, annotations, and reading positions must never be lost
- **Atomic operations**: Database writes must be transactional
- **Graceful degradation**: Network failures should not corrupt local state
- **Recovery**: Corrupted indexes must be automatically rebuildable

### Error Handling

- All errors must be caught and handled gracefully
- User-facing errors must be actionable
- No stack traces shown to users
- Errors logged for debugging (local only, no telemetry)

---

## Security Requirements

### Data Privacy

| Requirement | Implementation |
|-------------|----------------|
| **No telemetry** | Zero data collection or phone-home. **No exceptions, including "opt-in error telemetry"** — an earlier Phase 5 criterion proposed exactly that and has been removed. |
| **No analytics** | No usage tracking |
| **Local-first** | All data stored locally by default |
| **iCloud optional** | Sync is opt-in, not required |
| **No third-party requests at read time** | Documentation assets are localized at ingest, so opening a page never contacts the origin — otherwise the origin learns what the user reads and when |

**The cost of this stance, stated plainly:** no funnel data, no crash aggregation, no feature-usage
signal, no A/B testing. Product direction comes from conversations and issues. Every "% of users"
metric elsewhere in this plan set has been removed because it could not be measured — see
[PRD § Success Metrics](../PRD.md#success-metrics).

### Network Security

| Requirement | Implementation |
|-------------|----------------|
| **HTTPS by default** | `http://` is rejected unless the source sets `fetch.allow_insecure: true` (intranet mirrors, localhost). Note this is a *reject*, not a silent auto-upgrade — see below. |
| **Certificate validation** | Standard TLS verification; no custom CA, no pinning |
| **No tracking cookies** | Stateless scraping; cookie jar not persisted between sources |
| **SSRF filter** | Every URL validated post-DNS-resolution, and re-validated on each redirect |
| **Egress is bounded** | Only to hosts declared by a source config; no analytics, no update pings without user action |

> **Silent `http:` → `https:` upgrade was removed.** The original code sample rewrote the scheme
> and continued. That is not a security control — it fails confusingly on hosts that genuinely have
> no TLS, and it hides from the user that their configuration named an insecure URL. Reject with a
> message naming `allow_insecure` instead.

### Local Security

| Requirement | Implementation |
|-------------|----------------|
| **Hardened runtime** | Enabled, with the minimum exception set (JIT only) |
| **App Sandbox** | **Not enabled** — see below |
| **No elevated privileges** | Standard user permissions only |
| **API bound to loopback** | HTTP API bound to 127.0.0.1, **off by default** |
| **API authenticated** | Bearer token required on every request, loopback included |
| **API not browser-reachable** | No CORS headers by default; `Host`/`Origin` validated |
| **Secrets in Keychain** | API token never written to disk in plaintext |

> **Correction: App Sandbox is off, deliberately.** This document previously required
> "App Sandbox enabled" while the Phase 5 entitlements file never requested it and every data path
> in the plan was `~/.tome` — which a sandboxed app cannot write. The three could not all be true.
>
> The design requires the GUI app, the Homebrew-installed CLI, and the MCP server to share one
> library. A sandboxed app's paths are redirected into its container; the unsandboxed CLI would see
> a different directory. Since Tome ships via DMG and Homebrew rather than the Mac App Store, App
> Sandbox is not required, and Developer ID signing + notarization + hardened runtime is the
> correct posture. The authoritative entitlements file is in
> [`05-phase-5-polish-launch.md` P5-010](./05-phase-5-polish-launch.md#p5-010-macos-notarization-setup).
>
> This is a real reduction in defence-in-depth and it is worth naming rather than glossing.
> Compensating controls: minimal hardened-runtime exceptions, untrusted documentation rendered in a
> scripted-disabled iframe with a strict CSP, sanitization at ingest, no network server by default.

See [12-security-considerations.md](./12-security-considerations.md) for detailed security design.

---

## Accessibility Requirements

### Baseline Compliance

**Target:** WCAG 2.1 Level AA for contrast and keyboard; Level A elsewhere at v1.0.

> The original target was "Level A equivalent" while simultaneously requiring 4.5:1 contrast, which
> is a **AA** criterion (Level A only requires 3:1 for large text). Stating AA for the two things
> that are cheap to get right and honest about the rest is better than claiming a level the product
> does not meet. Note that the design system as originally written **failed its own 4.5:1 rule** —
> `--color-text-tertiary` measured 3.1:1 in light mode and 3.4:1 in dark. Fixed in
> [`15-design-system.md`](./15-design-system.md).

| Requirement | Implementation | Verified by |
|-------------|----------------|-------------|
| **Keyboard navigation** | Every feature reachable without a pointer; no keyboard traps | Tier A tests query by role/name |
| **Focus indicators** | Visible `:focus-visible` on every interactive element | Design system + review |
| **Color contrast** | ≥ 4.5:1 body text, ≥ 3:1 UI components/borders, both themes | **Automated contrast check over the token set in CI** |
| **Screen reader** | VoiceOver: all controls named, reader content navigable by heading | Manual pass per release |
| **Reduced motion** | `prefers-reduced-motion` respected, including smooth scroll | Design system |
| **Text scaling** | Reader honours the font-size preference without clipping or overlap | Tier A snapshot at min/max sizes |

**The reader is the accessibility surface that matters most**, and it renders third-party HTML.
Sanitization must therefore *preserve* semantics rather than flatten it: heading hierarchy, `id`
anchors, list structure, table headers, `alt` text, and `lang` attributes all have to survive. A
sanitizer that strips `alt` and `id` produces a document that is both inaccessible and
un-navigable — which is exactly what the original allowlist did.

### Keyboard Navigation

All primary actions must be keyboard accessible:

| Action | Shortcut |
|--------|----------|
| Global search | Cmd+K |
| In-page search | Cmd+F |
| Navigate back | Cmd+[ |
| Navigate forward | Cmd+] |
| Bookmark | Cmd+D |
| Toggle sidebars | Cmd+1, Cmd+2 |
| Preferences | Cmd+, |

### VoiceOver Support

- Semantic HTML in rendered documentation
- ARIA labels on interactive controls
- Heading hierarchy in reader view
- Live regions for search results

### Future Accessibility (Post-v1.0)

- WCAG 2.1 AA compliance
- High contrast mode
- Custom font size beyond current range
- Full VoiceOver optimization

---

## Internationalization Requirements

### i18n Readiness

**Scope:** Architecture supports i18n from v1.0; English only at launch

| Requirement | Implementation |
|-------------|----------------|
| **String externalization** | All UI strings in resource files |
| **Format** | ICU MessageFormat for plurals/variables |
| **RTL preparation** | Layout system supports RTL (not enabled) |
| **Date/time** | Use system locale formatting |
| **Number formatting** | Use system locale formatting |

### String Management

```typescript
// src/lib/i18n/en.json
{
  "search.placeholder": "Search documentation...",
  "search.noResults": "No results for \"{query}\"",
  "search.resultCount": "{count, plural, =0 {No results} =1 {1 result} other {# results}}",
  "bookmark.added": "Page bookmarked",
  "bookmark.removed": "Bookmark removed"
}
```

```typescript
// Usage
import { t } from '$lib/i18n';

const message = t('search.resultCount', { count: results.length });
```

### Locale Detection

```typescript
// Use system locale
const locale = navigator.language || 'en-US';
```

### Future Localization (Post-v1.0)

Priority languages for future localization:
1. Japanese (significant developer population)
2. Chinese (Simplified)
3. German
4. Spanish
5. French

---

## Usability Requirements

### Learnability

- First-time users should add a doc source within 2 minutes
- Core features discoverable without documentation
- Keyboard shortcuts shown in menus and tooltips

### Efficiency

- Common tasks (search, navigate) require < 3 actions
- Keyboard-first design for power users
- Recent items and history for quick access

### Error Prevention

- Confirm before destructive actions (delete source, remove bookmark)
- Undo support where feasible
- Clear validation messages

### Consistency

- Follow macOS Human Interface Guidelines
- Consistent keyboard shortcuts with system conventions
- Familiar patterns (Cmd+K for search, Cmd+, for preferences)

---

## Compatibility Requirements

### File Format Compatibility

| Format | Requirement |
|--------|-------------|
| **HTML5** | Full support for modern HTML |
| **UTF-8** | All text handling in UTF-8 |
| **YAML** | Config files in YAML 1.2 |
| **JSON** | API responses in JSON |
| **SQLite** | SQLite 3.x for database |

### Documentation Platform Compatibility

| Platform | Version Support |
|----------|-----------------|
| **Sphinx/ReadTheDocs** | Sphinx 3.x, 4.x, 5.x, 6.x, 7.x |
| **rustdoc** | Rust 1.60+ output format |
| **mdBook** | mdBook 0.4+ |
| **Man pages** | mandoc and groff formats |

### API Compatibility

- Local HTTP API follows semantic versioning
- MCP protocol version compatibility documented
- Deprecation warnings for breaking changes

---

## Maintainability Requirements

### Code Quality

| Metric | Target |
|--------|--------|
| Test coverage | 90% minimum |
| Lint warnings | 0 (CI enforced) |
| Documentation | Public APIs documented |
| Type safety | Strict TypeScript, Rust strong typing |

### Dependency Management

- Pin major versions of critical dependencies
- Monthly dependency update reviews
- No dependencies with known vulnerabilities
- Prefer widely-used, well-maintained libraries

### Modularity

- Clear separation between layers (UI, core, storage)
- Scrapers as independent modules
- Plugin architecture for future extensibility

---

## Scalability Requirements

### Documentation Volume

| Scenario | Performance Target |
|----------|-------------------|
| 10 sources, 5K pages | Baseline (all targets met) |
| 50 sources, 50K pages | < 20% performance degradation |
| 100 sources, 100K pages | Usable with lazy loading |

### Sync Volume

| Scenario | Target |
|----------|--------|
| 100 bookmarks | Sync < 2s |
| 1,000 bookmarks | Sync < 10s |
| 10,000 bookmarks | Sync < 60s |

---

## Compliance Requirements

### Software Distribution

| Requirement | Implementation |
|-------------|----------------|
| **Code signing** | Developer ID certificate |
| **Notarization** | Apple notarization required |
| **Gatekeeper** | Must pass Gatekeeper checks |

### Open Source

| Requirement | Implementation |
|-------------|----------------|
| **License** | **Unresolved — DEC-001.** No `LICENSE` file exists, while the Phase 5 landing-page copy asserted MIT. Nothing should be published until this is settled; "MIT or Apache 2.0 (TBD)" is not a licence. |
| **Third-party attribution** | Dependency licences collected and shipped (`cargo about` / `license-checker`) — a release blocker, not a nicety |
| **Documentation attribution** | Every rendered page links to its origin and displays the upstream licence when determinable. This is what keeps a local cache a cache. See RISK-011. |
| **Contributor agreement** | DCO sign-off |

---

## Environmental Requirements

### Resource Efficiency

- Minimize CPU usage when idle (< 1%)
- **No background network activity that the user has not configured.** Precisely: no telemetry, no
  update checks, and no unsolicited fetching. Scheduled and `watch` sync *are* background network
  activity — they are permitted because the user opted into them per source, they are visible in
  the UI, and they are rate-limited (see [PRD § Sync Strategy](../PRD.md#6-sync-strategy-configuration)).
  The blanket phrasing "no background network activity without user action" directly contradicted
  the entire sync-strategy feature; this is what was actually meant.
- Sync is deferred on battery below a threshold and on metered/personal-hotspot connections
- Respect system power management; support App Nap when not in foreground
- Long crawls yield to foreground work and are cancellable

### Storage Efficiency

- Deduplicate common assets across sources
- Compress stored content where beneficial
- Provide cache clearing option
- Warn when storage exceeds configurable threshold
