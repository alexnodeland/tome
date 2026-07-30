import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Library from './Library.svelte';
import type { PageSummary, SourceSummary } from '$lib/tauri';

const SOURCES: SourceSummary[] = [
  {
    id: 'rust-std',
    name: 'Rust standard library',
    category: 'Rust',
    page_count: 12,
    last_synced: null,
  },
  { id: 'python', name: 'Python 3', category: 'Python', page_count: 340, last_synced: null },
  { id: 'cargo', name: 'The Cargo Book', category: 'Rust', page_count: 40, last_synced: null },
];

const PAGES: PageSummary[] = [
  { path: 'index.html', title: 'Introduction' },
  { path: 'api/reference.html', title: 'API reference' },
];

function setup(overrides: Partial<Parameters<typeof Library>[1]> = {}) {
  const onselectsource = vi.fn();
  const onselectpage = vi.fn();
  render(Library, {
    sources: SOURCES,
    pages: PAGES,
    selectedSource: 'rust-std',
    selectedPage: 'index.html',
    onselectsource,
    onselectpage,
    ...overrides,
  });
  return { onselectsource, onselectpage };
}

describe('Library', () => {
  it('groups sources by category', () => {
    setup();
    // Exact names: `/Rust/` also matches "Rust standard library", and a
    // query that matches two things is a query that asserts nothing.
    expect(screen.getByRole('button', { name: 'Rust' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Python' })).toBeInTheDocument();
    // Both Rust sources under the one heading.
    expect(screen.getByRole('button', { name: /Rust standard library/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /The Cargo Book/ })).toBeInTheDocument();
  });

  it('shows the pages of the selected source and nothing else', () => {
    setup();
    expect(screen.getByRole('button', { name: 'Introduction' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'API reference' })).toBeInTheDocument();
  });

  it('marks the current page for a screen reader, not only by colour', () => {
    // The design system's rule: colour is never the only carrier of meaning.
    setup();
    expect(screen.getByRole('button', { name: 'Introduction' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(screen.getByRole('button', { name: 'API reference' })).not.toHaveAttribute(
      'aria-current',
    );
  });

  it('filters sources by name, id, and category', async () => {
    const user = userEvent.setup();
    setup();
    const filter = screen.getByRole('searchbox', { name: /filter/i });

    await user.type(filter, 'cargo');
    expect(screen.queryByRole('button', { name: /Python 3/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /The Cargo Book/ })).toBeInTheDocument();

    await user.clear(filter);
    await user.type(filter, 'python');
    expect(screen.getByRole('button', { name: /Python 3/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Cargo/ })).not.toBeInTheDocument();
  });

  it('says so when the filter matches nothing', async () => {
    const user = userEvent.setup();
    setup();
    await user.type(screen.getByRole('searchbox', { name: /filter/i }), 'zzzz');
    expect(screen.getByText(/Nothing matches/)).toBeInTheDocument();
  });

  it('collapses a category without losing the sources in it', async () => {
    const user = userEvent.setup();
    setup();
    const category = screen.getByRole('button', { name: 'Rust' });

    expect(category).toHaveAttribute('aria-expanded', 'true');
    await user.click(category);
    expect(category).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('button', { name: /Rust standard library/ })).not.toBeInTheDocument();

    await user.click(category);
    expect(screen.getByRole('button', { name: /Rust standard library/ })).toBeInTheDocument();
  });

  it('reports a selection to the shell rather than navigating itself', async () => {
    const user = userEvent.setup();
    const { onselectsource, onselectpage } = setup();

    await user.click(screen.getByRole('button', { name: /Python 3/ }));
    expect(onselectsource).toHaveBeenCalledWith('python');

    await user.click(screen.getByRole('button', { name: 'API reference' }));
    expect(onselectpage).toHaveBeenCalledWith('api/reference.html');
  });

  it('moves focus with the arrow keys', async () => {
    const user = userEvent.setup();
    setup();
    const first = screen.getByRole('button', { name: 'Python' });
    first.focus();

    await user.keyboard('{ArrowDown}');
    expect(document.activeElement).not.toBe(first);
    await user.keyboard('{ArrowUp}');
    expect(document.activeElement).toBe(first);
  });

  it('is reachable as a landmark', () => {
    setup();
    expect(screen.getByRole('navigation', { name: 'Documentation sources' })).toBeInTheDocument();
  });

  it('renders a window of a large page list rather than all of it', async () => {
    // Measured at S4-2: a 20 000-page source is 19 ms out of SQLite and 1 ms
    // to serialise — the backend is fine — but it is 20 000 DOM nodes, and
    // that is not. The window is well past what any screen shows.
    const many: PageSummary[] = Array.from({ length: 1000 }, (_, i) => ({
      path: `std/struct.Thing${i}.html`,
      title: `Struct Thing${i}`,
    }));
    setup({ pages: many });

    expect(screen.getAllByRole('button', { name: /^Struct Thing/ })).toHaveLength(200);
    // And it says how many are left rather than pretending this is all of them.
    const more = screen.getByRole('button', { name: /800 more/ });
    await userEvent.setup().click(more);
    expect(screen.getAllByRole('button', { name: /^Struct Thing/ })).toHaveLength(400);
  });

  it('does not offer to show more when everything is already shown', () => {
    setup();
    expect(screen.queryByRole('button', { name: /more/ })).not.toBeInTheDocument();
  });
});
