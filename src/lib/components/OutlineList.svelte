<!--
  One level of the outline tree, recursing into its children (S1-14, P1-019).

  Split from `Outline.svelte` rather than written as a nested `{#each}`
  because the tree is arbitrarily deep — Sphinx API pages nest four levels
  routinely — and a component that renders itself is the only way to express
  that without a fixed unroll.

  Indentation stops at `maxDepth`. Past it the entries are still listed and
  still clickable; they just stop marching off the right edge of a 200px
  sidebar, which is P1-019's "collapse deeply nested items".
-->
<script lang="ts">
  import type { OutlineEntry } from '$lib/tauri';
  import Self from './OutlineList.svelte';

  interface Props {
    entries: OutlineEntry[];
    depth: number;
    maxDepth: number;
    activeId: string | null;
    onselect: (id: string) => void;
  }

  let { entries, depth, maxDepth, activeId, onselect }: Props = $props();

  const indent = $derived(Math.min(depth, maxDepth));
</script>

<ul>
  {#each entries as entry (entry.id)}
    <li>
      <button
        class:active={entry.id === activeId}
        style:padding-left="calc(var(--space-3) + {indent} * var(--space-3))"
        aria-current={entry.id === activeId ? 'location' : undefined}
        onclick={() => onselect(entry.id)}
        title={entry.title}
      >
        {entry.title}
      </button>
      {#if entry.children.length > 0}
        <Self entries={entry.children} depth={depth + 1} {maxDepth} {activeId} {onselect} />
      {/if}
    </li>
  {/each}
</ul>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  button {
    display: block;
    width: 100%;
    text-align: left;
    padding-block: var(--space-1);
    padding-right: var(--space-3);
    color: var(--color-text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: default;
    /* The active marker is a rule on the leading edge; the transparent one
       here keeps the text from shifting by 2px when it appears. */
    box-shadow: inset 2px 0 0 transparent;
  }

  button:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-tertiary);
  }

  /* Weight and a rule as well as colour: the design system's rule is that
     colour is never the only carrier of meaning. */
  .active {
    color: var(--color-text-primary);
    font-weight: 600;
    box-shadow: inset 2px 0 0 var(--color-accent);
  }
</style>
