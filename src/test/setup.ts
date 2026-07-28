/**
 * Test setup. Stubs the Tauri IPC boundary once, centrally.
 *
 * Frontend tests must never reach a real backend: they assert on the *contract*
 * with Rust (which commands were invoked, with what arguments), not on Rust's
 * behaviour. Backend behaviour is covered by Tier B integration tests.
 */
import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

/**
 * An in-memory `localStorage`.
 *
 * jsdom under this Vitest configuration exposes none — `window.localStorage`
 * is `undefined` — so without this every persistence test would assert
 * against the fallback path rather than the real one. `$lib/stores/preferences`
 * is written to survive exactly that (every access is guarded), which is why
 * the app works in the test environment at all; this stub is so the *stored*
 * behaviour is also covered rather than only the degraded one.
 */
if (typeof globalThis.localStorage === 'undefined') {
  const store = new Map<string, string>();
  const storage: Storage = {
    get length() {
      return store.size;
    },
    clear: () => store.clear(),
    getItem: (key) => store.get(key) ?? null,
    key: (index) => [...store.keys()][index] ?? null,
    removeItem: (key) => {
      store.delete(key);
    },
    setItem: (key, value) => {
      store.set(key, String(value));
    },
  };
  Object.defineProperty(globalThis, 'localStorage', { value: storage, configurable: true });
}

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
