/**
 * Reading history (S1-15, P1-020).
 *
 * A browser-shaped stack: going somewhere new truncates whatever was ahead,
 * back and forward move a cursor. Deliberately a plain class with no
 * framework in it — history is the piece most likely to be subtly wrong, and
 * a pure object can be tested exhaustively without rendering anything.
 *
 * # It is per session, on purpose
 *
 * Nothing here is persisted. "Where I was three launches ago" is not a
 * question anyone asks of a documentation reader, and a restored history full
 * of pages from a source that has since been removed is worse than an empty
 * one. Bookmarks are the durable thing, and they are a different feature with
 * a different home (the database).
 */

export interface HistoryEntry {
  sourceId: string;
  path: string;
  /** In-page anchor, without the `#`. */
  fragment: string | null;
  /** Where the reader was scrolled to, restored on back/forward. */
  scrollTop: number;
}

/**
 * Upper bound on remembered entries.
 *
 * P1-020's success metric is "history survives 1000+ entries", so the cap is
 * above it and trimming is from the *oldest* end — a reader who has visited
 * 1 200 pages wants back to keep working, not to be told it has run out.
 */
const DEFAULT_LIMIT = 2000;

export class NavigationHistory {
  private entries: HistoryEntry[] = [];
  private index = -1;

  constructor(private readonly limit: number = DEFAULT_LIMIT) {}

  get current(): HistoryEntry | null {
    return this.entries[this.index] ?? null;
  }

  get length(): number {
    return this.entries.length;
  }

  canGoBack(): boolean {
    return this.index > 0;
  }

  canGoForward(): boolean {
    return this.index < this.entries.length - 1;
  }

  /**
   * Record a navigation.
   *
   * Navigating to where you already are is *not* a new entry — it replaces
   * the current one. Sphinx puts a permalink on every heading, so following
   * one would otherwise push a duplicate and make the back button undo
   * nothing visible, twice.
   */
  push(entry: HistoryEntry): void {
    const current = this.current;
    if (
      current &&
      current.sourceId === entry.sourceId &&
      current.path === entry.path &&
      current.fragment === entry.fragment
    ) {
      this.entries[this.index] = { ...entry };
      return;
    }

    // Anything ahead of the cursor is a future that no longer happens.
    this.entries.splice(this.index + 1);
    this.entries.push({ ...entry });

    if (this.entries.length > this.limit) {
      // Trim from the oldest end and move the cursor with it, so back keeps
      // working rather than the newest navigation being refused.
      const excess = this.entries.length - this.limit;
      this.entries.splice(0, excess);
      this.index = this.entries.length - 1;
    } else {
      this.index = this.entries.length - 1;
    }
  }

  back(): HistoryEntry | null {
    if (!this.canGoBack()) return null;
    this.index -= 1;
    return this.current;
  }

  forward(): HistoryEntry | null {
    if (!this.canGoForward()) return null;
    this.index += 1;
    return this.current;
  }

  /**
   * Update where the current entry was scrolled to.
   *
   * Called as the reader scrolls, which is why it mutates in place rather
   * than pushing: scrolling is not navigation, and a history entry per scroll
   * event would make the back button useless within one page.
   */
  recordScroll(scrollTop: number): void {
    const current = this.entries[this.index];
    if (current) current.scrollTop = scrollTop;
  }

  /** Everything remembered, oldest first. For diagnostics and tests. */
  snapshot(): readonly HistoryEntry[] {
    return this.entries.map((entry) => ({ ...entry }));
  }
}

/**
 * Split a link href from the renderer into its parts.
 *
 * The renderer has already decided what kind of link this is
 * (`pipeline::relink`): an internal one is a library path, an external one
 * keeps its scheme. So the *shape* is the routing decision and there is no
 * URL parsing, no host comparison, and no ambiguity about which source a
 * bare path belongs to — it is the one the reader is already in.
 */
export type LinkTarget =
  | { kind: 'fragment'; fragment: string }
  | { kind: 'page'; path: string; fragment: string | null }
  | { kind: 'external'; url: string };

export function classifyLink(href: string): LinkTarget {
  const trimmed = href.trim();

  if (trimmed.startsWith('#')) {
    return { kind: 'fragment', fragment: trimmed.slice(1) };
  }
  // A scheme, by the same grammar the sanitizer uses: `[a-z][a-z0-9+.-]*:`
  // before any `/`, `?`, or `#`. A protocol-relative `//host/x` counts too —
  // it is external navigation whatever the renderer thought.
  if (/^[a-z][a-z0-9+.-]*:/i.test(trimmed) || trimmed.startsWith('//')) {
    return { kind: 'external', url: trimmed };
  }

  const hash = trimmed.indexOf('#');
  if (hash === -1) return { kind: 'page', path: trimmed, fragment: null };
  return {
    kind: 'page',
    path: trimmed.slice(0, hash),
    fragment: trimmed.slice(hash + 1) || null,
  };
}
