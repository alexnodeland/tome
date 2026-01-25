// Type definitions for Tome
// See README.md in this directory for patterns

// === Result Type ===

/**
 * A result type that represents either success or failure.
 * Use this for all fallible operations instead of throwing.
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

// === Type Guards ===

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

// === Source Types ===

/**
 * A documentation source
 */
export interface Source {
  id: string;
  name: string;
  sourceType: SourceType;
  url?: string;
  localPath?: string;
  version?: string;
  category: string;
  icon?: string;
  accentColor?: string;
  syncStrategy: SyncStrategy;
  syncSchedule?: string;
  pinVersion: boolean;
  createdAt: string;
  lastSyncedAt?: string;
  pageCount: number;
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

// === Page Types ===

/**
 * A documentation page
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

// === Search Types ===

/**
 * Search query options
 */
export interface SearchQuery {
  query: string;
  sourceIds?: string[];
  categories?: string[];
  limit?: number;
  offset?: number;
}

/**
 * A single search result
 */
export interface SearchResult {
  id: string;
  sourceId: string;
  pagePath: string;
  title: string;
  snippet: string;
  score: number;
}

/**
 * Search response
 */
export interface SearchResponse {
  results: SearchResult[];
  totalCount: number;
  queryTimeMs: number;
}

// === Bookmark Types ===

/**
 * A bookmark
 */
export interface Bookmark {
  id: string;
  sourceId: string;
  pagePath: string;
  title: string;
  createdAt: string;
  collectionId?: string;
}

/**
 * A highlight annotation
 */
export interface Highlight {
  id: string;
  bookmarkId: string;
  startOffset: number;
  endOffset: number;
  text: string;
  note?: string;
  createdAt: string;
}
