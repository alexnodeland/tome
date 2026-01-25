# Services Directory

Tauri IPC wrappers and external API integrations.

## What Belongs Here

- **Service modules** (`.ts` files) that wrap Tauri `invoke` calls
- **Service tests** (`.test.ts` files) adjacent to their service
- **API response types** specific to service layer

## What Does NOT Belong Here

- UI components (use `components/`)
- State management (use `stores/`)
- Pure utility functions (use `utils/`)
- Shared type definitions (use `types/`)

## Naming Conventions

- Service files: `kebab-case.ts` (e.g., `source-sync.ts`)
- Test files: `service-name.test.ts` (e.g., `source-sync.test.ts`)
- Export all services from `index.ts`

## Service Pattern

```typescript
// sources.ts
import { invoke } from '@tauri-apps/api/tauri';
import type { Source, SourceConfig, TomeResult, TomeError } from '$lib/types';

/**
 * List all documentation sources
 */
export async function listSources(): Promise<TomeResult<Source[]>> {
  try {
    const sources = await invoke<Source[]>('list_sources');
    return { ok: true, value: sources };
  } catch (error) {
    return {
      ok: false,
      error: parseError(error),
    };
  }
}

/**
 * Get a single source by ID
 */
export async function getSource(id: string): Promise<TomeResult<Source>> {
  try {
    const source = await invoke<Source>('get_source', { id });
    return { ok: true, value: source };
  } catch (error) {
    return {
      ok: false,
      error: parseError(error),
    };
  }
}

/**
 * Add a new documentation source
 */
export async function addSource(config: SourceConfig): Promise<TomeResult<Source>> {
  try {
    const source = await invoke<Source>('add_source', { config });
    return { ok: true, value: source };
  } catch (error) {
    return {
      ok: false,
      error: parseError(error),
    };
  }
}

/**
 * Remove a documentation source
 */
export async function removeSource(id: string): Promise<TomeResult<void>> {
  try {
    await invoke<void>('remove_source', { id });
    return { ok: true, value: undefined };
  } catch (error) {
    return {
      ok: false,
      error: parseError(error),
    };
  }
}

/**
 * Trigger sync for a source
 */
export async function syncSource(
  id: string,
  options?: { force?: boolean }
): Promise<TomeResult<void>> {
  try {
    await invoke<void>('sync_source', { id, ...options });
    return { ok: true, value: undefined };
  } catch (error) {
    return {
      ok: false,
      error: parseError(error),
    };
  }
}

// --- Internal Helpers ---

function parseError(error: unknown): TomeError {
  if (typeof error === 'string') {
    // Tauri returns error strings from Rust
    if (error.includes('not found')) {
      return { code: 'NOT_FOUND', message: error };
    }
    if (error.includes('network') || error.includes('fetch')) {
      return { code: 'NETWORK', message: error };
    }
    return { code: 'UNKNOWN', message: error };
  }

  if (error instanceof Error) {
    return { code: 'UNKNOWN', message: error.message };
  }

  return { code: 'UNKNOWN', message: 'An unknown error occurred' };
}
```

## Testing Pattern

```typescript
// sources.test.ts
import { vi } from 'vitest';
import { listSources, addSource, removeSource } from './sources';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/tauri';

describe('Sources Service', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listSources', () => {
    it('returns sources on success', async () => {
      const mockSources = [{ id: '1', name: 'Test' }];
      vi.mocked(invoke).mockResolvedValue(mockSources);

      const result = await listSources();

      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.value).toEqual(mockSources);
      }
      expect(invoke).toHaveBeenCalledWith('list_sources');
    });

    it('returns error on failure', async () => {
      vi.mocked(invoke).mockRejectedValue('Database error');

      const result = await listSources();

      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.error.code).toBe('UNKNOWN');
        expect(result.error.message).toContain('Database');
      }
    });
  });

  describe('addSource', () => {
    it('adds source and returns it', async () => {
      const config = { name: 'New Source', url: 'https://example.com' };
      const mockSource = { id: '1', ...config };
      vi.mocked(invoke).mockResolvedValue(mockSource);

      const result = await addSource(config);

      expect(result.ok).toBe(true);
      expect(invoke).toHaveBeenCalledWith('add_source', { config });
    });
  });
});
```

## Architectural Rules

1. Services **cannot import from** `components/`, `stores/`, or `routes/`
2. Services **can import from** `utils/`, `types/`
3. All Tauri `invoke` calls must go through services (never directly in components)
4. Always return `TomeResult<T>` - never throw errors
5. Parse and normalize errors at the service layer
6. Keep services focused - one domain per file
