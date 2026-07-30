<!--
  Preferences (S4-5, P5-007). ⌘, opens it.

  A sheet over the shell rather than a second window. Tauri can open one, but
  a second window needs its own webview, its own capability set and its own
  copy of the token CSS, and none of that buys anything for a panel with four
  tabs — the SearchModal established the pattern and this follows it.

  **Changes apply as you make them.** There is no OK button, because there is
  nothing to confirm: every preference here is reversible, visible
  immediately, and stored the moment it changes. A modal that could be
  cancelled would need a draft copy of every value and a way to roll back the
  live preview.

  Two tabs P5-007 asked for are deliberately absent:

  * **Sync** — there is no sync. ADR-0001 designs it, Stage 5 is deferred, and
    a tab of controls that do nothing is worse than no tab.
  * **Keyboard customisation** — shortcuts are not rebindable. The tab is a
    reference instead, and says so, rather than showing fields that discard
    what is typed into them.
-->
<script lang="ts">
  import {
    loadAppearance,
    saveAppearance,
    applyToDocument,
    type Appearance,
  } from '$lib/appearance';
  import { trapFocus } from '$lib/a11y';
  import { accelerator, pretty, unusableBecause } from '$lib/accelerator';
  import { SHORTCUTS } from '$lib/shortcuts';
  import {
    preferences,
    MEASURES,
    TEXT_SIZES,
    THEMES,
    type Measure,
    type TextSize,
    type Theme,
  } from '$lib/stores/preferences';
  import { setDockVisible, setGlobalShortcut, type LibraryLocation } from '$lib/tauri';

  interface Props {
    open: boolean;
    /** Where the library lives, for the Library tab. Null until it loads. */
    location?: LibraryLocation | null;
    /** Appearance changed. The shell forwards it to the reader frame. */
    onappearance?: (appearance: Appearance) => void;
    /** Why the global shortcut is not registered, if it is not. */
    shortcutError?: string | null;
    /** The shortcut was re-registered, successfully or not. */
    onshortcut?: (error: string | null) => void;
    onclose?: () => void;
  }

  let {
    open,
    location = null,
    shortcutError = null,
    onappearance,
    onshortcut,
    onclose,
  }: Props = $props();

  type Tab = 'appearance' | 'general' | 'library' | 'keyboard';
  const TABS: { id: Tab; label: string }[] = [
    { id: 'appearance', label: 'Appearance' },
    { id: 'general', label: 'General' },
    { id: 'library', label: 'Library' },
    { id: 'keyboard', label: 'Keyboard' },
  ];
  let tab = $state<Tab>('appearance');

  let appearance = $state<Appearance>(loadAppearance());
  let confirmBeforeRemove = $state(preferences.confirmBeforeRemove.load());
  let shortcutEnabled = $state(preferences.globalShortcutEnabled.load());
  let shortcut = $state(preferences.globalShortcut.load());
  let showInDock = $state(preferences.showInDock.load());
  /** True while the next keystroke is being captured as the new shortcut. */
  let recording = $state(false);
  /** Why the last captured keystroke was refused, if it was. */
  let rejected = $state<string | null>(null);

  let dialog = $state<HTMLElement>();

  // Focus moves into the sheet when it opens. Without this the sheet is
  // visible and the keyboard is still driving the shell behind it.
  $effect(() => {
    if (open) dialog?.focus();
  });

  function change(next: Partial<Appearance>): void {
    appearance = { ...appearance, ...next };
    saveAppearance(appearance);
    applyToDocument(appearance);
    onappearance?.(appearance);
  }

  /**
   * Register whatever the two shortcut controls currently say.
   *
   * Always both at once: enabling and rebinding are the same operation to
   * Rust, which is told the final accelerator or `null`. Doing them
   * separately would leave a window where the old combination is still live.
   */
  async function applyShortcut(): Promise<void> {
    preferences.globalShortcutEnabled.save(shortcutEnabled);
    preferences.globalShortcut.save(shortcut);
    try {
      await setGlobalShortcut(shortcutEnabled ? shortcut : null);
      onshortcut?.(null);
    } catch (e) {
      onshortcut?.(e instanceof Error ? e.message : String(e));
    }
  }

  function record(event: KeyboardEvent): void {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      recording = false;
      rejected = null;
      return;
    }
    const next = accelerator(event);
    // Null means "not a shortcut yet" — a modifier still being held down. Keep
    // listening rather than treating it as a rejection.
    if (!next) return;

    const why = unusableBecause(next);
    if (why) {
      // Stay armed. The user's next attempt is the fix, and closing the
      // recorder to show an error would make them re-open it.
      rejected = why;
      return;
    }
    shortcut = next;
    recording = false;
    rejected = null;
    void applyShortcut();
  }

  function reset(): void {
    change({
      theme: 'system',
      textSize: 'default',
      measure: 'default',
      lineNumbers: false,
    });
    confirmBeforeRemove = true;
    preferences.confirmBeforeRemove.save(true);
    shortcutEnabled = false;
    shortcut = 'CmdOrCtrl+Shift+D';
    showInDock = true;
    void applyShortcut();
    setDockVisible(true).catch(() => {});
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onclose?.();
    }
  }

  const THEME_LABELS: Record<Theme, string> = {
    system: 'System',
    light: 'Light',
    dark: 'Dark',
  };
  const SIZE_LABELS: Record<TextSize, string> = {
    small: 'Small',
    default: 'Default',
    large: 'Large',
    xlarge: 'Extra large',
  };
  const MEASURE_LABELS: Record<Measure, string> = {
    narrow: 'Narrow',
    default: 'Default',
    wide: 'Wide',
  };
</script>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) onclose?.();
    }}
  >
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="preferences-title"
      use:trapFocus
      tabindex="-1"
      bind:this={dialog}
      onkeydown={(e) => {
        // The recorder claims every keystroke while it is armed, including
        // Escape — which cancels recording rather than closing the sheet.
        if (recording) record(e);
        else keydown(e);
      }}
    >
      <header>
        <h2 id="preferences-title">Preferences</h2>
        <button class="close" onclick={onclose} aria-label="Close preferences">
          <span aria-hidden="true">×</span>
        </button>
      </header>

      <div class="tabs" role="tablist" aria-label="Preference categories">
        {#each TABS as t (t.id)}
          <button
            role="tab"
            id="tab-{t.id}"
            aria-selected={tab === t.id}
            aria-controls="panel-{t.id}"
            class:active={tab === t.id}
            onclick={() => (tab = t.id)}
          >
            {t.label}
          </button>
        {/each}
      </div>

      <div class="panel" role="tabpanel" id="panel-{tab}" aria-labelledby="tab-{tab}">
        {#if tab === 'appearance'}
          <div class="field">
            <span class="label" id="label-theme">Theme</span>
            <div class="choices" role="radiogroup" aria-labelledby="label-theme">
              {#each THEMES as value (value)}
                <button
                  role="radio"
                  aria-checked={appearance.theme === value}
                  class:selected={appearance.theme === value}
                  onclick={() => change({ theme: value })}
                >
                  {THEME_LABELS[value]}
                </button>
              {/each}
            </div>
            <p class="hint">
              System follows macOS. Changing it re-paints nothing — the theme is CSS, so it costs
              the same on a page with two code blocks or two hundred.
            </p>
          </div>

          <div class="field">
            <span class="label" id="label-size">Text size</span>
            <div class="choices" role="radiogroup" aria-labelledby="label-size">
              {#each TEXT_SIZES as value (value)}
                <button
                  role="radio"
                  aria-checked={appearance.textSize === value}
                  class:selected={appearance.textSize === value}
                  onclick={() => change({ textSize: value })}
                >
                  {SIZE_LABELS[value]}
                </button>
              {/each}
            </div>
            <p class="hint">Also ⌘= and ⌘-. Every size in the app is relative to this one.</p>
          </div>

          <div class="field">
            <span class="label" id="label-measure">Column width</span>
            <div class="choices" role="radiogroup" aria-labelledby="label-measure">
              {#each MEASURES as value (value)}
                <button
                  role="radio"
                  aria-checked={appearance.measure === value}
                  class:selected={appearance.measure === value}
                  onclick={() => change({ measure: value })}
                >
                  {MEASURE_LABELS[value]}
                </button>
              {/each}
            </div>
            <p class="hint">
              Measured in characters, so the column holds the same number of words at any text size.
            </p>
          </div>

          <label class="check">
            <input
              type="checkbox"
              checked={appearance.lineNumbers}
              onchange={(e) => change({ lineNumbers: e.currentTarget.checked })}
            />
            <span>Line numbers in code blocks</span>
          </label>
        {:else if tab === 'general'}
          <label class="check">
            <input
              type="checkbox"
              checked={confirmBeforeRemove}
              onchange={(e) => {
                confirmBeforeRemove = e.currentTarget.checked;
                preferences.confirmBeforeRemove.save(confirmBeforeRemove);
              }}
            />
            <span>Ask before removing a source</span>
          </label>
          <p class="hint">
            Removing deletes the source's pages, assets and index entries. The configuration goes
            last, so a failed removal can be run again.
          </p>

          <div class="field">
            <span class="label" id="label-shortcut">Global shortcut</span>
            <label class="check">
              <input
                type="checkbox"
                checked={shortcutEnabled}
                onchange={(e) => {
                  shortcutEnabled = e.currentTarget.checked;
                  void applyShortcut();
                }}
              />
              <span>Summon Tome from anywhere</span>
            </label>
            <div class="choices">
              <button
                class:selected={recording}
                aria-labelledby="label-shortcut"
                disabled={!shortcutEnabled}
                onclick={() => (recording = !recording)}
              >
                {recording ? 'Press keys…' : pretty(shortcut)}
              </button>
            </div>
            {#if rejected}
              <p class="error" role="alert">{rejected}</p>
            {:else if shortcutError}
              <p class="error" role="alert">{shortcutError}</p>
            {:else}
              <p class="hint">
                Off by default: a system-wide shortcut claimed without asking is one taken from
                whatever you had bound to it. Use at least two modifiers, and avoid combinations
                macOS reserves — those register successfully and then never fire, which is the worst
                way to find out.
              </p>
            {/if}
          </div>

          <label class="check">
            <input
              type="checkbox"
              checked={showInDock}
              onchange={(e) => {
                showInDock = e.currentTarget.checked;
                preferences.showInDock.save(showInDock);
                setDockVisible(showInDock).catch(() => {});
              }}
            />
            <span>Show in the Dock</span>
          </label>
          <p class="hint">
            Off makes Tome menu-bar-only. The menu bar item is always there, so turning this off
            never leaves you without a way back in.
          </p>

          <div class="field">
            <span class="label">Updates</span>
            <p class="hint">
              Tome does not check for updates, and collects nothing. Upgrade with
              <code>brew upgrade --cask tome</code>.
            </p>
          </div>
        {:else if tab === 'library'}
          {#if location}
            <div class="field">
              <span class="label">Library</span>
              <code class="path selectable">{location.state_root}</code>
              <p class="hint">Configurations, the database, and logs. Back this up.</p>
            </div>
            <div class="field">
              <span class="label">Cache</span>
              <code class="path selectable">{location.cache_root}</code>
              <p class="hint">
                Fetched pages, assets and the search index. Safe to delete — a rebuild needs no
                network.
              </p>
            </div>
            <div class="field">
              <span class="label">Version</span>
              <code class="path selectable">{location.version} · {location.bundle_id}</code>
            </div>
          {:else}
            <p class="hint">Loading…</p>
          {/if}
          <p class="hint">
            <code>$TOME_HOME</code> overrides both. The <code>tome</code> command line tool reads
            the same library — run <code>tome status</code> to see it agree.
          </p>
        {:else}
          <p class="hint">
            Shortcuts are not customisable. This is what is bound today; the full plan, including
            shortcuts for features that do not exist yet, is in the PRD.
          </p>
          {#each SHORTCUTS as group (group.title)}
            <div class="field">
              <span class="label">{group.title}</span>
              <dl>
                {#each group.shortcuts as shortcut (shortcut.keys)}
                  <dt><kbd>{shortcut.keys}</kbd></dt>
                  <dd>{shortcut.action}</dd>
                {/each}
              </dl>
            </div>
          {/each}
        {/if}
      </div>

      <footer>
        <button class="quiet" onclick={reset}>Reset to defaults</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 4rem var(--space-4) var(--space-4);
    /* The same literal SearchModal uses, deliberately not a token: a scrim is
       not a palette colour — it is opacity over whatever is behind it, and it
       is identical in both themes. A token would imply it varies. */
    background: rgb(0 0 0 / 40%);
  }

  .sheet {
    display: flex;
    flex-direction: column;
    width: min(38rem, 100%);
    max-height: 100%;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    background: var(--color-bg-secondary);
    box-shadow: var(--shadow-lg);
    font-family: var(--font-ui);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }

  h2 {
    margin: 0;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--color-text-primary);
  }

  .close {
    width: 1.6rem;
    height: 1.6rem;
    border-radius: var(--radius-sm);
    font-size: var(--text-lg);
    line-height: 1;
    color: var(--color-text-secondary);
  }

  .close:hover {
    background: var(--color-bg-tertiary);
    color: var(--color-text-primary);
  }

  .tabs {
    display: flex;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-4) 0;
    border-bottom: 1px solid var(--color-border);
  }

  .tabs button {
    padding: var(--space-2) var(--space-3);
    border-bottom: 2px solid transparent;
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
  }

  .tabs button.active {
    border-bottom-color: var(--color-accent);
    color: var(--color-text-primary);
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4);
    overflow-y: auto;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .label {
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-text-secondary);
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .choices button {
    padding: var(--space-1) var(--space-3);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
  }

  .choices button.selected {
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: var(--color-text-inverse);
  }

  .check {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
    color: var(--color-text-primary);
  }

  .error {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    background: var(--color-bg-tertiary);
    color: var(--color-error-text);
    font-size: var(--text-xs);
    line-height: var(--leading-normal);
  }

  .hint {
    margin: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-normal);
    color: var(--color-text-tertiary);
  }

  code {
    font-family: var(--font-mono);
    font-size: 0.92em;
    background: var(--color-code-bg);
    padding: 0.1em 0.3em;
    border-radius: var(--radius-sm);
  }

  .path {
    display: block;
    overflow-x: auto;
    white-space: nowrap;
    padding: var(--space-2);
    color: var(--color-text-primary);
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

  footer {
    display: flex;
    justify-content: flex-end;
    padding: var(--space-3) var(--space-4);
    border-top: 1px solid var(--color-border);
  }

  footer button {
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    color: var(--color-text-secondary);
  }

  footer button:hover {
    background: var(--color-bg-tertiary);
    color: var(--color-text-primary);
  }
</style>
