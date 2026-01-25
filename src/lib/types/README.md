# Types Directory

TypeScript type definitions shared across the application.

## What Belongs Here

- **Type definitions** (`.ts` files) used by multiple modules
- **Interface definitions** for domain models
- **Utility types** for common patterns
- **Zod schemas** (if using runtime validation)

## What Does NOT Belong Here

- Implementation code (types only)
- Component-specific types (keep with component)
- Test utilities (use test fixtures)

## Naming Conventions

- Type files: `kebab-case.ts` (e.g., `source-types.ts`)
- Interface names: `PascalCase` (e.g., `interface Source`)
- Type names: `PascalCase` (e.g., `type SyncStrategy`)
- Export all types from `index.ts`

## Type Definitions

```typescript
// source.ts
// Domain model for documentation sources

/**
 * A documentation source represents a collection of docs from a single origin
 */
export interface Source {
  /** Unique identifier */
  id: string;
  /** Display name */
  name: string;
  /** Source type for platform-specific handling */
  sourceType: SourceType;
  /** Original URL (for web sources) */
  url?: string;
  /** Local filesystem path (for local sources) */
  localPath?: string;
  /** Version identifier */
  version?: string;
  /** Category for organization */
  category: string;
  /** Icon emoji or URL */
  icon?: string;
  /** Accent color for UI */
  accentColor?: string;
  /** When to sync */
  syncStrategy: SyncStrategy;
  /** Cron expression for scheduled sync */
  syncSchedule?: string;
  /** Whether to pin to specific version */
  pinVersion: boolean;
  /** When the source was added */
  createdAt: string;
  /** When the source was last synced */
  lastSyncedAt?: string;
  /** Number of pages in source */
  pageCount: number;
  /** Size of search index in bytes */
  indexSizeBytes: number;
}

/**
 * Supported documentation platforms
 */
export type SourceType =
  | 'readthedocs'
  | 'rustdoc'
  | 'mdbook'
  | 'man'
  | 'gitbook'
  | 'docusaurus'
  | 'mkdocs'
  | 'generic'
  | 'local';

/**
 * When to sync a source
 */
export type SyncStrategy = 'manual' | 'on_launch' | 'scheduled' | 'watch';

/**
 * Configuration for adding a new source
 */
export interface SourceConfig {
  name: string;
  source: SourceTypeConfig;
  version?: string;
  category?: string;
  display?: DisplayConfig;
  sync?: SyncConfig;
}

export type SourceTypeConfig =
  | { type: 'readthedocs'; url: string }
  | { type: 'rustdoc'; url: string }
  | { type: 'mdbook'; url: string }
  | { type: 'man'; paths: string[]; sections?: number[] }
  | { type: 'generic'; url: string; generic: GenericConfig }
  | { type: 'local'; path: string };

export interface GenericConfig {
  entryPoints: string[];
  maxDepth: number;
  contentSelector: string;
  titleSelector: string;
  navSelector?: string;
  includePatterns?: string[];
  excludePatterns?: string[];
}

export interface DisplayConfig {
  icon?: string;
  accentColor?: string;
}

export interface SyncConfig {
  strategy: SyncStrategy;
  schedule?: string;
  pinVersion?: boolean;
}
```

```typescript
// result.ts
// Result types for error handling

/**
 * A result type that represents either success or failure
 * Use this for all fallible operations instead of throwing
 */
export type TomeResult<T> = { ok: true; value: T } | { ok: false; error: TomeError };

/**
 * Standard error type for the application
 */
export interface TomeError {
  /** Error code for programmatic handling */
  code: TomeErrorCode;
  /** Human-readable error message */
  message: string;
  /** Additional context (for debugging) */
  context?: Record<string, unknown>;
}

/**
 * All possible error codes
 */
export type TomeErrorCode =
  | 'NOT_FOUND'
  | 'NETWORK'
  | 'PARSE'
  | 'VALIDATION'
  | 'PERMISSION'
  | 'SYNC'
  | 'UNKNOWN';

/**
 * Type guard to check if a result is successful
 */
export function isOk<T>(result: TomeResult<T>): result is { ok: true; value: T } {
  return result.ok;
}

/**
 * Type guard to check if a result is an error
 */
export function isErr<T>(result: TomeResult<T>): result is { ok: false; error: TomeError } {
  return !result.ok;
}

/**
 * Unwrap a result, throwing if it's an error
 * Only use this when you're certain the result is ok
 */
export function unwrap<T>(result: TomeResult<T>): T {
  if (result.ok) {
    return result.value;
  }
  throw new Error(`Unwrap called on error result: ${result.error.message}`);
}
```

```typescript
// page.ts
// Types for documentation pages

/**
 * A single page of documentation
 */
export interface Page {
  id: string;
  sourceId: string;
  path: string;
  title: string;
  contentHash: string;
  lastModified: string;
}

/**
 * Page content for rendering
 */
export interface PageContent {
  html: string;
  title: string;
  toc: TocEntry[];
}

/**
 * Table of contents entry
 */
export interface TocEntry {
  id: string;
  title: string;
  level: number;
  children: TocEntry[];
}
```

```typescript
// search.ts
// Types for search functionality

/**
 * Search query options
 */
export interface SearchQuery {
  /** Search text */
  query: string;
  /** Limit to specific sources */
  sourceIds?: string[];
  /** Limit to specific categories */
  categories?: string[];
  /** Maximum results */
  limit?: number;
  /** Offset for pagination */
  offset?: number;
}

/**
 * A single search result
 */
export interface SearchResult {
  /** Result ID */
  id: string;
  /** Source this result belongs to */
  sourceId: string;
  /** Page path within source */
  pagePath: string;
  /** Page title */
  title: string;
  /** Snippet with highlighted matches */
  snippet: string;
  /** Relevance score */
  score: number;
}

/**
 * Search response with results and metadata
 */
export interface SearchResponse {
  results: SearchResult[];
  totalCount: number;
  queryTimeMs: number;
}
```

## Index File

```typescript
// index.ts
export * from './source';
export * from './page';
export * from './search';
export * from './result';
```

## Architectural Rules

1. Types **cannot import from any sibling directory** (pure type definitions only)
2. Types **can import from** external type libraries (e.g., Zod)
3. Keep types **close to their domain** (sources, pages, search, etc.)
4. Use **discriminated unions** for variants (SourceTypeConfig)
5. Document types with **JSDoc comments**
6. Export **type guards** where useful (isOk, isErr)
