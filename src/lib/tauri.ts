/**
 * The single seam between the UI and the Rust core.
 *
 * Every `invoke` goes through here, which is what makes the frontend testable:
 * tests stub this one module rather than reaching for a real backend. See
 * `src/test/setup.ts` and docs/plans/08-testing-strategy.md § Tier A.
 */
import { invoke } from '@tauri-apps/api/core';

export interface LibraryLocation {
  bundle_id: string;
  version: string;
  state_root: string;
  cache_root: string;
  initialised: boolean;
}

export interface SourceSummary {
  id: string;
  name: string;
  category: string;
  page_count: number;
  last_synced: string | null;
}

export interface PageSummary {
  path: string;
  title: string;
}

/** One heading in a page's outline. Nested, so the sidebar can indent. */
export interface OutlineEntry {
  id: string;
  title: string;
  level: number;
  children: OutlineEntry[];
}

/**
 * A rendered page.
 *
 * `html` is produced by the Rust renderer, not by the frontend — every
 * attribute quoted and every value escaped there, which is the contract the
 * sanitizer depends on (`crates/tome-core/src/render.rs`). **Nothing in the
 * frontend may build reader HTML from page content**; it only moves this
 * string into the sandboxed frame.
 */
export interface ReaderPage {
  source_id: string;
  path: string;
  title: string;
  html: string;
  outline: OutlineEntry[];
}

export async function libraryLocation(): Promise<LibraryLocation> {
  return invoke<LibraryLocation>('library_location');
}

export async function listSources(): Promise<SourceSummary[]> {
  return invoke<SourceSummary[]>('list_sources');
}

export async function listPages(sourceId: string): Promise<PageSummary[]> {
  return invoke<PageSummary[]>('list_pages', { sourceId });
}

export async function readPage(sourceId: string, path: string): Promise<ReaderPage> {
  return invoke<ReaderPage>('read_page', { sourceId, path });
}

/**
 * Open a link in the user's browser.
 *
 * Validated in Rust against an allowlist of `http`, `https`, and `mailto` —
 * see `src-tauri/src/reader.rs`. The frontend does not decide what is safe to
 * hand to the operating system; it only asks.
 */
export async function openExternal(url: string): Promise<void> {
  return invoke<void>('open_external', { url });
}

/**
 * One run of snippet text.
 *
 * **Render `text` as a text node, never as HTML.** A snippet is crawled page
 * content, and it is drawn in the *app's* DOM rather than the sandboxed
 * reader frame — where the app's origin and its IPC layer are reachable. That
 * is why the backend sends spans and a boolean instead of a marked-up string:
 * there is no markup to escape because there is no markup. An `{@html}` here
 * would be the shortest path from a crawled page to script running with the
 * app's privileges.
 */
export interface SnippetSpan {
  text: string;
  matched: boolean;
}

/** What kind of thing a page documents, when it documents one (P2-015). */
export type SymbolKind = 'function' | 'type' | 'trait' | 'module' | 'constant' | 'macro';

export interface SearchHit {
  source_id: string;
  source_name: string;
  path: string;
  title: string;
  score: number;
  symbol_kind: SymbolKind | null;
  snippet: SnippetSpan[];
}

/** A term the user typed that matched nothing, and what it probably meant. */
export interface SearchSuggestion {
  typed: string;
  meant: string;
}

export interface SearchResponse {
  hits: SearchHit[];
  /** Always present, never null — no empty-case branch needed. */
  suggestions: SearchSuggestion[];
  elapsed_ms: number;
  /** Whether the ranked list hit `limit`. There is no total count: getting one
   *  needs a second uncapped pass, and an invented number is worse than none. */
  truncated: boolean;
}

/**
 * Search the library.
 *
 * `scope` limits results to one source. Prefixing a term with `@` searches
 * declared symbols only — the backend parses that, so the UI passes the query
 * through unchanged.
 */
export async function search(
  query: string,
  scope: string | null,
  limit: number,
): Promise<SearchResponse> {
  return invoke<SearchResponse>('search', { query, scope, limit });
}

/**
 * Whether a source id still exists.
 *
 * The search scope is remembered across launches, and a source can be removed
 * between them. Without this the UI would silently return no results for ever,
 * scoped to something that is gone.
 */
export async function sourceExists(sourceId: string): Promise<boolean> {
  return invoke<boolean>('source_exists', { sourceId });
}

/**
 * One installable source from the bundled registry (S4-4).
 *
 * The catalogue ships **inside the app bundle** rather than being fetched, so
 * that onboarding works with no network — see `src-tauri/src/onboarding.rs`.
 * Installing does reach the network, from the documentation's own origin.
 */
export interface CatalogueEntry {
  id: string;
  name: string;
  category: string;
  homepage: string;
  licence: string;
  /** When the verification job last confirmed this config still works. Shown,
   *  not hidden: a stale date is the only warning that a scraper has rotted. */
  verified: string;
  installed: boolean;
}

export interface InstallReport {
  source_id: string;
  pages: number;
  /** Pages that could not be fetched or parsed. Not fatal, and not hidden —
   *  40 pages of an expected 200 looks like success from the count alone. */
  page_errors: number;
  capped: boolean;
  /** Pages deleted because the site no longer has them. Always 0 on a first
   *  install; present anyway so the shape does not change on a re-pull. */
  pruned: number;
}

/** Progress while a source installs, pushed from Rust over the event bus. */
export interface InstallProgress {
  source_id: string;
  phase: 'crawling' | 'storing' | 'indexing';
  done: number;
  /** Zero while crawling: the total is unknown until the crawl ends, and an
   *  invented denominator makes a progress bar that goes backwards. */
  total: number;
}

export async function registryCatalogue(): Promise<CatalogueEntry[]> {
  return invoke<CatalogueEntry[]>('registry_catalogue');
}

/** Write a registry source's configuration and pull it. Resolves when done. */
export async function installRegistrySource(id: string): Promise<InstallReport> {
  return invoke<InstallReport>('install_registry_source', { id });
}

/**
 * Register or clear the system-wide shortcut (P5-009).
 *
 * `null` clears it. Rejects with the reason on failure — on macOS a refused
 * registration is the *only* conflict detection there is, since no API lists
 * which application holds which combination.
 */
export async function setGlobalShortcut(accelerator: string | null): Promise<void> {
  return invoke<void>('set_global_shortcut', { accelerator });
}

/** Show or hide the Dock icon. Hiding makes Tome menu-bar-only. */
export async function setDockVisible(visible: boolean): Promise<void> {
  return invoke<void>('set_dock_visible', { visible });
}

/** Why the app was brought forward from the menu bar or a global shortcut. */
export type ActivateIntent = 'search' | 'catalogue' | 'window';

/**
 * Listen for activation from outside the window. Returns an unlisten function.
 *
 * Someone who pressed a global shortcut is looking for something, so the
 * default intent opens search rather than only raising the window.
 */
export async function onActivate(handler: (intent: ActivateIntent) => void): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<{ intent: ActivateIntent }>('activate', (event) => handler(event.payload.intent));
}

/**
 * Listen for install progress. Returns an unlisten function.
 *
 * Imported lazily so that `src/test/setup.ts` — which stubs `invoke` — does
 * not also have to stand up the event bus for components that never install
 * anything.
 */
export async function onInstallProgress(
  handler: (progress: InstallProgress) => void,
): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<InstallProgress>('install-progress', (event) => handler(event.payload));
}
