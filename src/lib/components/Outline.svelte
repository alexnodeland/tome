<!--
  The page outline sidebar (S1-14, P1-019).

  The headings come from the Rust renderer, which derives an id for any
  heading that lacks one — so every entry here has somewhere to go. That is
  not a detail: the original sanitizer draft stripped `id`, which would have
  left this sidebar full of links to nowhere, and it is why the renderer
  guarantees an anchor rather than hoping for one.

  The active entry is tracked by the *frame*, not by this component. The app
  webview cannot read the reader's document — that is the whole point of the
  sandbox — so the frame computes which heading is current and posts it back
  (`public/reader-frame.js`). This component only draws the answer.
-->
<script lang="ts">
  import type { OutlineEntry } from '$lib/tauri';
  import OutlineList from './OutlineList.svelte';

  interface Props {
    outline: OutlineEntry[];
    activeId: string | null;
    onselect: (id: string) => void;
  }

  let { outline, activeId, onselect }: Props = $props();

  /** Entries below this depth collapse into their parent's indentation. */
  const MAX_INDENT_DEPTH = 3;
</script>

<nav class="outline" aria-label="On this page">
  <h2>On this page</h2>
  {#if outline.length === 0}
    <p class="empty">No headings.</p>
  {:else}
    <OutlineList entries={outline} depth={0} maxDepth={MAX_INDENT_DEPTH} {activeId} {onselect} />
  {/if}
</nav>

<style>
  .outline {
    font-family: var(--font-ui);
    font-size: var(--text-sm);
    padding: var(--space-3) 0 var(--space-6);
  }

  h2 {
    margin: 0 0 var(--space-2);
    padding: 0 var(--space-3);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-text-tertiary);
  }

  .empty {
    margin: 0;
    padding: 0 var(--space-3);
    color: var(--color-text-tertiary);
  }
</style>
