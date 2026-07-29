/**
 * Recent searches (S2-7, spec P2-016).
 *
 * `localStorage`, for the same reason as `preferences`: this is something it
 * would be mildly annoying to lose, not something that would be *lost*. A
 * search history in SQLite is a write on every query and a migration to
 * maintain.
 *
 * # It is reading history, and it is treated as such
 *
 * A list of what someone searched their documentation for is as revealing as
 * a browser history. Two consequences are built in rather than assumed:
 * nothing here is ever logged or sent anywhere (there is no telemetry in Tome
 * of any kind), and `clear` really removes it rather than marking it hidden.
 *
 * Every read is defensive. `localStorage` can be unavailable, can hold what
 * another version wrote, and can hold what somebody typed into the inspector.
 * A history that throws on load would stop the app from opening, which no
 * convenience is worth.
 */

const KEY = 'tome.search.history';

/**
 * P2-016's "store last 50 searches".
 *
 * A cap rather than unbounded growth: `localStorage` has a quota, and a
 * history long enough to need scrolling is not a history anyone reads.
 */
export const MAX_ENTRIES = 50;

/** Longest query kept. Anything longer is a paste, not a search. */
const MAX_QUERY_LENGTH = 200;

function load(): string[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw === null) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    // Filter rather than trust: the stored value is JSON somebody could have
    // edited, and a non-string entry would render as `[object Object]` or
    // throw in the click handler.
    return parsed
      .filter((entry): entry is string => typeof entry === 'string')
      .map((entry) => entry.slice(0, MAX_QUERY_LENGTH))
      .filter((entry) => entry.trim().length > 0)
      .slice(0, MAX_ENTRIES);
  } catch {
    return [];
  }
}

function save(entries: string[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(entries));
  } catch {
    // A full or disabled store is not worth a broken UI. The history is
    // simply not remembered.
  }
}

export const searchHistory = {
  /** Most recent first. */
  list(): string[] {
    return load();
  },

  /**
   * Record a query.
   *
   * Deduplicates against the *whole* list rather than only the previous entry,
   * which P2-016 asks for as "deduplicate consecutive identical searches" but
   * is not enough on its own: a search re-run from the history list would
   * otherwise appear twice, since the entry between them is whatever was
   * searched in between.
   *
   * The query is stored trimmed. An empty or whitespace-only query is not a
   * search and is not recorded.
   */
  add(query: string): string[] {
    const trimmed = query.trim().slice(0, MAX_QUERY_LENGTH);
    if (trimmed.length === 0) return load();
    const rest = load().filter((entry) => entry !== trimmed);
    const next = [trimmed, ...rest].slice(0, MAX_ENTRIES);
    save(next);
    return next;
  },

  /** Forget one query (P2-016's "clear individual"). */
  remove(query: string): string[] {
    const next = load().filter((entry) => entry !== query);
    save(next);
    return next;
  },

  /** Forget everything. */
  clear(): string[] {
    try {
      localStorage.removeItem(KEY);
    } catch {
      // Nothing to do, and nothing worth breaking the UI over.
    }
    return [];
  },
};
