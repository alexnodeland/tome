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

- Apple Silicon provides unified memory architecture beneficial for search indexing
- Simplifies testing matrix and reduces maintenance burden
- Intel Macs are being phased out; new users predominantly on Apple Silicon
- Native ARM64 binaries for optimal performance

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

### Memory Usage

| Metric | Target | Measurement |
|--------|--------|-------------|
| Idle memory | < 200MB | App open, no activity |
| Active browsing | < 400MB | Normal usage |
| Peak during index | < 1GB | During full reindex |
| Large doc set (100K) | < 500MB | Idle with large index |

### Network Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Scrape rate | 5 req/sec | Default, configurable |
| Page fetch timeout | 30s | Per page |
| Crawl timeout | 10 min | Per source |

---

## Reliability Requirements

### Availability

| Metric | Target |
|--------|--------|
| Crash-free sessions | > 99.9% |
| Data corruption incidents | 0 |
| Sync success rate | > 99.5% |

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
| **No telemetry** | Zero data collection or phone-home |
| **No analytics** | No usage tracking |
| **Local-first** | All data stored locally by default |
| **iCloud optional** | Sync is opt-in, not required |

### Network Security

| Requirement | Implementation |
|-------------|----------------|
| **HTTPS only** | Reject HTTP URLs (auto-upgrade) |
| **Certificate validation** | Standard TLS verification |
| **No tracking cookies** | Stateless scraping |

### Local Security

| Requirement | Implementation |
|-------------|----------------|
| **Sandboxed** | App Sandbox enabled |
| **Hardened runtime** | Required for notarization |
| **No elevated privileges** | Standard user permissions only |
| **API localhost only** | HTTP API bound to 127.0.0.1 |

See [12-security-considerations.md](./12-security-considerations.md) for detailed security design.

---

## Accessibility Requirements

### Baseline Compliance

**Target:** Basic accessibility compliance (WCAG 2.1 Level A equivalent)

| Requirement | Implementation |
|-------------|----------------|
| **Keyboard navigation** | All features accessible via keyboard |
| **Focus indicators** | Visible focus states on all interactive elements |
| **Color contrast** | Minimum 4.5:1 for text |
| **Screen reader** | VoiceOver basic support |
| **Reduced motion** | Respect `prefers-reduced-motion` |

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
| **License** | MIT or Apache 2.0 (TBD) |
| **Attribution** | Third-party licenses documented |
| **Contributor agreement** | DCO sign-off |

---

## Environmental Requirements

### Resource Efficiency

- Minimize CPU usage when idle (< 1%)
- No background network activity without user action
- Respect system power management
- Support App Nap when not in foreground

### Storage Efficiency

- Deduplicate common assets across sources
- Compress stored content where beneficial
- Provide cache clearing option
- Warn when storage exceeds configurable threshold
