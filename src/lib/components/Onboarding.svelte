<!--
  First run (S4-4, P5-006).

  Three steps and a way out of all of them. The middle step is the ticket:
  "first source installed from the registry in one click — not by
  hand-writing YAML. This is the single highest-leverage thing in onboarding:
  it is the difference between a product and a configuration exercise."

  Two rules shape the rest:

  * **Onboarding never blocks the app.** Skip is always available, the shell
    is behind it, and dismissing is remembered — someone who removes their
    last source has not become a first-time user again.
  * **It works with no network.** The catalogue is in the bundle, so the list
    renders offline; only installing needs the network, and a failure there
    says so rather than leaving a spinner.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { SHORTCUTS } from '$lib/shortcuts';
  import {
    installRegistrySource,
    onInstallProgress,
    registryCatalogue,
    type CatalogueEntry,
    type InstallProgress,
  } from '$lib/tauri';

  interface Props {
    /** Called when a source finished installing, so the shell can reload. */
    oninstalled?: (sourceId: string) => void;
    /** Called when onboarding is finished or skipped. */
    onclose?: () => void;
  }

  let { oninstalled, onclose }: Props = $props();

  type Step = 'welcome' | 'source' | 'shortcuts';
  let step = $state<Step>('welcome');

  let catalogue = $state<CatalogueEntry[]>([]);
  let catalogueError = $state<string | null>(null);
  let loadingCatalogue = $state(true);

  let installing = $state<string | null>(null);
  let progress = $state<InstallProgress | null>(null);
  let installError = $state<string | null>(null);
  let installed = $state<string[]>([]);

  onMount(() => {
    void load();
    // The unlisten function arrives asynchronously, so the cleanup has to wait
    // for it rather than capture it — returning the promise from onMount would
    // make Svelte treat it as the cleanup itself.
    let unlisten: (() => void) | undefined;
    void onInstallProgress((p) => {
      progress = p;
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        // No event bus (a browser dev server, a test) means no progress
        // display. The install still works and still reports its result.
      });
    return () => unlisten?.();
  });

  async function load(): Promise<void> {
    try {
      catalogue = await registryCatalogue();
    } catch (e) {
      catalogueError = e instanceof Error ? e.message : String(e);
    } finally {
      loadingCatalogue = false;
    }
  }

  async function install(entry: CatalogueEntry): Promise<void> {
    installing = entry.id;
    installError = null;
    progress = null;
    try {
      const report = await installRegistrySource(entry.id);
      installed = [...installed, entry.id];
      catalogue = catalogue.map((c) => (c.id === entry.id ? { ...c, installed: true } : c));
      oninstalled?.(report.source_id);
      step = 'shortcuts';
    } catch (e) {
      installError = e instanceof Error ? e.message : String(e);
    } finally {
      installing = null;
      progress = null;
    }
  }

  function phaseLabel(p: InstallProgress): string {
    switch (p.phase) {
      case 'crawling':
        // No denominator: the crawler does not know how many pages a site has
        // until it has finished finding them.
        return `Fetching — ${p.done} page${p.done === 1 ? '' : 's'} so far`;
      case 'storing':
        return `Saving — ${p.done} of ${p.total}`;
      case 'indexing':
        return `Indexing — ${p.done} of ${p.total}`;
    }
  }

  const grouped = $derived(
    Object.entries(
      catalogue.reduce<Record<string, CatalogueEntry[]>>((groups, entry) => {
        (groups[entry.category] ??= []).push(entry);
        return groups;
      }, {}),
    ),
  );
</script>

<section class="onboarding" aria-labelledby="onboarding-title">
  {#if step === 'welcome'}
    <h2 id="onboarding-title">Tome keeps documentation on your machine.</h2>
    <p class="lede">
      Point it at a documentation site and it fetches, indexes and typesets the whole thing. After
      that it works with no network, and your coding agent can read it too.
    </p>
    <div class="actions">
      <button class="primary" onclick={() => (step = 'source')}>Add your first source</button>
      <button class="quiet" onclick={onclose}>Skip</button>
    </div>
  {:else if step === 'source'}
    <h2 id="onboarding-title">Pick something to read.</h2>
    <p class="lede">
      These are ready-made configurations. Tome fetches the documentation from its own site — it
      hosts none of it.
    </p>

    {#if loadingCatalogue}
      <p class="notice" aria-live="polite">Loading…</p>
    {:else if catalogueError}
      <p class="error" role="alert">{catalogueError}</p>
    {:else if catalogue.length === 0}
      <p class="notice">The bundled catalogue is empty.</p>
    {:else}
      <div class="catalogue">
        {#each grouped as [category, entries] (category)}
          <h3>{category}</h3>
          <ul>
            {#each entries as entry (entry.id)}
              <li>
                <div class="entry">
                  <span class="name">{entry.name}</span>
                  <span class="meta">{entry.licence} · verified {entry.verified}</span>
                </div>
                {#if entry.installed}
                  <span class="installed">Installed</span>
                {:else}
                  <button
                    class="primary small"
                    disabled={installing !== null}
                    onclick={() => install(entry)}
                  >
                    {installing === entry.id ? 'Installing…' : 'Install'}
                  </button>
                {/if}
              </li>
            {/each}
          </ul>
        {/each}
      </div>
    {/if}

    {#if installing}
      <!-- A first pull is minutes, which P5-006 requires to be visible. -->
      <p class="progress" aria-live="polite">
        {progress ? phaseLabel(progress) : 'Starting…'}
      </p>
    {/if}
    {#if installError}
      <p class="error" role="alert">{installError}</p>
    {/if}

    <div class="actions">
      <button class="quiet" onclick={() => (step = 'welcome')} disabled={installing !== null}>
        Back
      </button>
      <button class="quiet" onclick={onclose} disabled={installing !== null}>Skip</button>
    </div>
  {:else}
    <h2 id="onboarding-title">
      {installed.length > 0 ? 'Ready.' : 'Here is how to get around.'}
    </h2>
    <div class="shortcuts">
      {#each SHORTCUTS as group (group.title)}
        <div class="group">
          <h3>{group.title}</h3>
          <dl>
            {#each group.shortcuts as shortcut (shortcut.keys)}
              <dt><kbd>{shortcut.keys}</kbd></dt>
              <dd>{shortcut.action}</dd>
            {/each}
          </dl>
        </div>
      {/each}
    </div>
    <div class="actions">
      <button class="primary" onclick={onclose}>Start reading</button>
    </div>
  {/if}
</section>

<style>
  .onboarding {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 42rem;
    margin: 0 auto;
    padding: var(--space-6) var(--space-4);
    font-family: var(--font-ui);
    overflow-y: auto;
  }

  h2 {
    margin: 0;
    font-family: var(--font-heading);
    font-size: var(--text-xl);
    font-weight: 600;
    color: var(--color-text-primary);
  }

  h3 {
    margin: var(--space-3) 0 var(--space-1);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-text-secondary);
  }

  .lede {
    margin: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-relaxed);
    color: var(--color-text-secondary);
  }

  .catalogue ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .catalogue li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--color-border);
  }

  .entry {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .name {
    font-size: var(--text-sm);
    color: var(--color-text-primary);
  }

  .meta {
    font-size: var(--text-xs);
    color: var(--color-text-tertiary);
  }

  .installed {
    font-size: var(--text-xs);
    color: var(--color-text-tertiary);
  }

  .actions {
    display: flex;
    gap: var(--space-2);
    align-items: center;
  }

  button {
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    font-family: var(--font-ui);
    font-size: var(--text-sm);
  }

  button.small {
    padding: var(--space-1) var(--space-3);
  }

  button.primary {
    background: var(--color-accent);
    color: var(--color-text-inverse);
  }

  button.primary:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }

  button.quiet {
    color: var(--color-text-secondary);
  }

  button.quiet:hover:not(:disabled) {
    background: var(--color-bg-tertiary);
    color: var(--color-text-primary);
  }

  button:disabled {
    opacity: 0.45;
  }

  .notice,
  .progress {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
  }

  .error {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    background: var(--color-bg-tertiary);
    color: var(--color-error-text);
    font-size: var(--text-sm);
  }

  .shortcuts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: var(--space-4);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-3);
    margin: 0;
    align-items: baseline;
  }

  dt {
    justify-self: start;
  }

  dd {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
  }

  kbd {
    display: inline-block;
    min-width: 2.2rem;
    padding: 0.1rem 0.35rem;
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-sm);
    background: var(--color-bg-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-align: center;
    color: var(--color-text-primary);
  }
</style>
