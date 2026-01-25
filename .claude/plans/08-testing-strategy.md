# Testing Strategy

**Coverage Target:** 90%
**Frameworks:** Rust built-in tests, Jest (JS), Playwright (E2E)

---

## Testing Philosophy

1. **Test behavior, not implementation** - Tests should verify what the code does, not how
2. **Fast feedback loops** - Unit tests run in < 10s, full suite in < 5 minutes
3. **Confidence over coverage** - 90% coverage is a floor, not a ceiling; focus on critical paths
4. **Tests as documentation** - Test names should describe expected behavior clearly

---

## Test Pyramid

```
                    ┌───────────┐
                    │    E2E    │  ~5% of tests
                    │ Playwright│  Critical user journeys
                    ├───────────┤
                    │Integration│  ~15% of tests
                    │  Tests    │  Component interactions
                    ├───────────┤
                    │   Unit    │  ~80% of tests
                    │   Tests   │  Individual functions/modules
                    └───────────┘
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

### JavaScript/TypeScript (Jest)

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
    const onSelect = jest.fn();
    const results = [{ id: '1', title: 'Test' }];

    render(SearchResults, { props: { results, onSelect } });
    await fireEvent.click(screen.getByText('Test'));

    expect(onSelect).toHaveBeenCalledWith(results[0]);
  });
});
```

**Jest Configuration:**
```javascript
// jest.config.js
module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'jsdom',
  moduleNameMapper: {
    '^\\$lib/(.*)$': '<rootDir>/src/lib/$1',
  },
  setupFilesAfterEnv: ['@testing-library/jest-dom'],
  collectCoverageFrom: [
    'src/**/*.{ts,svelte}',
    '!src/**/*.d.ts',
  ],
  coverageThreshold: {
    global: {
      branches: 90,
      functions: 90,
      lines: 90,
      statements: 90,
    },
  },
};
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

## End-to-End Testing (Playwright)

### Setup

```typescript
// playwright.config.ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 30000,
  retries: process.env.CI ? 2 : 0,
  use: {
    trace: 'on-first-retry',
    video: 'on-first-retry',
  },
  projects: [
    {
      name: 'Tome App',
      use: {
        // Custom Tauri/Electron test setup
      },
    },
  ],
});
```

### Test Structure

```
e2e/
├── fixtures/
│   ├── test-docs/           # Sample doc sources for testing
│   └── app.ts               # App fixture helpers
├── add-source.spec.ts
├── search.spec.ts
├── bookmarks.spec.ts
├── navigation.spec.ts
└── sync.spec.ts
```

### Critical User Journeys

#### 1. Add Documentation Source
```typescript
// e2e/add-source.spec.ts
import { test, expect } from '@playwright/test';
import { launchApp, mockDocServer } from './fixtures/app';

test.describe('Add Documentation Source', () => {
  test('should add a ReadTheDocs source via URL', async () => {
    const app = await launchApp();
    const docServer = await mockDocServer('sphinx');

    // Open add source dialog
    await app.keyboard.press('Meta+n');
    await expect(app.getByRole('dialog')).toBeVisible();

    // Enter URL
    await app.getByPlaceholder('Enter URL').fill(docServer.url);
    await app.getByRole('button', { name: 'Add' }).click();

    // Wait for detection
    await expect(app.getByText('Detected: Sphinx/ReadTheDocs')).toBeVisible();

    // Confirm
    await app.getByRole('button', { name: 'Confirm' }).click();

    // Wait for sync
    await expect(app.getByText('Syncing...')).toBeVisible();
    await expect(app.getByText('Syncing...')).not.toBeVisible({ timeout: 60000 });

    // Verify in library
    await expect(app.getByRole('listitem', { name: /test-docs/ })).toBeVisible();
  });
});
```

#### 2. Search Documentation
```typescript
// e2e/search.spec.ts
test.describe('Search', () => {
  test.beforeEach(async ({ app }) => {
    await app.fixtures.loadTestDocumentation();
  });

  test('should find results with global search', async ({ app }) => {
    // Open search
    await app.keyboard.press('Meta+k');
    await expect(app.getByRole('searchbox')).toBeFocused();

    // Type query
    await app.keyboard.type('async iterator');

    // Wait for results
    await expect(app.getByTestId('search-results')).toBeVisible();
    await expect(app.getByRole('option')).toHaveCount.greaterThan(0);

    // Select result
    await app.keyboard.press('Enter');

    // Verify navigation
    await expect(app.getByRole('heading', { level: 1 })).toContainText(/iterator/i);
  });

  test('should handle no results gracefully', async ({ app }) => {
    await app.keyboard.press('Meta+k');
    await app.keyboard.type('xyznonexistent123');

    await expect(app.getByText('No results')).toBeVisible();
  });
});
```

#### 3. Bookmark Workflow
```typescript
// e2e/bookmarks.spec.ts
test.describe('Bookmarks', () => {
  test('should bookmark current page with Cmd+D', async ({ app }) => {
    await app.fixtures.navigateToPage('rust-std', '/std/vec/struct.Vec.html');

    // Bookmark
    await app.keyboard.press('Meta+d');

    // Verify indicator
    await expect(app.getByTestId('bookmark-indicator')).toHaveAttribute('aria-pressed', 'true');

    // Open bookmarks
    await app.keyboard.press('Meta+b');

    // Verify in list
    await expect(app.getByRole('listitem', { name: /Vec/ })).toBeVisible();
  });

  test('should create highlight with note', async ({ app }) => {
    await app.fixtures.navigateToPage('rust-std', '/std/vec/struct.Vec.html');

    // Select text
    await app.getByText('A contiguous growable array').selectText();

    // Highlight
    await app.keyboard.press('Meta+h');
    await expect(app.getByRole('mark')).toBeVisible();

    // Add note
    await app.getByRole('mark').click();
    await app.getByPlaceholder('Add note').fill('Important concept');
    await app.keyboard.press('Escape');

    // Verify note saved
    await app.getByRole('mark').click();
    await expect(app.getByText('Important concept')).toBeVisible();
  });
});
```

#### 4. Sync Verification
```typescript
// e2e/sync.spec.ts
test.describe('iCloud Sync', () => {
  test.skip(({ browserName }) => browserName !== 'webkit', 'macOS only');

  test('should sync bookmarks between sessions', async ({ app }) => {
    // Create bookmark
    await app.fixtures.navigateToPage('rust-std', '/std/vec/struct.Vec.html');
    await app.keyboard.press('Meta+d');

    // Wait for sync
    await expect(app.getByTestId('sync-status')).toHaveAttribute('data-state', 'synced');

    // Simulate second device (new app instance with same iCloud)
    const app2 = await launchApp({ clearLocalData: true });

    // Wait for sync
    await expect(app2.getByTestId('sync-status')).toHaveAttribute('data-state', 'synced');

    // Verify bookmark exists
    await app2.keyboard.press('Meta+b');
    await expect(app2.getByRole('listitem', { name: /Vec/ })).toBeVisible();
  });
});
```

---

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

### Performance Assertions in E2E

```typescript
test('search should return results within 100ms', async ({ app }) => {
  await app.keyboard.press('Meta+k');

  const start = performance.now();
  await app.keyboard.type('iterator');
  await expect(app.getByTestId('search-results')).toBeVisible();
  const duration = performance.now() - start;

  expect(duration).toBeLessThan(200); // 100ms search + 100ms render
});
```

---

## Coverage Requirements

### By Module

| Module | Minimum Coverage | Rationale |
|--------|-----------------|-----------|
| `scraper/*` | 90% | Critical data ingestion |
| `search/*` | 95% | Core functionality |
| `storage/*` | 90% | Data integrity |
| `sync/*` | 95% | Data sync reliability |
| `api/*` | 85% | External interface |
| `ui/stores/*` | 90% | State management |
| `ui/components/*` | 80% | UI components |

### Coverage Enforcement

```yaml
# In CI, fail if coverage drops below threshold
- name: Check Rust Coverage
  run: |
    cargo llvm-cov --fail-under 90

- name: Check JS Coverage
  run: |
    npm test -- --coverage --coverageThreshold='{"global":{"lines":90}}'
```

---

## Test Execution

### Local Development

```bash
# Run all unit tests
cargo test
npm test

# Run with coverage
cargo llvm-cov
npm test -- --coverage

# Run E2E tests
npx playwright test

# Run specific test file
cargo test scraper::tests
npm test -- bookmarks.test.ts
npx playwright test search.spec.ts
```

### CI Pipeline

See [09-cicd-devops.md](./09-cicd-devops.md) for full CI configuration.

```yaml
test:
  - cargo test --workspace
  - cargo llvm-cov --fail-under 90
  - npm test -- --coverage
  - npx playwright test
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
