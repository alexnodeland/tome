/**
 * UI preferences that survive a relaunch (S1-14, P1-017).
 *
 * `localStorage`, not the database. The split is deliberate and follows the
 * same rule as `paths.rs`: the database holds things that would be *lost* if
 * they went away — bookmarks, annotations, page metadata — and this holds
 * things it would merely be mildly annoying to rebuild. A sidebar width in
 * SQLite is a write on every drag frame and a migration to maintain.
 *
 * Every read is defensive. `localStorage` can be unavailable (a webview with
 * storage disabled, a test environment), can hold something another version
 * wrote, and can hold something a person typed into the inspector. A
 * preference that throws on load would stop the app from opening, which no
 * preference is worth.
 */

const PREFIX = 'tome.';

function read<T>(key: string, fallback: T, parse: (raw: string) => T | null): T {
  try {
    const raw = localStorage.getItem(PREFIX + key);
    if (raw === null) return fallback;
    const value = parse(raw);
    return value === null ? fallback : value;
  } catch {
    return fallback;
  }
}

function write(key: string, value: string): void {
  try {
    localStorage.setItem(PREFIX + key, value);
  } catch {
    // A full or disabled store is not worth a broken UI. The preference is
    // simply not remembered.
  }
}

/** A number preference, clamped to a range the UI can actually render. */
export function numberPreference(key: string, fallback: number, min: number, max: number) {
  const clamp = (n: number) => Math.min(max, Math.max(min, n));
  return {
    load: () =>
      read(key, fallback, (raw) => {
        const parsed = Number(raw);
        return Number.isFinite(parsed) ? clamp(parsed) : null;
      }),
    save: (value: number) => write(key, String(clamp(value))),
  };
}

/** A boolean preference. */
export function booleanPreference(key: string, fallback: boolean) {
  return {
    load: () =>
      read(key, fallback, (raw) => (raw === 'true' ? true : raw === 'false' ? false : null)),
    save: (value: boolean) => write(key, String(value)),
  };
}

export const preferences = {
  leftWidth: numberPreference('sidebar.left.width', 240, 180, 400),
  rightWidth: numberPreference('sidebar.right.width', 200, 180, 400),
  leftOpen: booleanPreference('sidebar.left.open', true),
  rightOpen: booleanPreference('sidebar.right.open', true),
};
