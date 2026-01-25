# Components Directory

Reusable UI components for the Tome application.

## What Belongs Here

- **Svelte components** (`.svelte` files) that are reusable across multiple pages
- **Component tests** (`.test.ts` files) adjacent to their component
- **Component-specific types** if not shared elsewhere

## What Does NOT Belong Here

- Page-level components (use `routes/` instead)
- Business logic (use `stores/` or `services/`)
- Utility functions (use `utils/`)
- Global styles (use `app.css`)

## Naming Conventions

- Component files: `PascalCase.svelte` (e.g., `SearchResults.svelte`)
- Test files: `ComponentName.test.ts` (e.g., `SearchResults.test.ts`)
- Export all public components from `index.ts`

## Component Structure

```svelte
<script lang="ts">
  // 1. Imports
  import { createEventDispatcher } from 'svelte';
  import type { SearchResult } from '$lib/types';

  // 2. Props (with defaults where appropriate)
  export let results: SearchResult[] = [];
  export let loading = false;

  // 3. Local state
  let selectedIndex = 0;

  // 4. Computed values
  $: hasResults = results.length > 0;

  // 5. Event handlers
  const dispatch = createEventDispatcher<{ select: SearchResult }>();

  function handleSelect(result: SearchResult) {
    dispatch('select', result);
  }
</script>

<!-- Template -->
<div class="search-results" class:loading>
  {#each results as result, i}
    <button
      class="result"
      class:selected={i === selectedIndex}
      on:click={() => handleSelect(result)}
    >
      {result.title}
    </button>
  {/each}
</div>

<style>
  /* Component-scoped styles using CSS variables from design system */
  .search-results {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .result {
    padding: var(--space-2) var(--space-3);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    text-align: left;
    cursor: pointer;
  }

  .result:hover,
  .result.selected {
    background: var(--color-bg-tertiary);
  }
</style>
```

## Testing Pattern

```typescript
// SearchResults.test.ts
import { render, screen, fireEvent } from '@testing-library/svelte';
import SearchResults from './SearchResults.svelte';

describe('SearchResults', () => {
  it('renders results when provided', () => {
    const results = [{ id: '1', title: 'Test Result' }];
    render(SearchResults, { props: { results } });
    expect(screen.getByText('Test Result')).toBeInTheDocument();
  });

  it('dispatches select event on click', async () => {
    const results = [{ id: '1', title: 'Test Result' }];
    const { component } = render(SearchResults, { props: { results } });

    const selectHandler = vi.fn();
    component.$on('select', selectHandler);

    await fireEvent.click(screen.getByText('Test Result'));
    expect(selectHandler).toHaveBeenCalledWith(
      expect.objectContaining({ detail: results[0] })
    );
  });
});
```

## Architectural Rules

1. Components **cannot import from** `routes/`
2. Components **can import from** `stores/`, `services/`, `utils/`, `types/`
3. Keep components focused - if it's doing too much, split it
4. Prefer composition over inheritance
5. Use slots for flexible content injection
