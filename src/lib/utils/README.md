# Utils Directory

Pure utility functions with no side effects.

## What Belongs Here

- **Pure functions** that transform data
- **Helper functions** used across the application
- **Constants** that don't change at runtime
- **Utility tests** (`.test.ts` files) adjacent to their module

## What Does NOT Belong Here

- Functions with side effects (API calls, DOM manipulation)
- State management (use `stores/`)
- Tauri integrations (use `services/`)
- UI components (use `components/`)

## Naming Conventions

- Utility files: `kebab-case.ts` (e.g., `format-date.ts`)
- Test files: `util-name.test.ts` (e.g., `format-date.test.ts`)
- Export all utilities from `index.ts`

## Utility Pattern

```typescript
// format-date.ts

/**
 * Format a date for display in the UI
 *
 * @param date - Date to format
 * @param options - Formatting options
 * @returns Formatted date string
 *
 * @example
 * formatDate(new Date()) // "Today at 3:45 PM"
 * formatDate(new Date(), { relative: false }) // "Jan 25, 2026"
 */
export function formatDate(
  date: Date | string,
  options: { relative?: boolean } = { relative: true }
): string {
  const d = typeof date === 'string' ? new Date(date) : date;

  if (options.relative) {
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffDays === 0) {
      return `Today at ${formatTime(d)}`;
    }
    if (diffDays === 1) {
      return `Yesterday at ${formatTime(d)}`;
    }
    if (diffDays < 7) {
      return `${diffDays} days ago`;
    }
  }

  return d.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

function formatTime(date: Date): string {
  return date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  });
}

/**
 * Debounce a function call
 *
 * @param fn - Function to debounce
 * @param delay - Delay in milliseconds
 * @returns Debounced function
 */
export function debounce<T extends (...args: unknown[]) => unknown>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout>;

  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn(...args), delay);
  };
}

/**
 * Truncate text to a maximum length
 *
 * @param text - Text to truncate
 * @param maxLength - Maximum length
 * @param suffix - Suffix to append when truncated (default: "...")
 * @returns Truncated text
 */
export function truncate(text: string, maxLength: number, suffix = '...'): string {
  if (text.length <= maxLength) {
    return text;
  }
  return text.slice(0, maxLength - suffix.length) + suffix;
}

/**
 * Group an array by a key function
 *
 * @param items - Array to group
 * @param keyFn - Function to extract the key
 * @returns Object with keys and grouped items
 */
export function groupBy<T, K extends string | number>(
  items: T[],
  keyFn: (item: T) => K
): Record<K, T[]> {
  return items.reduce(
    (acc, item) => {
      const key = keyFn(item);
      acc[key] = [...(acc[key] ?? []), item];
      return acc;
    },
    {} as Record<K, T[]>
  );
}
```

## Testing Pattern

```typescript
// format-date.test.ts
import { formatDate, truncate, groupBy } from './format-date';

describe('formatDate', () => {
  it('formats today as relative', () => {
    const now = new Date();
    const result = formatDate(now);
    expect(result).toMatch(/^Today at/);
  });

  it('formats old dates as absolute', () => {
    const oldDate = new Date('2020-01-15');
    const result = formatDate(oldDate, { relative: false });
    expect(result).toBe('Jan 15, 2020');
  });

  it('handles string dates', () => {
    const result = formatDate('2020-01-15', { relative: false });
    expect(result).toBe('Jan 15, 2020');
  });
});

describe('truncate', () => {
  it('returns text unchanged if under limit', () => {
    expect(truncate('hello', 10)).toBe('hello');
  });

  it('truncates with default suffix', () => {
    expect(truncate('hello world', 8)).toBe('hello...');
  });

  it('truncates with custom suffix', () => {
    expect(truncate('hello world', 8, '…')).toBe('hello w…');
  });
});

describe('groupBy', () => {
  it('groups items by key', () => {
    const items = [
      { name: 'a', category: 'x' },
      { name: 'b', category: 'y' },
      { name: 'c', category: 'x' },
    ];

    const result = groupBy(items, (i) => i.category);

    expect(result['x']).toHaveLength(2);
    expect(result['y']).toHaveLength(1);
  });

  it('handles empty array', () => {
    const result = groupBy([], () => 'key');
    expect(result).toEqual({});
  });
});
```

## Architectural Rules

1. Utils **cannot import from any sibling directory** (`components/`, `stores/`, `services/`, `routes/`)
2. Utils **can only import from** `types/` or external libraries
3. All functions must be **pure** (no side effects)
4. All functions must be **well-documented** with JSDoc
5. All functions must have **comprehensive tests**
6. Prefer small, focused functions over large multi-purpose ones
