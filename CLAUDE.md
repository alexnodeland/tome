# Tome - Developer Guide

> **Single source of truth** for engineers and AI agents working on this codebase.
> Last updated: 2026-01-25

## Quick Reference

### Essential Commands

```bash
# Development
npm run dev                    # Start Tauri dev server with hot reload
npm run build                  # Production build
npm run tauri dev              # Alias for dev

# Testing
cargo test                     # Run Rust unit tests
npm test                       # Run JS/Svelte tests (Jest)
npm run test:e2e               # Run E2E tests (Playwright)
npm run test:all               # Run all tests

# Linting & Formatting
cargo fmt                      # Format Rust code
cargo clippy                   # Lint Rust code
npm run lint                   # ESLint for TypeScript/Svelte
npm run format                 # Prettier format
npm run check                  # Run all checks (format + lint + typecheck)

# Code Generation
npm run gen:component <Name>   # Generate Svelte component with tests
npm run gen:command <name>     # Generate Tauri command with tests
npm run gen:store <name>       # Generate Svelte store with tests

# Utilities
npm run setup                  # First-time setup (installs all deps)
npm run clean                  # Clean build artifacts
```

### Keyboard Shortcuts (Development)

| Action | Shortcut |
|--------|----------|
| Hot reload | Automatic |
| Open DevTools | `Cmd+Option+I` |
| Restart app | `Cmd+R` in terminal |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Swift Shell (AppKit)                      │
│              Menu bar, notifications, global shortcuts       │
├─────────────────────────────────────────────────────────────┤
│                    Svelte UI Layer                           │
│         Library, Reader, Search, Bookmarks, Settings         │
├─────────────────────────────────────────────────────────────┤
│                    Rust Core Engine                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Ingestion  │  │   Search    │  │   Local API & MCP   │  │
│  │  Pipeline   │  │  (Tantivy)  │  │                     │  │
│  ├─────────────┤  └─────────────┘  └─────────────────────┘  │
│  │   Render    │  ┌─────────────────────────────────────┐   │
│  │  Pipeline   │  │            Sync Manager              │   │
│  └─────────────┘  └─────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                    Storage Layer                             │
│           SQLite  +  Filesystem  +  iCloud (optional)        │
└─────────────────────────────────────────────────────────────┘
```

### Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Native Shell | Swift + AppKit | Menu bar, notifications, global shortcuts |
| UI Framework | Svelte + TypeScript | Reactive, lightweight frontend |
| Core Engine | Rust | Parsing, indexing, sync, performance-critical ops |
| Web Rendering | WKWebView | Native WebKit with typography control |
| Search Index | Tantivy | Rust-native full-text search engine |
| Database | SQLite | Metadata, bookmarks, sync state |
| File Storage | Filesystem | Raw doc content organized by source |
| IPC Bridge | Tauri | Rust <-> JavaScript communication |
| Sync | CloudKit | Native macOS iCloud integration |

### Target Platform

- **macOS:** 12 (Monterey) and later
- **Architecture:** Apple Silicon only (M1, M2, M3, M4)

---

## Directory Structure

```
tome/
├── CLAUDE.md                 # This file - project documentation
├── README                    # Product requirements document
├── package.json              # NPM dependencies and scripts
├── tsconfig.json             # TypeScript configuration
├── vite.config.ts            # Vite bundler configuration
├── svelte.config.js          # Svelte compiler configuration
├── playwright.config.ts      # E2E test configuration
│
├── src/                      # Svelte frontend source
│   ├── lib/                  # Shared library code
│   │   ├── components/       # Reusable UI components
│   │   ├── stores/           # Svelte state stores
│   │   ├── services/         # Tauri IPC wrappers
│   │   ├── utils/            # Pure utility functions
│   │   └── types/            # TypeScript type definitions
│   ├── routes/               # Page components (SvelteKit)
│   ├── app.html              # HTML shell
│   ├── app.css               # Global styles
│   └── main.ts               # Application entry point
│
├── src-tauri/                # Rust backend source
│   ├── Cargo.toml            # Rust dependencies
│   ├── tauri.conf.json       # Tauri configuration
│   ├── build.rs              # Build script
│   └── src/
│       ├── main.rs           # Application entry point
│       ├── lib.rs            # Library exports
│       ├── commands/         # Tauri command handlers
│       ├── scraper/          # Documentation scraping
│       ├── parser/           # HTML parsing and normalization
│       ├── search/           # Tantivy search integration
│       ├── storage/          # SQLite + filesystem
│       ├── sync/             # iCloud sync logic
│       ├── config/           # Configuration management
│       └── error.rs          # Centralized error types
│
├── src-swift/                # Swift native shell (optional)
│   ├── Package.swift         # Swift Package Manager config
│   └── Sources/
│       └── TomeShell/        # Menu bar, notifications
│
├── e2e/                      # Playwright E2E tests
│   ├── fixtures/             # Test data and helpers
│   └── *.spec.ts             # Test specifications
│
├── scripts/                  # Development scripts
│   ├── setup.sh              # First-time setup
│   ├── gen-component.ts      # Component generator
│   ├── gen-command.ts        # Tauri command generator
│   └── check-boundaries.ts   # Architecture boundary checker
│
├── .github/                  # GitHub configuration
│   ├── workflows/            # CI/CD pipelines
│   └── CODEOWNERS            # Code ownership rules
│
└── .claude/                  # Claude Code configuration
    ├── plans/                # Project planning documents
    └── settings.json         # Claude Code settings
```

---

## Naming Conventions

### Files

| Type | Convention | Example |
|------|------------|---------|
| Svelte components | PascalCase.svelte | `SearchResults.svelte` |
| TypeScript modules | kebab-case.ts | `navigation-history.ts` |
| Test files | *.test.ts or *.spec.ts | `bookmarks.test.ts` |
| Rust modules | snake_case.rs | `url_filter.rs` |
| Rust tests | tests.rs in same dir | `scraper/tests.rs` |

### Code

| Language | Convention | Example |
|----------|------------|---------|
| TypeScript functions | camelCase | `createBookmark()` |
| TypeScript interfaces | PascalCase | `interface SearchResult` |
| TypeScript types | PascalCase | `type SyncStrategy` |
| Svelte components | PascalCase | `<SearchBox />` |
| Rust functions | snake_case | `fn parse_html()` |
| Rust structs | PascalCase | `struct PageMetadata` |
| Rust constants | SCREAMING_SNAKE | `const MAX_DEPTH: u32` |
| CSS classes | kebab-case | `.search-results` |
| CSS variables | --kebab-case | `--color-accent` |

### Tauri Commands

- Rust handler: `snake_case` - `fn list_sources()`
- JavaScript caller: `snake_case` string - `invoke('list_sources')`
- Keep names identical across boundary for clarity

---

## Blessed Patterns

### Error Handling (Rust)

```rust
// Use thiserror for error definitions in src-tauri/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TomeError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Parse error: {message}")]
    Parse { message: String },

    #[error("Not found: {resource}")]
    NotFound { resource: String },
}

// Tauri commands return Result<T, String> for IPC
// Convert TomeError to String at command boundary
#[tauri::command]
fn get_source(id: String, state: State<AppState>) -> Result<Source, String> {
    state.db.get_source(&id).map_err(|e| e.to_string())
}
```

### Error Handling (TypeScript)

```typescript
// Use typed errors with discriminated unions
type TomeResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: TomeError };

interface TomeError {
  code: 'NOT_FOUND' | 'NETWORK' | 'PARSE' | 'UNKNOWN';
  message: string;
}

// Wrap Tauri invoke calls
async function invokeCommand<T>(cmd: string, args?: object): Promise<TomeResult<T>> {
  try {
    const value = await invoke<T>(cmd, args);
    return { ok: true, value };
  } catch (error) {
    return {
      ok: false,
      error: {
        code: 'UNKNOWN',
        message: error instanceof Error ? error.message : String(error),
      },
    };
  }
}
```

### Logging (Rust)

```rust
// Use tracing crate for structured logging
use tracing::{info, warn, error, debug, instrument};

// Add #[instrument] to functions for automatic span creation
#[instrument(skip(state), fields(source_id = %id))]
async fn sync_source(id: &str, state: &AppState) -> Result<(), TomeError> {
    info!("Starting sync");

    // Use structured fields
    debug!(page_count = 42, "Fetched pages");

    if something_wrong {
        warn!(reason = "timeout", "Sync incomplete");
    }

    Ok(())
}
```

### Logging (TypeScript)

```typescript
// Use console with consistent prefixes (dev only)
// Production builds strip console.* calls
const log = {
  info: (msg: string, data?: object) =>
    console.log(`[tome:info] ${msg}`, data ?? ''),
  warn: (msg: string, data?: object) =>
    console.warn(`[tome:warn] ${msg}`, data ?? ''),
  error: (msg: string, data?: object) =>
    console.error(`[tome:error] ${msg}`, data ?? ''),
  debug: (msg: string, data?: object) =>
    console.debug(`[tome:debug] ${msg}`, data ?? ''),
};
```

### Data Fetching (Svelte)

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { sources } from '$lib/stores/sources';
  import { listSources } from '$lib/services/sources';

  // Fetch on mount, update store
  onMount(async () => {
    const result = await listSources();
    if (result.ok) {
      sources.set(result.value);
    } else {
      // Handle error - show toast, log, etc.
      console.error('Failed to load sources:', result.error);
    }
  });
</script>

<!-- Render from store -->
{#each $sources as source}
  <SourceItem {source} />
{/each}
```

### Svelte Store Pattern

```typescript
// src/lib/stores/sources.ts
import { writable, derived } from 'svelte/store';
import type { Source } from '$lib/types';

// Private writable store
const _sources = writable<Source[]>([]);

// Public readable API
export const sources = {
  subscribe: _sources.subscribe,
  set: _sources.set,
  add: (source: Source) => _sources.update(s => [...s, source]),
  remove: (id: string) => _sources.update(s => s.filter(x => x.id !== id)),
  update: (id: string, updates: Partial<Source>) =>
    _sources.update(s => s.map(x => x.id === id ? { ...x, ...updates } : x)),
};

// Derived stores for computed values
export const sourceCount = derived(_sources, $s => $s.length);
export const sourcesByCategory = derived(_sources, $s =>
  $s.reduce((acc, source) => {
    const cat = source.category ?? 'Uncategorized';
    acc[cat] = [...(acc[cat] ?? []), source];
    return acc;
  }, {} as Record<string, Source[]>)
);
```

### Tauri Command Pattern

```rust
// src-tauri/src/commands/sources.rs
use tauri::State;
use crate::{storage::Database, error::TomeError};

/// List all documentation sources
///
/// # Errors
/// Returns error string if database query fails
#[tauri::command]
pub async fn list_sources(db: State<'_, Database>) -> Result<Vec<Source>, String> {
    db.list_sources()
        .await
        .map_err(|e| e.to_string())
}

/// Add a new documentation source
///
/// # Arguments
/// * `config` - Source configuration from YAML
///
/// # Errors
/// Returns error if URL is invalid or database insert fails
#[tauri::command]
pub async fn add_source(
    config: SourceConfig,
    db: State<'_, Database>,
) -> Result<Source, String> {
    // Validate
    config.validate().map_err(|e| e.to_string())?;

    // Insert
    let source = Source::from_config(config);
    db.insert_source(&source)
        .await
        .map_err(|e| e.to_string())?;

    Ok(source)
}
```

### CSS Architecture

```css
/* Use CSS custom properties from design system */
/* src/app.css defines all variables */

.component {
  /* Layout */
  display: flex;
  gap: var(--space-4);
  padding: var(--space-3);

  /* Typography */
  font-family: var(--font-ui);
  font-size: var(--text-md);
  color: var(--color-text-primary);

  /* Visual */
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;

  /* Interaction */
  transition: background var(--duration-fast) var(--ease-out);
}

.component:hover {
  background: var(--color-bg-tertiary);
}

/* Dark mode handled automatically via CSS variables */
/* No need for manual dark mode classes */
```

---

## Anti-Patterns to Avoid

### General

- **No `any` types** - Always use proper TypeScript types
- **No `// @ts-ignore`** - Fix the type error instead
- **No `unwrap()` in production Rust** - Use `?` or explicit error handling
- **No `console.log` for errors** - Use structured logging
- **No hardcoded colors/sizes** - Use CSS variables from design system
- **No inline styles** - Use component-scoped CSS or utility classes
- **No `setTimeout` for async** - Use proper async/await patterns

### Architecture

```typescript
// BAD: Direct Tauri invoke in component
// This couples UI to IPC layer
<script>
  import { invoke } from '@tauri-apps/api/tauri';
  const sources = await invoke('list_sources'); // Don't do this!
</script>

// GOOD: Use service layer
<script>
  import { listSources } from '$lib/services/sources';
  const result = await listSources();
</script>
```

```rust
// BAD: Business logic in command handler
#[tauri::command]
fn sync_source(id: String, db: State<Database>) -> Result<(), String> {
    // 200 lines of scraping, parsing, storing... NO!
}

// GOOD: Thin command handler, logic in domain modules
#[tauri::command]
fn sync_source(id: String, state: State<AppState>) -> Result<(), String> {
    state.sync_manager.sync(&id).await.map_err(|e| e.to_string())
}
```

### State Management

```typescript
// BAD: Mutating store directly
$sources.push(newSource); // This doesn't trigger reactivity!

// GOOD: Use store methods
sources.add(newSource);

// BAD: Derived state in component
$: categoryCount = $sources.filter(s => s.category === 'Rust').length;

// GOOD: Derived store
export const rustSourceCount = derived(sources, $s =>
  $s.filter(s => s.category === 'Rust').length
);
```

### Testing

```typescript
// BAD: Testing implementation details
expect(component.internalState).toBe(...)  // Don't test internals!

// GOOD: Test behavior
expect(screen.getByText('Vec')).toBeInTheDocument();
```

```rust
// BAD: Tests that depend on external services
#[test]
fn test_fetch_docs() {
    let result = fetch("https://docs.rs/...");  // Flaky!
}

// GOOD: Mock external dependencies
#[test]
fn test_fetch_docs() {
    let server = MockServer::start();
    server.mock(...);
    let result = fetch(&server.url());  // Reliable
}
```

---

## Module Boundaries

### Dependency Rules

```
┌─────────────────────────────────────────────────────────────┐
│                         src/ (UI)                            │
│  Can import: lib/*, routes/*, $lib/*                        │
│  Cannot import: src-tauri/*, src-swift/*                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ Via Tauri IPC only
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    src-tauri/ (Core)                         │
│  Can import: Any Rust crate                                 │
│  Cannot import: src/*, src-swift/*                          │
└─────────────────────────────────────────────────────────────┘

Within src/:
  components/ → Cannot import from routes/
  stores/     → Cannot import from components/ or routes/
  services/   → Cannot import from components/ or stores/
  utils/      → Cannot import from any sibling (pure functions only)
  types/      → Cannot import from any sibling (type definitions only)

Within src-tauri/src/:
  commands/   → Can import from all other modules (thin handlers)
  scraper/    → Cannot import from commands/, search/, sync/
  parser/     → Cannot import from commands/, scraper/, search/, sync/
  search/     → Cannot import from commands/, scraper/
  storage/    → Cannot import from commands/ (data layer, no business logic)
  sync/       → Can import from storage/, scraper/ (orchestration)
  config/     → Cannot import from any sibling (pure configuration)
```

### Enforcement

Architecture boundaries are enforced by:
1. **ESLint import rules** - `eslint-plugin-import` with custom boundaries
2. **Rust module visibility** - `pub(crate)` and `pub(super)` appropriately
3. **CI check** - `npm run check:boundaries` runs on every PR

---

## Common Tasks

### Adding a New Svelte Component

```bash
# 1. Generate boilerplate
npm run gen:component SearchResults

# Creates:
# - src/lib/components/SearchResults.svelte
# - src/lib/components/SearchResults.test.ts

# 2. Implement component (see generated file for template)

# 3. Export from index if it's a public component
# Edit src/lib/components/index.ts
export { default as SearchResults } from './SearchResults.svelte';

# 4. Run tests
npm test SearchResults
```

### Adding a New Tauri Command

```bash
# 1. Generate boilerplate
npm run gen:command sync_source

# Creates:
# - src-tauri/src/commands/sync_source.rs
# - Updates src-tauri/src/commands/mod.rs
# - Creates src/lib/services/sync-source.ts

# 2. Implement Rust handler
# Edit src-tauri/src/commands/sync_source.rs

# 3. Register in main.rs
# Add to generate_handler![] macro

# 4. Implement TypeScript wrapper
# Edit src/lib/services/sync-source.ts

# 5. Run tests
cargo test sync_source
npm test sync-source
```

### Adding a New Svelte Store

```bash
# 1. Generate boilerplate
npm run gen:store navigation

# Creates:
# - src/lib/stores/navigation.ts
# - src/lib/stores/navigation.test.ts

# 2. Implement store (see generated file for template)

# 3. Export from index
# Edit src/lib/stores/index.ts
export * from './navigation';

# 4. Run tests
npm test navigation
```

### Adding a Database Migration

```bash
# 1. Create migration file
# src-tauri/migrations/YYYYMMDD_HHMMSS_description.sql

# 2. Write UP migration
-- Migration: Add bookmarks table
CREATE TABLE bookmarks (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    page_path TEXT NOT NULL,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(source_id, page_path)
);

# 3. Test migration locally
cargo test migration

# 4. Migrations run automatically on app start
```

### Running Specific Tests

```bash
# Rust - specific module
cargo test scraper::tests

# Rust - specific test
cargo test test_parse_url_valid

# JavaScript - specific file
npm test -- bookmarks.test.ts

# JavaScript - specific test
npm test -- -t "should create bookmark"

# E2E - specific spec
npx playwright test search.spec.ts

# E2E - specific test
npx playwright test -g "should find results"
```

### Debugging

```bash
# Rust - enable debug logging
RUST_LOG=tome=debug npm run dev

# Rust - enable trace logging for specific module
RUST_LOG=tome::scraper=trace npm run dev

# JavaScript - enable source maps
npm run dev  # Source maps enabled by default in dev

# E2E - headed mode (see browser)
npx playwright test --headed

# E2E - debug mode (pause on failure)
npx playwright test --debug
```

---

## Performance Guidelines

### Targets

| Metric | Target |
|--------|--------|
| Cold start | < 500ms to usable UI |
| Simple search | < 50ms (P50) |
| Complex search | < 100ms (P95) |
| Index 1000 pages | < 30s |
| Idle memory | < 200MB |

### Best Practices

1. **Lazy load** - Don't load all sources at startup, use pagination
2. **Debounce search** - Wait 150ms after typing stops before searching
3. **Virtual lists** - Use virtual scrolling for lists > 100 items
4. **Incremental indexing** - Only re-index changed pages
5. **Background sync** - Sync in background, don't block UI

---

## Security Guidelines

### Mandatory

- **No telemetry** - Never collect or transmit user data
- **HTTPS only** - All network requests must use HTTPS
- **Localhost API** - Local API binds to 127.0.0.1 only
- **Input validation** - Validate all user input and config files
- **No eval()** - Never use eval or Function constructor
- **Content Security Policy** - Strict CSP in WebView

### Secrets

- **Never commit secrets** - Use environment variables or keychain
- **No hardcoded credentials** - Use Tauri's credential storage
- **.env files** - Never commit, add to .gitignore

---

## Testing Strategy

### Coverage Target: 90%

### Test Pyramid

| Type | Percentage | Framework | Location |
|------|------------|-----------|----------|
| Unit | 80% | Rust tests, Jest | Adjacent to source |
| Integration | 15% | Rust tests, Jest | `tests/` directories |
| E2E | 5% | Playwright | `e2e/` directory |

### Test Naming

```rust
// Rust: test_<function>_<scenario>_<expected>
#[test]
fn test_parse_url_invalid_scheme_returns_error() { ... }
```

```typescript
// TypeScript: describe/it with behavior description
describe('Bookmarks Store', () => {
  it('should create bookmark with required fields', () => { ... });
});
```

---

## Git Workflow

### Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feature/<description>` | `feature/add-rustdoc-scraper` |
| Bug fix | `fix/<description>` | `fix/search-pagination` |
| Docs | `docs/<description>` | `docs/update-readme` |

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`

Examples:
```
feat(search): add fuzzy matching with typo tolerance
fix(scraper): handle relative URLs in Sphinx docs
docs: update CLAUDE.md with new patterns
```

### Pre-commit Checks

All commits must pass:
1. `cargo fmt --check` - Rust formatting
2. `cargo clippy` - Rust linting
3. `npm run lint` - TypeScript/Svelte linting
4. `npm run format:check` - Prettier formatting
5. `cargo test` - Rust unit tests
6. `npm test` - JavaScript unit tests

---

## Troubleshooting

### Build Failures

```bash
# Clean and rebuild
npm run clean
npm run build

# Reset Cargo cache
cargo clean
cargo build

# Reset node_modules
rm -rf node_modules
npm install
```

### Tauri Issues

```bash
# Rebuild Tauri
cd src-tauri
cargo clean
cd ..
npm run build
```

### Database Issues

```bash
# Reset database (development only!)
rm ~/.tome/tome.db
npm run dev  # Recreates with migrations
```

### Test Failures

```bash
# Run with verbose output
cargo test -- --nocapture
npm test -- --verbose

# Run single test for debugging
cargo test test_name -- --nocapture
npm test -- -t "test name" --verbose
```

---

## Resources

### Internal Documentation

- [Product Requirements](./README) - Full PRD
- [Phase 1 Plan](./.claude/plans/01-phase-1-foundation.md) - Foundation tickets
- [Testing Strategy](./.claude/plans/08-testing-strategy.md) - Test details
- [Design System](./.claude/plans/15-design-system.md) - UI components
- [CI/CD](./.claude/plans/10-cicd-devops.md) - Pipeline details

### External Documentation

- [Tauri Docs](https://tauri.app/v1/guides/)
- [Svelte Docs](https://svelte.dev/docs)
- [Tantivy Docs](https://docs.rs/tantivy/)
- [Rust Book](https://doc.rust-lang.org/book/)

---

## Contact

For questions or issues, check the project issues or consult the planning documents in `.claude/plans/`.
