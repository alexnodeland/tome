<!--
  Test harness for `Layout` (S1-14).

  `Layout` takes three snippets, and snippets are a compile-time construct — a
  `.ts` test cannot build one. This supplies them, and exposes the toggles as
  buttons so the test can drive them through the DOM rather than by reaching
  into the component instance.
-->
<script lang="ts">
  import Layout from './Layout.svelte';

  let layout = $state<Layout>();
</script>

<button onclick={() => layout?.toggleLeft()}>toggle left</button>
<button onclick={() => layout?.toggleRight()}>toggle right</button>
<button onclick={() => layout?.toggleBoth()}>toggle both</button>

<Layout bind:this={layout}>
  {#snippet left()}
    <p>library</p>
  {/snippet}
  {#snippet main()}
    <p>reader</p>
  {/snippet}
  {#snippet right()}
    <p>outline</p>
  {/snippet}
</Layout>
