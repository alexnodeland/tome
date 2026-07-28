# Testing Strategy

**Coverage Target:** per-module (see [Coverage Requirements](#coverage-requirements))
**Frameworks:** Rust built-in tests + `criterion`, **Vitest** (JS/Svelte), XCUITest smoke suite (macOS UI)

> **Two framework choices changed, both because the originals could not run.**
>
> 1. **Playwright cannot drive a Tauri app on macOS.** Tauri exposes automation through WebDriver
>    (`tauri-driver`), and `tauri-driver` supports **Linux and Windows only** — macOS `WKWebView`
>    has no WebDriver implementation. The original CI job ran `playwright install webkit`, which
>    installs Playwright's *own bundled* browser; every "E2E" spec would have exercised a browser
>    that is not Tome, and reported green. See [End-to-End Testing](#end-to-end-testing).
> 2. **Jest cannot compile `.svelte` under the specified config.** `preset: 'ts-jest'` has no
>    Svelte transform, and the project is Vite-based. Vitest reuses Vite's pipeline, so Svelte
>    components compile with no extra configuration.
>
> Neither would have surfaced until someone ran the suite — at which point the testing plan gets
> rewritten under deadline pressure, which is the expensive time to discover it.

---

## Testing Philosophy

1. **Test behavior, not implementation** - Tests should verify what the code does, not how
2. **Fast feedback loops** - Unit tests run in < 10s, full suite in < 5 minutes
3. **Confidence over coverage** - 90% coverage is a floor, not a ceiling; focus on critical paths
4. **Tests as documentation** - Test names should describe expected behavior clearly

---

## Test Pyramid

```
                  ┌─────────────┐
                  │ UI smoke    │  ~2%   XCUITest, release candidates only
                  │ (Tier C)    │        Only what needs a real app bundle
                  ├─────────────┤
                  │ Backend E2E │  ~8%   Rust integration tests, headless
                  │ (Tier B)    │        Full pipeline against fixture servers
                  ├─────────────┤
                  │ Frontend    │  ~15%  Vitest + jsdom, real components,
                  │ integration │        stubbed IPC. The workhorse.
                  │ (Tier A)    │
                  ├─────────────┤
                  │    Unit     │  ~75%  Rust + TS, individual functions
                  └─────────────┘
```

---

## Unit Testing

### Rust (Built-in Test Framework)

**Location:** Tests live alongside code in `#[cfg(test)]` modules

**Structure:**
```
src-tauri/
├── src/
│   ├── scraper/
│   │   ├── mod.rs
│   │   ├── generic.rs
│   │   └── tests.rs          # Unit tests for scraper
│   ├── search/
│   │   ├── mod.rs
│   │   ├── engine.rs
│   │   └── tests.rs          # Unit tests for search
│   └── lib.rs
└── tests/                    # Integration tests
    ├── scraper_integration.rs
    └── search_integration.rs
```

**Conventions:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Test naming: test_<function>_<scenario>_<expected>
    #[test]
    fn test_parse_url_valid_https_returns_parsed() {
        let result = parse_url("https://example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_url_invalid_scheme_returns_error() {
        let result = parse_url("ftp://example.com");
        assert!(matches!(result, Err(ParseError::InvalidScheme(_))));
    }

    // Use test fixtures
    #[test]
    fn test_normalize_html_sphinx_structure_preserved() {
        let html = include_str!("fixtures/sphinx_page.html");
        let result = normalize_html(html);
        assert!(result.contains("<h1>"));
    }
}
```

**Async Tests:**
```rust
#[tokio::test]
async fn test_fetch_page_success() {
    let server = MockServer::start().await;
    server.mock(|when, then| {
        when.path("/docs");
        then.status(200).body("<html>...</html>");
    });

    let scraper = Scraper::new();
    let result = scraper.fetch(&server.url("/docs")).await;
    assert!(result.is_ok());
}
```

**Coverage:** Run with `cargo tarpaulin` or `cargo llvm-cov`

```bash
# Generate coverage report
cargo llvm-cov --html --output-dir coverage/rust
```

### JavaScript/TypeScript (Vitest)

**Location:** Test files adjacent to source with `.test.ts` suffix

**Structure:**
```
src/
├── lib/
│   ├── stores/
│   │   ├── bookmarks.ts
│   │   └── bookmarks.test.ts
│   ├── utils/
│   │   ├── highlight.ts
│   │   └── highlight.test.ts
│   └── components/
│       ├── SearchResults.svelte
│       └── SearchResults.test.ts
└── jest.config.js
```

**Conventions:**
```typescript
// bookmarks.test.ts
import { describe, it, expect, vi } from 'vitest';
import { createBookmark, deleteBookmark } from './bookmarks';

describe('Bookmarks Store', () => {
  describe('createBookmark', () => {
    it('should create a bookmark with required fields', () => {
      const bookmark = createBookmark({
        sourceId: 'abc',
        pagePath: '/docs/intro',
        title: 'Introduction',
      });

      expect(bookmark.id).toBeDefined();
      expect(bookmark.sourceId).toBe('abc');
      expect(bookmark.createdAt).toBeInstanceOf(Date);
    });

    it('should throw if sourceId is missing', () => {
      expect(() => createBookmark({ pagePath: '/docs', title: 'Test' }))
        .toThrow('sourceId is required');
    });
  });
});
```

**Svelte Component Testing:**
```typescript
// SearchResults.test.ts
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import SearchResults from './SearchResults.svelte';

describe('SearchResults', () => {
  it('should display results when provided', () => {
    const results = [
      { title: 'Vec', snippet: 'A growable array...' },
    ];

    render(SearchResults, { props: { results, loading: false } });

    expect(screen.getByText('Vec')).toBeInTheDocument();
  });

  it('should show loading state', () => {
    render(SearchResults, { props: { results: [], loading: true } });

    expect(screen.getByText('Searching...')).toBeInTheDocument();
  });

  it('should call onSelect when result clicked', async () => {
    const onSelect = vi.fn();
    const results = [{ id: '1', title: 'Test' }];

    render(SearchResults, { props: { results, onSelect } });
    await fireEvent.click(screen.getByText('Test'));

    expect(onSelect).toHaveBeenCalledWith(results[0]);
  });
});
```

**Vitest Configuration:**
```typescript
// vite.config.ts (test section) -- reuses the app's own transform pipeline,
// which is why Svelte components compile without extra plugins.
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],   // @testing-library/jest-dom/vitest
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,svelte}'],
      exclude: ['src/**/*.d.ts', 'src/test/**'],
      // Per-module thresholds -- see Coverage Requirements. A single global
      // number is the wrong shape: UI components and data-integrity code do
      // not deserve the same bar.
      thresholds: {
        'src/lib/stores/**':     { lines: 90, branches: 85 },
        'src/lib/utils/**':      { lines: 90, branches: 85 },
        'src/lib/components/**': { lines: 80, branches: 70 },
      },
    },
  },
});
```

**Mocking the Tauri IPC boundary.** Frontend tests must not reach a real backend. Stub
`@tauri-apps/api` once, centrally, and assert on the commands invoked -- this is the seam that
makes the frontend testable at all, and the original plan never mentioned it:

```typescript
// src/test/setup.ts
import { vi } from 'vitest';
export const invoked: Array<[string, unknown]> = [];
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string, args: unknown) => {
    invoked.push([cmd, args]);
    return mockResponses[cmd] ?? null;
  }),
}));
```

---

## Integration Testing

### Rust Integration Tests

**Location:** `src-tauri/tests/`

**Purpose:** Test interactions between modules (scraper + storage, search + index)

```rust
// tests/scraper_storage_integration.rs
use tome::{scraper::Scraper, storage::PageRepository};
use tempfile::TempDir;

#[tokio::test]
async fn test_scrape_and_store_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let repo = PageRepository::new(temp_dir.path()).await.unwrap();
    let scraper = Scraper::new();

    // Scrape mock server
    let server = MockServer::start().await;
    server.mock_sphinx_site();

    let pages = scraper.crawl(&server.url("/"), Default::default()).await.unwrap();

    // Store pages
    repo.upsert_pages(&pages).await.unwrap();

    // Verify
    let stored = repo.list_pages().await.unwrap();
    assert_eq!(stored.len(), pages.len());
}
```

### Tauri Command Integration Tests

Test the IPC layer between Rust and JavaScript:

```rust
// tests/tauri_commands.rs
use tauri::test::{mock_builder, MockRuntime};

#[test]
fn test_search_command() {
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![search])
        .build()
        .unwrap();

    let result: Vec<SearchResult> = tauri::test::invoke(
        &app,
        "search",
        json!({ "query": "iterator", "limit": 10 }),
    ).unwrap();

    assert!(!result.is_empty());
}
```

---

## End-to-End Testing

### Why there is no Playwright tier

The original plan specified Playwright specs driving the real app, and a CI job running
`npx playwright install --with-deps webkit` followed by `npx playwright test`. This cannot work:

- **Tauri automation goes through WebDriver**, via `tauri-driver`.
- **`tauri-driver` supports Linux and Windows only.** macOS has no WebDriver implementation for
  `WKWebView`, and Apple provides none.
- `playwright install webkit` installs *Playwright's* WebKit build. Specs written against it would
  have driven a stock browser, not Tome — passing tests, zero coverage of the product.

Since Tome is macOS-only (`09-non-functional-requirements.md`), there is no platform on which the
original E2E tier could ever have run. It is replaced by three tiers that do run, arranged so that
the cheap ones carry most of the weight.

### Tier A — Frontend integration (Vitest + jsdom), the workhorse

Renders the **real** Svelte components with a stubbed IPC layer. This covers nearly everything the
Playwright specs were written to cover — keyboard handling, search results, bookmark toggling,
navigation state, error states — at a fraction of the runtime and with no flake.

```typescript
// src/routes/__tests__/search.integration.test.ts
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { invoked } from '../../test/setup';
import App from '../App.svelte';

it('Cmd+K opens search, typing queries the backend, Enter navigates', async () => {
  const user = userEvent.setup();
  render(App);

  await user.keyboard('{Meta>}k{/Meta}');
  expect(screen.getByRole('searchbox')).toHaveFocus();

  await user.type(screen.getByRole('searchbox'), 'async iterator');
  await screen.findByRole('option', { name: /AsyncIterator/ });

  // Assert on the contract with the backend, not on backend behaviour.
  expect(invoked).toContainEqual(['search', { query: 'async iterator', limit: 10 }]);

  await user.keyboard('{Enter}');
  expect(await screen.findByRole('heading', { level: 1 })).toHaveTextContent(/AsyncIterator/);
});
```

**Deliberately assert accessibility here**, using role- and name-based queries. A test that can
only find an element by `data-testid` is telling you the element has no accessible name — which is
a real defect given the accessibility commitments in `09-non-functional-requirements.md`.

### Tier B — Backend end-to-end (Rust integration tests)

The whole pipeline, headless, against a local fixture server: add a source → crawl → normalize →
sanitize → localize assets → index → search → render. This is where the genuinely risky logic
lives, and it is fully testable without a UI.

```rust
#[tokio::test]
async fn add_source_to_search_result() {
    let tmp = TempDir::new().unwrap();
    let server = fixture_server("sphinx-example").await;   // serves committed fixtures
    let app = TestApp::new(tmp.path()).await;

    app.add_source(source_config(&server.url())).await.unwrap();
    app.pull("test-docs").await.unwrap();

    let hits = app.search("iterator", None, 10).await.unwrap();
    assert!(hits.results.iter().any(|r| r.title.contains("Iterator")));

    // Offline guarantee: shut the server down, then render.
    server.shutdown().await;
    let page = app.render_page("test-docs", "/api/reference.html").await.unwrap();
    assert!(!page.html.contains("http://"), "rendered page must not reference remote resources");
}
```

Also covers, with no UI involved: sync convergence (simulated multi-device), migration up/down,
crash-recovery at each await point, and the SSRF/sanitizer corpora.

### Tier C — macOS UI smoke suite (XCUITest), thin and few

A handful of tests for what genuinely requires the real app bundle, and nothing more:

| Test | Why it needs a real bundle |
|------|----------------------------|
| App launches and shows the library | Bundle, entitlements, code signature |
| Global search opens via `Cmd+K` | Real key event delivery through AppKit |
| Menu bar item appears and opens | `NSStatusItem` |
| Deep link / file association opens the app | System integration |
| Notarized build passes Gatekeeper | `spctl --assess` on the signed artefact |

Keep this tier small on purpose. UI automation is the slowest and flakiest thing in any suite, and
every assertion that *can* live in Tier A should.

### What runs where

| Tier | Runtime | Where | Blocking |
|------|---------|-------|----------|
| Rust unit | < 30 s | Every push | Yes |
| Vitest unit + Tier A | < 60 s | Every push | Yes |
| Rust integration (Tier B) | < 3 min | Every push | Yes |
| Benchmarks | < 5 min | Nightly + on demand | No (alerts on regression) |
| Tier C smoke | ~2 min | Release candidates only | Yes, for release |

## Test Data & Fixtures

### Documentation Fixtures

Create a set of test documentation sources:

```
e2e/fixtures/test-docs/
├── sphinx-example/          # Minimal Sphinx site
│   ├── searchindex.js
│   ├── index.html
│   └── api/
│       └── reference.html
├── rustdoc-example/         # Minimal rustdoc output
│   ├── search-index.js
│   └── test_crate/
│       └── index.html
└── mdbook-example/          # Minimal mdBook
    ├── book.toml
    ├── SUMMARY.md
    └── src/
        └── chapter1.md
```

### Mock Servers

```typescript
// e2e/fixtures/mock-server.ts
export async function mockDocServer(type: 'sphinx' | 'rustdoc' | 'mdbook') {
  const server = await createServer();

  switch (type) {
    case 'sphinx':
      server.use(
        rest.get('/searchindex.js', (req, res, ctx) =>
          res(ctx.body(readFixture('sphinx-example/searchindex.js')))
        ),
        rest.get('/*', (req, res, ctx) =>
          res(ctx.body(readFixture(`sphinx-example${req.url.pathname}`)))
        ),
      );
      break;
    // ... other types
  }

  return server;
}
```

---

## Performance Testing

### Benchmarks (Rust)

```rust
// benches/search_benchmark.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn search_benchmark(c: &mut Criterion) {
    let engine = setup_test_engine(10_000); // 10K pages

    let mut group = c.benchmark_group("search");

    for query in ["iterator", "async function", "Vec::new"] {
        group.bench_with_input(
            BenchmarkId::new("simple_query", query),
            query,
            |b, q| b.iter(|| engine.search(q, 10)),
        );
    }

    group.finish();
}

fn index_benchmark(c: &mut Criterion) {
    let pages = generate_test_pages(1000);

    c.bench_function("index_1000_pages", |b| {
        b.iter(|| {
            let engine = SearchEngine::new_temp().unwrap();
            for page in &pages {
                engine.index_page(page).unwrap();
            }
            engine.commit().unwrap();
        })
    });
}

criterion_group!(benches, search_benchmark, index_benchmark);
criterion_main!(benches);
```

### Where performance is *not* asserted

Performance assertions do not belong in UI tests. A wall-clock threshold measured through jsdom or
a UI driver is dominated by test-harness overhead and machine noise, so it flakes under load and
tells you nothing when it fails. The original plan asserted `duration < 200ms` inside an E2E
spec — a guaranteed source of intermittent red builds that teams learn to ignore, which is worse
than having no assertion.

Latency is measured by `criterion` benchmarks against a fixed corpus (above), with regression
alerts comparing to a stored baseline. The UI tier asserts *behaviour* — that a loading state
appears, that results render, that focus moves — never timings.

### Property and fuzz testing

The plan had none, and three components in Tome are exactly the shape that rewards them:

| Target | Technique | What it catches |
|--------|-----------|-----------------|
| HTML parser / normalizer | `cargo-fuzz` over a real-page corpus | Panics on malformed input — the plan's "zero panics on any input" criterion is otherwise unverified |
| Sanitizer | Fuzz + an XSS payload corpus | Bypasses; anchor/`id` preservation regressions |
| Sync convergence | `proptest`: random op sets, random permutations, assert identical final state | The bugs that lose user data, which example-based tests miss by construction |
| Annotation re-anchoring | `proptest`: random content edits, assert anchors resolve or become `orphaned` | Silent drift onto the wrong text |
| URL/SSRF filter | Table-driven with an adversarial vector list | Encoding tricks, redirect chains |

---

## Coverage Requirements

### By Module

| Module | Minimum Coverage | Rationale |
|--------|-----------------|-----------|
| `sanitize/*`, `sync/*` | 95% | A defect here loses or corrupts user data, or lets script through |
| `search/*` | 95% | Core functionality |
| `scraper/*` | 90% | Critical data ingestion |
| `storage/*` | 90% | Data integrity |
| `api/*` | 85% | External interface |
| `ui/stores/*` | 90% | State management |
| `ui/components/*` | 80% | UI components |
| `cli/*` | 70% | Thin shell over tested library code |

### Coverage Enforcement

**The global gate and the per-module table contradicted each other.** CI enforced
`--fail-under 90` globally while the table permitted 80–85% for UI and API, and the philosophy
section says "confidence over coverage" — three positions in one document. Per-module thresholds
are the ones that mean something; the global floor is set below the weakest module so it catches
collapse rather than fighting the table.

```yaml
# Global floor: catches a collapse, does not contradict the per-module table.
- name: Rust coverage floor
  run: cargo llvm-cov --fail-under 80

# Per-module thresholds: the gates that actually encode intent.
- name: Rust coverage per module
  run: ./scripts/check-coverage.sh   # reads the table above; fails naming the module

# Diff coverage: new code is held to a higher bar than the legacy average,
# which is what actually moves a codebase in the right direction.
- name: Diff coverage
  run: ./scripts/diff-coverage.sh --min 90
```

**Coverage is a floor, not a goal.** A 95% figure on `sync/*` means nothing without the
convergence property tests and fault injection described in Phase 3 — those find the bugs that
matter, and they can pass with far less line coverage than a suite of shallow tests that touches
every line and asserts nothing.

---

## Test Execution

### Local Development

```bash
# Unit + integration
cargo test --workspace
npm run test              # vitest

# Coverage
cargo llvm-cov
npm run test:coverage

# Backend end-to-end (Tier B)
cargo test --test '*' -- --include-ignored

# Benchmarks
cargo bench

# macOS UI smoke (Tier C) -- requires a built .app
xcodebuild test -scheme TomeUITests

# A specific test
cargo test scraper::tests
npm run test -- bookmarks
```

### CI Pipeline

See [10-cicd-devops.md](./10-cicd-devops.md) for full CI configuration.

```yaml
test:
  - cargo test --workspace
  - cargo llvm-cov --fail-under 80
  - ./scripts/check-coverage.sh
  - npm run test:coverage
  # Tier C runs only on release candidates -- see "What runs where".
```

---

## Test Maintenance

### Flaky Test Policy

1. Flaky tests are bugs - fix immediately or skip with tracking issue
2. Use `test.skip` with issue link, never silent `skip`
3. Review skipped tests weekly

### Test Review Checklist

- [ ] Test name clearly describes behavior
- [ ] Test is independent (no shared state)
- [ ] Test runs in < 1 second (unit) or < 30 seconds (E2E)
- [ ] Assertions are specific (not just "no error")
- [ ] Edge cases covered
- [ ] No hardcoded waits (use proper async handling)
