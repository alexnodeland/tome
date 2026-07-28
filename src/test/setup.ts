/**
 * Test setup. Stubs the Tauri IPC boundary once, centrally.
 *
 * Frontend tests must never reach a real backend: they assert on the *contract*
 * with Rust (which commands were invoked, with what arguments), not on Rust's
 * behaviour. Backend behaviour is covered by Tier B integration tests.
 */
import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

/** Every `invoke` made during a test, in order. Assert against this. */
export const invoked: Array<[string, unknown]> = [];

/** Per-command canned responses. Override in a test before rendering. */
export const mockResponses: Record<string, unknown> = {
  library_location: {
    bundle_id: 'com.alexnodeland.tome',
    version: '0.0.0',
    state_root: '/Users/test/Library/Application Support/Tome',
    cache_root: '/Users/test/Library/Caches/Tome',
    initialised: true,
  },
};

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string, args?: unknown) => {
    invoked.push([cmd, args]);
    if (!(cmd in mockResponses)) {
      // Loud by default: a command with no stub is almost always a test that
      // has drifted from the backend, not a test that wants `undefined`.
      throw new Error(`No mock response registered for Tauri command "${cmd}"`);
    }
    return mockResponses[cmd];
  }),
}));

beforeEach(() => {
  invoked.length = 0;
});
