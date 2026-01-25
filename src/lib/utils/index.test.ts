// Unit tests for utility functions
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

import { debounce, truncate, formatDate, groupBy, generateId } from './index';

describe('debounce', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should delay function execution', () => {
    const fn = vi.fn();
    const debouncedFn = debounce(fn, 100);

    debouncedFn();
    expect(fn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('should reset timer on subsequent calls', () => {
    const fn = vi.fn();
    const debouncedFn = debounce(fn, 100);

    debouncedFn();
    vi.advanceTimersByTime(50);

    debouncedFn();
    vi.advanceTimersByTime(50);

    expect(fn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(50);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('should pass arguments to debounced function', () => {
    const fn = vi.fn();
    const debouncedFn = debounce(fn, 100);

    debouncedFn('arg1', 'arg2');
    vi.advanceTimersByTime(100);

    expect(fn).toHaveBeenCalledWith('arg1', 'arg2');
  });
});

describe('truncate', () => {
  it('should return text unchanged if under limit', () => {
    expect(truncate('hello', 10)).toBe('hello');
  });

  it('should truncate with default suffix', () => {
    expect(truncate('hello world', 8)).toBe('hello...');
  });

  it('should truncate with custom suffix', () => {
    expect(truncate('hello world', 8, '…')).toBe('hello w…');
  });

  it('should handle exact length', () => {
    expect(truncate('hello', 5)).toBe('hello');
  });

  it('should handle empty string', () => {
    expect(truncate('', 5)).toBe('');
  });
});

describe('formatDate', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-01-25T12:00:00Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('should format today as relative', () => {
    const today = new Date('2026-01-25T10:30:00Z');
    const result = formatDate(today);
    expect(result).toMatch(/^Today at/);
  });

  it('should format yesterday as relative', () => {
    const yesterday = new Date('2026-01-24T10:30:00Z');
    const result = formatDate(yesterday);
    expect(result).toMatch(/^Yesterday at/);
  });

  it('should format old dates as days ago', () => {
    const threeDaysAgo = new Date('2026-01-22T10:30:00Z');
    const result = formatDate(threeDaysAgo);
    expect(result).toBe('3 days ago');
  });

  it('should format very old dates as absolute', () => {
    const oldDate = new Date('2025-01-15T10:30:00Z');
    const result = formatDate(oldDate);
    expect(result).toBe('Jan 15, 2025');
  });

  it('should format as absolute when relative is false', () => {
    const today = new Date('2026-01-25T10:30:00Z');
    const result = formatDate(today, { relative: false });
    expect(result).toBe('Jan 25, 2026');
  });

  it('should handle string dates', () => {
    const result = formatDate('2025-01-15', { relative: false });
    expect(result).toBe('Jan 15, 2025');
  });
});

describe('groupBy', () => {
  it('should group items by key', () => {
    const items = [
      { name: 'a', category: 'x' },
      { name: 'b', category: 'y' },
      { name: 'c', category: 'x' },
    ];

    const result = groupBy(items, (i) => i.category);

    expect(result['x']).toHaveLength(2);
    expect(result['y']).toHaveLength(1);
  });

  it('should handle empty array', () => {
    const result = groupBy([], () => 'key');
    expect(result).toEqual({});
  });

  it('should handle numeric keys', () => {
    const items = [
      { value: 1, group: 1 },
      { value: 2, group: 2 },
      { value: 3, group: 1 },
    ];

    const result = groupBy(items, (i) => i.group);

    expect(result[1]).toHaveLength(2);
    expect(result[2]).toHaveLength(1);
  });
});

describe('generateId', () => {
  it('should generate a unique ID', () => {
    const id1 = generateId();
    const id2 = generateId();

    expect(id1).not.toBe(id2);
  });

  it('should generate valid UUID format', () => {
    const id = generateId();
    const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

    expect(id).toMatch(uuidRegex);
  });
});
