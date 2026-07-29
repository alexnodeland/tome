import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MAX_ENTRIES, searchHistory } from './searchHistory';

const KEY = 'tome.search.history';

describe('searchHistory', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it('returns nothing before anything is searched', () => {
    expect(searchHistory.list()).toEqual([]);
  });

  it('puts the most recent search first', () => {
    searchHistory.add('vec');
    searchHistory.add('hashmap');
    expect(searchHistory.list()).toEqual(['hashmap', 'vec']);
  });

  it('deduplicates against the whole list, not just the previous entry', () => {
    // P2-016 asks for consecutive dedup, which is not enough on its own: a
    // search re-run from the recents list has whatever was searched in
    // between sitting between the two, so a consecutive-only check would
    // record it twice and the list would fill with one query.
    searchHistory.add('vec');
    searchHistory.add('hashmap');
    searchHistory.add('vec');
    expect(searchHistory.list()).toEqual(['vec', 'hashmap']);
  });

  it('keeps at most the last 50', () => {
    for (let i = 0; i < MAX_ENTRIES + 10; i += 1) searchHistory.add(`query ${i}`);
    const list = searchHistory.list();
    expect(list).toHaveLength(MAX_ENTRIES);
    expect(list[0]).toBe(`query ${MAX_ENTRIES + 9}`);
  });

  it('does not record an empty or whitespace-only query', () => {
    searchHistory.add('');
    searchHistory.add('   ');
    searchHistory.add('\n\t');
    expect(searchHistory.list()).toEqual([]);
  });

  it('stores queries trimmed, so leading space does not make a new entry', () => {
    searchHistory.add('  vec  ');
    searchHistory.add('vec');
    expect(searchHistory.list()).toEqual(['vec']);
  });

  it('forgets one entry and leaves the rest', () => {
    searchHistory.add('vec');
    searchHistory.add('hashmap');
    expect(searchHistory.remove('vec')).toEqual(['hashmap']);
    expect(searchHistory.list()).toEqual(['hashmap']);
  });

  it('clearing really removes it rather than hiding it', () => {
    // This is reading history. "Clear" has to mean the bytes are gone, not
    // that a flag was set.
    searchHistory.add('something private');
    searchHistory.clear();
    expect(searchHistory.list()).toEqual([]);
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it('survives a store holding something that is not a history', () => {
    // localStorage can hold what another version wrote, or what somebody
    // typed into the inspector. A history that throws on load would stop the
    // app from opening, which no convenience is worth.
    for (const junk of ['not json', '{"not":"an array"}', '42', 'null', '[]']) {
      localStorage.setItem(KEY, junk);
      expect(searchHistory.list()).toEqual([]);
    }
  });

  it('drops non-string entries rather than rendering them', () => {
    localStorage.setItem(KEY, JSON.stringify(['vec', 42, null, { a: 1 }, 'hashmap']));
    expect(searchHistory.list()).toEqual(['vec', 'hashmap']);
  });

  it('survives localStorage being unavailable entirely', () => {
    // A webview with storage disabled, or a test environment. Every path has
    // to degrade to "not remembered" rather than throw.
    const boom = () => {
      throw new Error('storage disabled');
    };
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(boom);
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(boom);
    vi.spyOn(Storage.prototype, 'removeItem').mockImplementation(boom);

    expect(() => searchHistory.list()).not.toThrow();
    expect(() => searchHistory.add('vec')).not.toThrow();
    expect(() => searchHistory.remove('vec')).not.toThrow();
    expect(() => searchHistory.clear()).not.toThrow();
    expect(searchHistory.list()).toEqual([]);
  });

  it('caps a pasted query rather than storing a document', () => {
    searchHistory.add('x'.repeat(5000));
    const [entry] = searchHistory.list();
    expect(entry?.length).toBeLessThanOrEqual(200);
  });
});
