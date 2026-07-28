<script lang="ts">
  import { onMount } from 'svelte';
  import { libraryLocation, type LibraryLocation } from '$lib/tauri';

  let location = $state<LibraryLocation | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      location = await libraryLocation();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  });
</script>

<main>
  <h1>Tome</h1>
  <p class="tagline">A personal library for technical documentation.</p>

  <section aria-labelledby="status-heading">
    <h2 id="status-heading">Scaffold</h2>

    {#if error}
      <p class="error" role="alert">Could not read the library location: {error}</p>
    {:else if location}
      <dl>
        <dt>Version</dt>
        <dd>{location.version}</dd>
        <dt>Bundle</dt>
        <dd><code>{location.bundle_id}</code></dd>
        <dt>State</dt>
        <dd><code>{location.state_root}</code></dd>
        <dt>Cache</dt>
        <dd><code>{location.cache_root}</code></dd>
        <dt>Status</dt>
        <dd>{location.initialised ? 'initialised' : 'not yet initialised'}</dd>
      </dl>
      <p class="note">
        The <code>tome</code> CLI resolves these same paths. That is the point of the shared
        <code>tome-core</code> crate — see <code>docs/decisions/0002-no-app-sandbox.md</code>.
      </p>
    {:else}
      <p aria-live="polite">Loading…</p>
    {/if}
  </section>
</main>

<style>
  main {
    max-width: 42rem;
    margin: 0 auto;
    padding: var(--space-6) var(--space-5);
  }
  h1 {
    font-size: var(--text-2xl);
    font-weight: 600;
    letter-spacing: -0.02em;
    margin: 0;
  }
  .tagline {
    font-family: var(--font-body);
    color: var(--color-text-secondary);
    margin: var(--space-2) 0 var(--space-6);
  }
  h2 {
    font-size: var(--text-lg);
    font-weight: 600;
    margin: 0 0 var(--space-4);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    margin: 0;
  }
  dt {
    color: var(--color-text-tertiary);
    font-size: var(--text-sm);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  code {
    font-family: var(--font-mono);
    font-size: 0.9em;
    background: var(--color-code-bg);
    padding: 0.1em 0.3em;
    border-radius: 3px;
  }
  .note {
    margin-top: var(--space-5);
    padding-top: var(--space-4);
    border-top: 1px solid var(--color-border);
    color: var(--color-text-tertiary);
    font-size: var(--text-sm);
  }
  .error {
    /* Token, not a literal: #FF3B30 measures 3.4:1 on the page background and
       is an indicator fill, not a text colour. See scripts/check-contrast.mjs. */
    color: var(--color-error-text);
  }
</style>
