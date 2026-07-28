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
