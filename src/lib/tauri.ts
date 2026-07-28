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

export async function libraryLocation(): Promise<LibraryLocation> {
  return invoke<LibraryLocation>('library_location');
}
