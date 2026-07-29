import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import SearchModal from './SearchModal.svelte';
import type { SearchHit, SearchResponse, SourceSummary } from '$lib/tauri';

// The one seam. Stubbing this module rather than a real backend is what makes
// the frontend testable at all — see `src/lib/tauri.ts` and
// docs/plans/08-testing-strategy.md § Tier A.
const search = vi.hoisted(() => vi.fn());
vi.mock('$lib/tauri', () => ({ search }));

const SOURCES: SourceSummary[] = [
  { id: 'rust-std', name: 'Rust std', category: 'Rust', page_count: 10, last_synced: null },
  { id: 'python', name: 'Python 3', category: 'Python', page_count: 20, last_synced: null },
];

function hit(overrides: Partial<SearchHit> = {}): SearchHit {
  return {
    source_id: 'rust-std',
    source_name: 'Rust std',
    path: 'std/vec/struct.Vec.html',
    title: 'Struct Vec',
    score: 1,
    symbol_kind: 'type',
    snippet: [
      { text: 'A contiguous growable ', matched: false },
      { text: 'array', matched: true },
      { text: ' type.', matched: false },
    ],
    ...overrides,
  };
}

function response(overrides: Partial<SearchResponse> = {}): SearchResponse {
  return {
    hits: [hit()],
    suggestions: [],
    elapsed_ms: 4,
    truncated: false,
    ...overrides,
  };
}

function setup(overrides: Record<string, unknown> = {}) {
  const onclose = vi.fn();
  const onselect = vi.fn();
  const onscope = vi.fn();
  render(SearchModal, {
    open: true,
    sources: SOURCES,
    scope: null,
    onscope,
    onclose,
    onselect,
    ...overrides,
  });
  return { onclose, onselect, onscope };
}

/** The debounce is 150 ms and the tests use real timers, so waits are real. */
async function results() {
  return waitFor(() => expect(screen.getAllByRole('option').length).toBeGreaterThan(0), {
    timeout: 2000,
  });
}

describe('SearchModal', () => {
  beforeEach(() => {
    localStorage.clear();
    search.mockReset();
    search.mockResolvedValue(response());
  });

  it('does not render anything while closed', () => {
    setup({ open: false });
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('shows results for a typed query', async () => {
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'vec');
    await results();
    expect(screen.getByText('Struct Vec')).toBeTruthy();
    // Scoped to the result: "Rust std" is also a scope button, and an
    // unscoped query would pass whichever of the two happened to exist.
    expect(screen.getAllByRole('option')[0]?.textContent).toContain('Rust std');
  });

  it('debounces rather than searching on every keystroke', async () => {
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'vector');
    await results();
    // Six characters, far fewer than six searches. The exact count depends on
    // typing speed, so this asserts the property rather than a number.
    expect(search.mock.calls.length).toBeLessThan(6);
  });

  it('renders a snippet as text, never as markup', async () => {
    // The rule this component exists to keep. A crawled page's snippet is
    // drawn in the app's own DOM, where the IPC bridge is reachable.
    search.mockResolvedValue(
      response({
        hits: [
          hit({
            snippet: [
              { text: '<img src=x onerror="alert(1)">', matched: false },
              { text: '</mark><script>bad()</script>', matched: true },
            ],
          }),
        ],
      }),
    );
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'vec');
    await results();

    const option = screen.getAllByRole('option')[0];
    expect(option?.querySelector('img')).toBeNull();
    expect(option?.querySelector('script')).toBeNull();
    // And the text is still shown, verbatim.
    expect(option?.textContent).toContain('<img src=x onerror="alert(1)">');
    expect(option?.textContent).toContain('<script>bad()</script>');
  });

  it('marks the matched runs of a snippet', async () => {
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'array');
    await results();
    const marks = screen.getAllByRole('option')[0]?.querySelectorAll('mark');
    expect(marks).toHaveLength(1);
    expect(marks?.[0]?.textContent).toBe('array');
  });

  it('moves the selection with the arrow keys and wraps at both ends', async () => {
    // Wrapping matters: without it, holding ↓ stops silently at the last
    // result and the user cannot tell whether the key stopped working.
    search.mockResolvedValue(
      response({
        hits: [hit({ path: 'a.html', title: 'A' }), hit({ path: 'b.html', title: 'B' })],
      }),
    );
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'vec');
    await results();

    const selected = () =>
      screen.getAllByRole('option').findIndex((o) => o.getAttribute('aria-selected') === 'true');

    expect(selected()).toBe(0);
    await userEvent.keyboard('{ArrowDown}');
    expect(selected()).toBe(1);
    await userEvent.keyboard('{ArrowDown}');
    expect(selected()).toBe(0);
    await userEvent.keyboard('{ArrowUp}');
    expect(selected()).toBe(1);
  });

  it('opens the selected result on Enter', async () => {
    const { onselect, onclose } = setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'vec');
    await results();
    await userEvent.keyboard('{Enter}');
    expect(onselect).toHaveBeenCalledWith(
      expect.objectContaining({ path: 'std/vec/struct.Vec.html' }),
    );
    expect(onclose).toHaveBeenCalled();
  });

  it('closes on Escape', async () => {
    const { onclose } = setup();
    await userEvent.keyboard('{Escape}');
    expect(onclose).toHaveBeenCalled();
  });

  it('closes when the backdrop is clicked', async () => {
    const { onclose } = setup();
    await userEvent.click(document.querySelector('.backdrop') as Element);
    expect(onclose).toHaveBeenCalled();
  });

  it('says so when there are no results, rather than showing nothing', async () => {
    search.mockResolvedValue(response({ hits: [] }));
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'nothingmatches');
    await waitFor(() => expect(screen.getByText(/No results/)).toBeTruthy(), { timeout: 2000 });
  });

  it('reports an error instead of an empty list', async () => {
    // An empty result list and a failed search look identical otherwise, and
    // the user would conclude their library does not contain the thing.
    search.mockRejectedValue(new Error('the index is unreadable'));
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'vec');
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('unreadable'), {
      timeout: 2000,
    });
  });

  it('offers a correction and re-searches when it is taken', async () => {
    search.mockResolvedValue(
      response({ hits: [], suggestions: [{ typed: 'enviroment', meant: 'environment' }] }),
    );
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'enviroment');
    await waitFor(() => expect(screen.getByText('environment')).toBeTruthy(), { timeout: 2000 });

    search.mockResolvedValue(response());
    await userEvent.click(screen.getByText('environment'));
    await waitFor(
      () => expect(search).toHaveBeenCalledWith('environment', null, expect.any(Number)),
      { timeout: 2000 },
    );
  });

  it('shows recent searches when the query is empty', async () => {
    localStorage.setItem('tome.search.history', JSON.stringify(['vec', 'hashmap']));
    setup();
    expect(screen.getByText('Recent searches')).toBeTruthy();
    expect(screen.getByText('vec')).toBeTruthy();
  });

  it('records a query only when a result is opened', async () => {
    // Typing is not searching. Recording every keystroke's prefix would fill
    // the history with `v`, `ve`, `vec`.
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'vec');
    await results();
    expect(localStorage.getItem('tome.search.history')).toBeNull();

    await userEvent.keyboard('{Enter}');
    expect(JSON.parse(localStorage.getItem('tome.search.history') ?? '[]')).toEqual(['vec']);
  });

  it('re-runs a recent search when it is clicked', async () => {
    localStorage.setItem('tome.search.history', JSON.stringify(['vec']));
    setup();
    await userEvent.click(screen.getByText('vec'));
    await waitFor(() => expect(search).toHaveBeenCalledWith('vec', null, expect.any(Number)), {
      timeout: 2000,
    });
  });

  it('forgets one recent search without touching the others', async () => {
    localStorage.setItem('tome.search.history', JSON.stringify(['vec', 'hashmap']));
    setup();
    await userEvent.click(screen.getByLabelText('Forget vec'));
    expect(JSON.parse(localStorage.getItem('tome.search.history') ?? '[]')).toEqual(['hashmap']);
  });

  it('clears the whole history', async () => {
    localStorage.setItem('tome.search.history', JSON.stringify(['vec', 'hashmap']));
    setup();
    await userEvent.click(screen.getByText('Clear all'));
    expect(localStorage.getItem('tome.search.history')).toBeNull();
  });

  it('shows the scope and passes it to the backend', async () => {
    setup({ scope: 'rust-std' });
    await userEvent.type(screen.getByLabelText('Search query'), 'vec');
    await waitFor(
      () => expect(search).toHaveBeenCalledWith('vec', 'rust-std', expect.any(Number)),
      {
        timeout: 2000,
      },
    );
    expect(screen.getByLabelText('Search all sources')).toBeTruthy();
  });

  it('clears the scope through its × button', async () => {
    const { onscope } = setup({ scope: 'rust-std' });
    await userEvent.click(screen.getByLabelText('Search all sources'));
    expect(onscope).toHaveBeenCalledWith(null);
  });

  it('shows the symbol kind on a reference page and nothing on prose', async () => {
    search.mockResolvedValue(
      response({
        hits: [
          hit({ path: 'a.html', title: 'Struct Vec', symbol_kind: 'type' }),
          hit({ path: 'b.html', title: 'Storing Lists', symbol_kind: null }),
        ],
      }),
    );
    setup();
    await userEvent.type(screen.getByLabelText('Search query'), 'vec');
    await results();
    const options = screen.getAllByRole('option');
    expect(options[0]?.textContent).toContain('type');
    expect(options[1]?.querySelector('.kind')).toBeNull();
  });

  it('ignores a slow response that a newer query has superseded', async () => {
    // A debounce does not serialise requests. Without a sequence check, a
    // slow early query resolving late overwrites the results of the query the
    // user has actually finished typing.
    let releaseSlow: (value: SearchResponse) => void = () => {};
    const slow = new Promise<SearchResponse>((resolve) => (releaseSlow = resolve));
    search.mockReturnValueOnce(slow);
    search.mockResolvedValue(response({ hits: [hit({ title: 'Newer result' })] }));

    setup();
    const input = screen.getByLabelText('Search query');
    await userEvent.type(input, 'v');
    await new Promise((r) => setTimeout(r, 200));
    await userEvent.type(input, 'ec');
    await waitFor(() => expect(screen.getByText('Newer result')).toBeTruthy(), { timeout: 2000 });

    releaseSlow(response({ hits: [hit({ title: 'Stale result' })] }));
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.queryByText('Stale result')).toBeNull();
    expect(screen.getByText('Newer result')).toBeTruthy();
  });

  it('does not search for an empty query', async () => {
    setup();
    const input = screen.getByLabelText('Search query');
    await userEvent.type(input, 'vec');
    await results();
    search.mockClear();
    await userEvent.clear(input);
    await new Promise((r) => setTimeout(r, 300));
    expect(search).not.toHaveBeenCalled();
  });
});
