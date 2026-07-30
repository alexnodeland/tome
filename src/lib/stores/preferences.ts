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

/** A string preference, length-capped so a hand-edited store cannot bloat. */
export function stringPreference(key: string, fallback: string, maxLength = 200) {
  return {
    load: () => read(key, fallback, (raw) => raw.slice(0, maxLength)),
    save: (value: string) => write(key, value.slice(0, maxLength)),
  };
}

/**
 * A preference with a fixed set of values.
 *
 * Separate from `stringPreference` because the values here reach the DOM as
 * attribute values that CSS selects on. A stored string that is not one of
 * `allowed` would set `data-theme="whatever-was-typed"`, match no rule, and
 * leave the app in the default theme with a preference that reads as changed —
 * which looks like a broken theme rather than a rejected value.
 */
export function enumPreference<T extends string>(key: string, fallback: T, allowed: readonly T[]) {
  const valid = (raw: string): T | null => (allowed.includes(raw as T) ? (raw as T) : null);
  return {
    load: () => read<T>(key, fallback, valid),
    save: (value: T) => {
      if (valid(value) !== null) write(key, value);
    },
    values: allowed,
  };
}

/** Colour scheme. `system` means "no attribute", so the media query decides. */
export const THEMES = ['system', 'light', 'dark'] as const;
export type Theme = (typeof THEMES)[number];

/**
 * Reader text size.
 *
 * Named steps rather than a pixel number, because `tokens.css` rescales the
 * *whole system* off the root font size — `:root[data-text-size="large"]`
 * — and every other size in the design system is a rem against it. An
 * arbitrary px value would let someone pick 13px and shrink the UI chrome
 * along with the prose.
 */
export const TEXT_SIZES = ['small', 'default', 'large', 'xlarge'] as const;
export type TextSize = (typeof TEXT_SIZES)[number];

/** Column width. `default` is the design system's 70ch optimal measure. */
export const MEASURES = ['narrow', 'default', 'wide'] as const;
export type Measure = (typeof MEASURES)[number];

export const preferences = {
  leftWidth: numberPreference('sidebar.left.width', 240, 180, 400),
  rightWidth: numberPreference('sidebar.right.width', 200, 180, 400),
  leftOpen: booleanPreference('sidebar.left.open', true),
  rightOpen: booleanPreference('sidebar.right.open', true),
  /** The last search scope (P2-008). Empty string means "all sources" — a
   *  sentinel rather than `null`, because `localStorage` only holds strings
   *  and an absent key already means "never set". */
  searchScope: stringPreference('search.scope', ''),

  // ---- Appearance (P5-007) ------------------------------------------------
  // Each of these becomes one attribute on <html>, in the app and in the
  // reader frame. See `$lib/appearance`.
  theme: enumPreference<Theme>('appearance.theme', 'system', THEMES),
  textSize: enumPreference<TextSize>('appearance.textSize', 'default', TEXT_SIZES),
  measure: enumPreference<Measure>('appearance.measure', 'default', MEASURES),
  /** Line numbers in code blocks. Reader-only; the app chrome has no code. */
  lineNumbers: booleanPreference('appearance.lineNumbers', false),

  // ---- General ------------------------------------------------------------
  /** Ask before removing a source. On by default: removal deletes content. */
  confirmBeforeRemove: booleanPreference('general.confirmBeforeRemove', true),
  /**
   * Whether onboarding has been dismissed or completed.
   *
   * Distinct from "the library is empty": someone who removes their last
   * source has not become a first-time user, and putting the welcome screen
   * back in front of them would be a lie about what they know.
   */
  onboarded: booleanPreference('onboarding.done', false),
};
