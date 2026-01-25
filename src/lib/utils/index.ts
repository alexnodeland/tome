// Utility functions
// See README.md in this directory for patterns

/**
 * Debounce a function call
 *
 * @param fn - Function to debounce
 * @param delay - Delay in milliseconds
 * @returns Debounced function
 */
export function debounce<T extends (...args: Parameters<T>) => ReturnType<T>>(
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
 * @param suffix - Suffix to append when truncated
 * @returns Truncated text
 */
export function truncate(text: string, maxLength: number, suffix = '...'): string {
  if (text.length <= maxLength) {
    return text;
  }
  return text.slice(0, maxLength - suffix.length) + suffix;
}

/**
 * Format a date for display
 *
 * @param date - Date to format
 * @param options - Formatting options
 * @returns Formatted date string
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
      const existing = acc[key];
      acc[key] = existing ? [...existing, item] : [item];
      return acc;
    },
    {} as Record<K, T[]>
  );
}

/**
 * Generate a unique ID
 *
 * @returns A unique ID string
 */
export function generateId(): string {
  return crypto.randomUUID();
}
