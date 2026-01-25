# Stores Directory

Svelte stores for application state management.

## What Belongs Here

- **Svelte stores** (`.ts` files) for shared application state
- **Store tests** (`.test.ts` files) adjacent to their store
- **Derived stores** that compute values from other stores

## What Does NOT Belong Here

- UI components (use `components/`)
- API calls (use `services/`)
- Utility functions (use `utils/`)
- Type definitions (use `types/`)

## Naming Conventions

- Store files: `kebab-case.ts` (e.g., `navigation-history.ts`)
- Test files: `store-name.test.ts` (e.g., `navigation-history.test.ts`)
- Export all stores from `index.ts`

## Store Pattern

```typescript
// sources.ts
import { writable, derived, type Readable } from 'svelte/store';
import type { Source } from '$lib/types';

// --- Private State ---
// Use underscore prefix for internal writable stores
const _sources = writable<Source[]>([]);
const _loading = writable(false);
const _error = writable<string | null>(null);

// --- Public Readable State ---
// Expose read-only subscriptions
export const sources: Readable<Source[]> = { subscribe: _sources.subscribe };
export const sourcesLoading: Readable<boolean> = { subscribe: _loading.subscribe };
export const sourcesError: Readable<string | null> = { subscribe: _error.subscribe };

// --- Derived State ---
// Computed values that update automatically
export const sourceCount = derived(_sources, ($sources) => $sources.length);

export const sourcesByCategory = derived(_sources, ($sources) =>
  $sources.reduce(
    (acc, source) => {
      const category = source.category ?? 'Uncategorized';
      acc[category] = [...(acc[category] ?? []), source];
      return acc;
    },
    {} as Record<string, Source[]>
  )
);

// --- Actions ---
// Named functions that modify state
export const sourcesStore = {
  // Initialize/refresh from backend
  async load(): Promise<void> {
    _loading.set(true);
    _error.set(null);
    try {
      const result = await listSources();
      if (result.ok) {
        _sources.set(result.value);
      } else {
        _error.set(result.error.message);
      }
    } finally {
      _loading.set(false);
    }
  },

  // Add a source
  add(source: Source): void {
    _sources.update((current) => [...current, source]);
  },

  // Remove a source
  remove(id: string): void {
    _sources.update((current) => current.filter((s) => s.id !== id));
  },

  // Update a source
  update(id: string, updates: Partial<Source>): void {
    _sources.update((current) =>
      current.map((s) => (s.id === id ? { ...s, ...updates } : s))
    );
  },

  // Clear all sources (for testing)
  clear(): void {
    _sources.set([]);
    _error.set(null);
  },
};
```

## Testing Pattern

```typescript
// sources.test.ts
import { get } from 'svelte/store';
import { sources, sourceCount, sourcesStore } from './sources';

describe('Sources Store', () => {
  beforeEach(() => {
    sourcesStore.clear();
  });

  it('starts with empty sources', () => {
    expect(get(sources)).toEqual([]);
    expect(get(sourceCount)).toBe(0);
  });

  it('adds a source', () => {
    const source = { id: '1', name: 'Test', category: 'Rust' };
    sourcesStore.add(source);

    expect(get(sources)).toContainEqual(source);
    expect(get(sourceCount)).toBe(1);
  });

  it('removes a source', () => {
    const source = { id: '1', name: 'Test', category: 'Rust' };
    sourcesStore.add(source);
    sourcesStore.remove('1');

    expect(get(sources)).toEqual([]);
  });

  it('computes sources by category', () => {
    sourcesStore.add({ id: '1', name: 'A', category: 'Rust' });
    sourcesStore.add({ id: '2', name: 'B', category: 'Python' });
    sourcesStore.add({ id: '3', name: 'C', category: 'Rust' });

    const byCategory = get(sourcesByCategory);
    expect(byCategory['Rust']).toHaveLength(2);
    expect(byCategory['Python']).toHaveLength(1);
  });
});
```

## Architectural Rules

1. Stores **cannot import from** `components/` or `routes/`
2. Stores **can import from** `services/`, `utils/`, `types/`
3. Keep stores focused on a single domain concept
4. Prefer derived stores over manual computation in components
5. Always provide a `clear()` method for testing
6. Never expose writable stores directly - wrap with actions
