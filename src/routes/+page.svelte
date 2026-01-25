<script lang="ts">
  import { onMount } from 'svelte';

  let version = 'Loading...';

  onMount(() => {
    // In real app, this would use our service layer to call Tauri
    // For now, just set placeholder version
    version = '0.1.0';
  });
</script>

<svelte:head>
  <title>Tome - Documentation Reader</title>
</svelte:head>

<main class="home">
  <div class="welcome">
    <h1 class="title">Welcome to Tome</h1>
    <p class="subtitle">Your personal library for technical documentation</p>
    <p class="version">Version {version}</p>

    <div class="quick-start">
      <h2>Quick Start</h2>
      <ol>
        <li>Create a source configuration in <code>~/.tome/sources/</code></li>
        <li>Tome will detect and sync your documentation</li>
        <li>
          Press <kbd>Cmd</kbd>+<kbd>K</kbd> to search across all sources
        </li>
      </ol>
    </div>

    <div class="shortcuts">
      <h2>Keyboard Shortcuts</h2>
      <ul>
        <li><kbd>Cmd</kbd>+<kbd>K</kbd> — Global search</li>
        <li><kbd>Cmd</kbd>+<kbd>D</kbd> — Bookmark page</li>
        <li><kbd>Cmd</kbd>+<kbd>1</kbd> — Toggle sources sidebar</li>
        <li><kbd>Cmd</kbd>+<kbd>2</kbd> — Toggle TOC sidebar</li>
        <li><kbd>J</kbd>/<kbd>K</kbd> — Scroll down/up</li>
      </ul>
    </div>
  </div>
</main>

<style>
  .home {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-6);
  }

  .welcome {
    max-width: 600px;
    text-align: center;
  }

  .title {
    font-family: var(--font-heading);
    font-size: var(--text-3xl);
    font-weight: 600;
    margin-bottom: var(--space-2);
    color: var(--color-text-primary);
  }

  .subtitle {
    font-family: var(--font-body);
    font-size: var(--text-lg);
    color: var(--color-text-secondary);
    margin-bottom: var(--space-2);
  }

  .version {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--color-text-tertiary);
    margin-bottom: var(--space-8);
  }

  .quick-start,
  .shortcuts {
    text-align: left;
    margin-bottom: var(--space-6);
  }

  h2 {
    font-family: var(--font-heading);
    font-size: var(--text-lg);
    font-weight: 600;
    margin-bottom: var(--space-3);
    color: var(--color-text-primary);
  }

  ol,
  ul {
    margin-left: var(--space-5);
  }

  li {
    margin-bottom: var(--space-2);
    color: var(--color-text-secondary);
  }

  kbd {
    display: inline-block;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    padding: var(--space-1) var(--space-2);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    box-shadow: 0 1px 0 var(--color-border-strong);
  }

  code {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    background: var(--color-code-bg);
    padding: var(--space-1) var(--space-2);
    border-radius: 4px;
  }
</style>
