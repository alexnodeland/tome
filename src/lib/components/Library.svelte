<!--
  The library sidebar (S1-14, P1-018).

  Sources grouped by category, each expanding to its pages. The plan's ticket
  describes a source list; a source list alone is not navigable, because
  choosing a source still leaves the question of which of its 5 000 pages to
  open. So the tree goes one level deeper and the filter searches both.

  The filter is the reason `$lib/keys.ts` exists: a text input in the sidebar
  is exactly what makes the PRD's "single-letter reading keys must not fire
  while typing" rule load-bearing rather than theoretical.
-->
<script lang="ts">
  import { SvelteSet } from 'svelte/reactivity';
  import type { PageSummary, SourceSummary } from '$lib/tauri';

  interface Props {
    sources: SourceSummary[];
    /** Pages of the expanded source, or `[]`. Loaded lazily by the shell. */
    pages: PageSummary[];
    selectedSource: string | null;
    selectedPage: string | null;
    onselectsource: (id: string) => void;
    onselectpage: (path: string) => void;
  }

  let { sources, pages, selectedSource, selectedPage, onselectsource, onselectpage }: Props =
    $props();

  let filter = $state('');
  // `SvelteSet`, not `Set`: a plain Set has to be copied and reassigned on
  // every change for Svelte to notice, which is a copy of the whole set per
  // click and easy to get wrong. This one is reactive in place.
  const collapsed = new SvelteSet<string>();

  const needle = $derived(filter.trim().toLowerCase());

  const visibleSources = $derived(
    needle === ''
      ? sources
      : sources.filter(
          (source) =>
            source.name.toLowerCase().includes(needle) ||
            source.id.toLowerCase().includes(needle) ||
            source.category.toLowerCase().includes(needle),
        ),
  );

  /**
   * Category → sources. A `Map` rather than an object literal: category names
   * come from source configurations, and an object would let one named
   * `__proto__` or `constructor` do something surprising.
   *
   * A plain `Map` rather than `SvelteMap`, which is what
   * `svelte/prefer-svelte-reactivity` asks for: this one is a throwaway local
   * inside a `$derived.by`, rebuilt from scratch every time the derivation
   * runs. Nothing observes a mutation of it, so reactive-collection overhead
   * would buy nothing.
   */
  const grouped = $derived.by(() => {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- see above
    const groups = new Map<string, SourceSummary[]>();
    for (const source of visibleSources) {
      const category = source.category.trim() || 'Uncategorized';
      const existing = groups.get(category);
      if (existing) existing.push(source);
      else groups.set(category, [source]);
    }
    return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
  });

  /**
   * When the filter matches nothing in the source names, fall back to
   * matching page titles inside the open source. Typing a page title into a
   * box next to a list of pages should find the page.
   */
  const visiblePages = $derived(
    needle === '' || visibleSources.some((s) => s.id === selectedSource)
      ? pages
      : pages.filter(
          (page) =>
            page.title.toLowerCase().includes(needle) || page.path.toLowerCase().includes(needle),
        ),
  );

  function toggleCategory(category: string): void {
    if (collapsed.has(category)) collapsed.delete(category);
    else collapsed.add(category);
  }

  /**
   * Up/down through whatever is currently rendered.
   *
   * Reads the DOM rather than maintaining a parallel index of visible rows.
   * The rendered order already accounts for the filter, the collapsed
   * categories, and the expanded source; a second model of the same thing is
   * a second thing to get out of step.
   */
  function arrowNavigate(event: KeyboardEvent): void {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    const root = (event.currentTarget as HTMLElement).closest('.library');
    if (!root) return;
    const items = [...root.querySelectorAll<HTMLElement>('[data-nav]')];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const next = event.key === 'ArrowDown' ? at + 1 : at - 1;
    const target = items[Math.min(items.length - 1, Math.max(0, next))];
    if (target) {
      event.preventDefault();
      target.focus();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- A <nav> with lists and buttons, not role="tree". The ARIA tree pattern
     requires roving tabindex and treeitem roles, and it buys nothing here:
     the rows are real <button>s, so they are already focusable, operable by
     Enter and Space, and announced as controls. Arrow-key navigation is added
     on top rather than replacing what the browser gives for free. The
     keydown listener sits on the container because arrow keys move focus
     BETWEEN the buttons; putting it on each button would be the same handler
     copied N times. That is what the ignore above is for. -->
<nav class="library" onkeydown={arrowNavigate} aria-label="Documentation sources">
  <div class="filter">
    <input
      type="search"
      bind:value={filter}
      placeholder="Filter"
      aria-label="Filter sources and pages"
    />
  </div>

  {#if sources.length === 0}
    <p class="empty">No sources yet.</p>
  {:else if grouped.length === 0 && visiblePages.length === 0}
    <p class="empty">Nothing matches “{filter}”.</p>
  {/if}

  {#each grouped as [category, items] (category)}
    <section>
      <button
        class="category"
        data-nav
        aria-expanded={!collapsed.has(category)}
        onclick={() => toggleCategory(category)}
      >
        <span class="chevron" class:closed={collapsed.has(category)} aria-hidden="true">▾</span>
        {category}
      </button>

      {#if !collapsed.has(category)}
        <ul>
          {#each items as source (source.id)}
            <li>
              <button
                class="source"
                class:selected={source.id === selectedSource}
                data-nav
                aria-current={source.id === selectedSource ? 'true' : undefined}
                onclick={() => onselectsource(source.id)}
              >
                <span class="name">{source.name}</span>
                <span class="count" aria-label="{source.page_count} pages">
                  {source.page_count}
                </span>
              </button>

              {#if source.id === selectedSource}
                <ul class="pages">
                  {#each visiblePages as page (page.path)}
                    <li>
                      <button
                        class="page"
                        class:selected={page.path === selectedPage}
                        data-nav
                        aria-current={page.path === selectedPage ? 'page' : undefined}
                        onclick={() => onselectpage(page.path)}
                        title={page.path}
                      >
                        {page.title}
                      </button>
                    </li>
                  {/each}
                  {#if visiblePages.length === 0}
                    <li class="empty page-empty">No pages.</li>
                  {/if}
                </ul>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}
</nav>

<style>
  .library {
    font-family: var(--font-ui);
    font-size: var(--text-sm);
    padding-bottom: var(--space-4);
  }

  .filter {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: var(--space-3);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  section {
    margin-top: var(--space-3);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  button {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    text-align: left;
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-sm);
    cursor: default;
  }

  button:hover {
    background: var(--color-bg-tertiary);
  }

  .category {
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-text-tertiary);
  }

  .chevron {
    display: inline-block;
    transition: transform var(--duration-fast) var(--ease-standard);
  }
  .chevron.closed {
    transform: rotate(-90deg);
  }

  .source {
    padding-left: var(--space-5);
    color: var(--color-text-primary);
  }

  .name {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    flex: 0 0 auto;
    font-size: var(--text-xs);
    color: var(--color-text-tertiary);
    font-variant-numeric: tabular-nums;
  }

  .page {
    padding-left: var(--space-8);
    color: var(--color-text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: block;
  }

  /* The selected row is marked by weight and a rule as well as by colour.
     The design system's rule: colour is never the only carrier of meaning. */
  .selected {
    background: var(--color-bg-tertiary);
    color: var(--color-text-primary);
    font-weight: 600;
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  .empty {
    margin: var(--space-4) var(--space-3);
    color: var(--color-text-tertiary);
    font-size: var(--text-sm);
  }

  .page-empty {
    margin-left: var(--space-8);
  }
</style>
