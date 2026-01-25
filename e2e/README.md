# E2E Tests Directory

Playwright end-to-end tests for critical user journeys.

## What Belongs Here

- **Test specifications** (`.spec.ts` files)
- **Test fixtures** (helpers, test data)
- **Page objects** for reusable interactions
- **Mock servers** for documentation sources

## What Does NOT Belong Here

- Unit tests (use adjacent `.test.ts` files)
- Integration tests (use `tests/` directories)
- Application code

## Directory Structure

```
e2e/
├── fixtures/
│   ├── app.ts              # App launch helper
│   ├── mock-server.ts      # Mock documentation server
│   └── test-docs/          # Sample documentation
│       ├── sphinx-example/
│       ├── rustdoc-example/
│       └── mdbook-example/
├── pages/
│   ├── library.ts          # Library page object
│   ├── reader.ts           # Reader page object
│   └── search.ts           # Search page object
├── add-source.spec.ts
├── search.spec.ts
├── bookmarks.spec.ts
├── navigation.spec.ts
└── sync.spec.ts
```

## Test Specification Pattern

```typescript
// search.spec.ts
import { test, expect } from '@playwright/test';
import { launchApp, loadTestDocs } from './fixtures/app';
import { SearchPage } from './pages/search';

test.describe('Search', () => {
  test.beforeEach(async ({ page }) => {
    await launchApp(page);
    await loadTestDocs(page);
  });

  test('should find results with global search', async ({ page }) => {
    const search = new SearchPage(page);

    // Open search with keyboard
    await page.keyboard.press('Meta+k');
    await expect(search.input).toBeFocused();

    // Type query
    await search.input.fill('iterator');

    // Wait for results
    await expect(search.results).toBeVisible();
    await expect(search.resultItems).toHaveCount.greaterThan(0);

    // Select first result
    await search.resultItems.first().click();

    // Verify navigation
    await expect(page.locator('h1')).toContainText(/iterator/i);
  });

  test('should show no results state', async ({ page }) => {
    const search = new SearchPage(page);

    await page.keyboard.press('Meta+k');
    await search.input.fill('xyznonexistent123');

    await expect(search.emptyState).toBeVisible();
    await expect(search.emptyState).toContainText('No results');
  });

  test('should filter by source', async ({ page }) => {
    const search = new SearchPage(page);

    await page.keyboard.press('Meta+k');
    await search.input.fill('function');

    // Open filter
    await search.filterButton.click();
    await search.sourceFilter('rust-std').click();

    // All results should be from rust-std
    const results = await search.getResults();
    for (const result of results) {
      expect(result.sourceId).toBe('rust-std');
    }
  });
});
```

## Page Object Pattern

```typescript
// pages/search.ts
import { Locator, Page } from '@playwright/test';

export class SearchPage {
  readonly page: Page;
  readonly input: Locator;
  readonly results: Locator;
  readonly resultItems: Locator;
  readonly emptyState: Locator;
  readonly filterButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.input = page.getByRole('searchbox');
    this.results = page.getByTestId('search-results');
    this.resultItems = page.getByTestId('search-result');
    this.emptyState = page.getByTestId('search-empty');
    this.filterButton = page.getByTestId('search-filter-button');
  }

  async open() {
    await this.page.keyboard.press('Meta+k');
  }

  async search(query: string) {
    await this.open();
    await this.input.fill(query);
    await this.results.waitFor({ state: 'visible' });
  }

  async selectResult(index: number) {
    await this.resultItems.nth(index).click();
  }

  sourceFilter(sourceId: string) {
    return this.page.getByTestId(`source-filter-${sourceId}`);
  }

  async getResults(): Promise<SearchResult[]> {
    const items = await this.resultItems.all();
    const results: SearchResult[] = [];

    for (const item of items) {
      results.push({
        title: await item.getByTestId('result-title').textContent() ?? '',
        sourceId: await item.getAttribute('data-source-id') ?? '',
      });
    }

    return results;
  }
}

interface SearchResult {
  title: string;
  sourceId: string;
}
```

## Fixture Pattern

```typescript
// fixtures/app.ts
import { Page } from '@playwright/test';
import { MockDocServer } from './mock-server';

let mockServer: MockDocServer | null = null;

export async function launchApp(page: Page) {
  // For Tauri apps, we need special setup
  // This might involve launching the app process
  // For now, assume we're testing the web view directly
  await page.goto('tauri://localhost');

  // Wait for app to be ready
  await page.waitForSelector('[data-testid="app-ready"]');
}

export async function loadTestDocs(page: Page) {
  // Start mock server if not running
  if (!mockServer) {
    mockServer = await MockDocServer.start();
  }

  // Add test sources via app
  await addTestSource(page, 'rust-std', mockServer.rustdocUrl);
  await addTestSource(page, 'python-docs', mockServer.sphinxUrl);

  // Wait for sync
  await page.waitForSelector('[data-testid="sync-complete"]');
}

async function addTestSource(page: Page, name: string, url: string) {
  // Open add source dialog
  await page.keyboard.press('Meta+n');

  // Fill form
  await page.getByPlaceholder('Source URL').fill(url);
  await page.getByPlaceholder('Name').fill(name);

  // Submit
  await page.getByRole('button', { name: 'Add' }).click();
}

export async function cleanup() {
  if (mockServer) {
    await mockServer.stop();
    mockServer = null;
  }
}
```

## Mock Server Pattern

```typescript
// fixtures/mock-server.ts
import { createServer, Server } from 'http';
import * as fs from 'fs';
import * as path from 'path';

export class MockDocServer {
  private server: Server;
  readonly port: number;

  private constructor(server: Server, port: number) {
    this.server = server;
    this.port = port;
  }

  static async start(): Promise<MockDocServer> {
    return new Promise((resolve) => {
      const server = createServer((req, res) => {
        const filePath = path.join(
          __dirname,
          'test-docs',
          req.url ?? '/index.html'
        );

        if (fs.existsSync(filePath)) {
          const content = fs.readFileSync(filePath, 'utf-8');
          res.writeHead(200, { 'Content-Type': 'text/html' });
          res.end(content);
        } else {
          res.writeHead(404);
          res.end('Not found');
        }
      });

      server.listen(0, () => {
        const address = server.address();
        const port = typeof address === 'object' ? address?.port ?? 0 : 0;
        resolve(new MockDocServer(server, port));
      });
    });
  }

  get rustdocUrl(): string {
    return `http://localhost:${this.port}/rustdoc-example/`;
  }

  get sphinxUrl(): string {
    return `http://localhost:${this.port}/sphinx-example/`;
  }

  async stop(): Promise<void> {
    return new Promise((resolve) => {
      this.server.close(() => resolve());
    });
  }
}
```

## Running Tests

```bash
# Run all E2E tests
npx playwright test

# Run specific test file
npx playwright test search.spec.ts

# Run specific test
npx playwright test -g "should find results"

# Run in headed mode (see browser)
npx playwright test --headed

# Run with debug mode (pause on failure)
npx playwright test --debug

# Generate HTML report
npx playwright show-report
```

## Architectural Rules

1. E2E tests focus on **critical user journeys** only
2. Use **page objects** for reusable interactions
3. Use **mock servers** for documentation sources
4. Keep tests **independent** (no shared state between tests)
5. Use **semantic locators** (getByRole, getByText) over CSS selectors
6. Tests must be **deterministic** (no flaky tests)
