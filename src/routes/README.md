# Routes Directory

Page-level components and routing configuration.

## What Belongs Here

- **Page components** (`.svelte` files) that represent full views
- **Layout components** for shared page structure
- **Route-specific logic** and data loading

## What Does NOT Belong Here

- Reusable components (use `lib/components/`)
- Business logic (use `lib/stores/` or `lib/services/`)
- Utility functions (use `lib/utils/`)

## Naming Conventions

Following SvelteKit conventions:
- `+page.svelte` - Page component
- `+layout.svelte` - Layout wrapper
- `+page.ts` - Page data loading
- `+error.svelte` - Error page

## Directory Structure

```
routes/
├── +layout.svelte        # Root layout (three-panel shell)
├── +page.svelte          # Home page (library view)
├── +error.svelte         # Global error page
├── reader/
│   └── [sourceId]/
│       └── [...path]/
│           └── +page.svelte    # Reader view
├── search/
│   └── +page.svelte      # Search results page
├── bookmarks/
│   └── +page.svelte      # Bookmarks page
└── settings/
    └── +page.svelte      # Settings page
```

## Page Component Pattern

```svelte
<!-- +page.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { sources, sourcesStore } from '$lib/stores/sources';
  import { SourceList, SourceItem } from '$lib/components';

  // Load data on mount
  onMount(() => {
    sourcesStore.load();
  });

  // Reactive to route params
  $: sourceId = $page.params.sourceId;
</script>

<svelte:head>
  <title>Library - Tome</title>
</svelte:head>

<div class="library-page">
  <header class="page-header">
    <h1>Library</h1>
  </header>

  <main class="page-content">
    <SourceList sources={$sources} />
  </main>
</div>

<style>
  .library-page {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .page-header {
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }

  .page-content {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-4);
  }
</style>
```

## Layout Pattern

```svelte
<!-- +layout.svelte -->
<script lang="ts">
  import { page } from '$app/stores';
  import { ThreePanel, Sidebar, TOC } from '$lib/components';
  import { leftSidebarOpen, rightSidebarOpen } from '$lib/stores/ui';

  // Keyboard shortcuts for sidebar toggle
  function handleKeydown(e: KeyboardEvent) {
    if (e.metaKey && e.key === '1') {
      e.preventDefault();
      leftSidebarOpen.toggle();
    }
    if (e.metaKey && e.key === '2') {
      e.preventDefault();
      rightSidebarOpen.toggle();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<ThreePanel
  leftOpen={$leftSidebarOpen}
  rightOpen={$rightSidebarOpen}
>
  <Sidebar slot="left" />

  <slot />

  <TOC slot="right" />
</ThreePanel>
```

## Data Loading Pattern

```typescript
// +page.ts
import type { PageLoad } from './$types';
import { getSource } from '$lib/services/sources';
import { error } from '@sveltejs/kit';

export const load: PageLoad = async ({ params }) => {
  const result = await getSource(params.sourceId);

  if (!result.ok) {
    throw error(404, {
      message: `Source not found: ${params.sourceId}`,
    });
  }

  return {
    source: result.value,
  };
};
```

## Architectural Rules

1. Routes **can import from** all `lib/` directories
2. Routes **cannot be imported by** any other code
3. Keep page components **thin** - delegate to lib components
4. Use **layouts** for shared structure
5. Handle **loading states** at the page level
6. Handle **errors** with proper error pages
