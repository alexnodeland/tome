import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Outline from './Outline.svelte';
import type { OutlineEntry } from '$lib/tauri';

const OUTLINE: OutlineEntry[] = [
  {
    id: 'widget',
    title: 'Widget',
    level: 1,
    children: [
      { id: 'install', title: 'Installation', level: 2, children: [] },
      {
        id: 'api',
        title: 'API',
        level: 2,
        children: [
          {
            id: 'api-resize',
            title: 'resize()',
            level: 3,
            children: [
              {
                id: 'api-resize-args',
                title: 'Arguments',
                level: 4,
                children: [{ id: 'api-resize-args-w', title: 'width', level: 4, children: [] }],
              },
            ],
          },
        ],
      },
    ],
  },
];

describe('Outline', () => {
  it('renders the whole tree, however deep', () => {
    render(Outline, { outline: OUTLINE, activeId: null, onselect: vi.fn() });
    for (const title of ['Widget', 'Installation', 'API', 'resize()', 'Arguments', 'width']) {
      expect(screen.getByRole('button', { name: title })).toBeInTheDocument();
    }
  });

  it('marks the current heading for a screen reader, not only by colour', () => {
    render(Outline, { outline: OUTLINE, activeId: 'api', onselect: vi.fn() });
    expect(screen.getByRole('button', { name: 'API' })).toHaveAttribute('aria-current', 'location');
    expect(screen.getByRole('button', { name: 'Widget' })).not.toHaveAttribute('aria-current');
  });

  it('reports the heading rather than scrolling anything itself', async () => {
    // It cannot scroll the reader: that document is in a sandboxed frame the
    // app webview cannot reach. The shell forwards this over the bridge.
    const user = userEvent.setup();
    const onselect = vi.fn();
    render(Outline, { outline: OUTLINE, activeId: null, onselect });

    await user.click(screen.getByRole('button', { name: 'Installation' }));
    expect(onselect).toHaveBeenCalledWith('install');
  });

  it('indents by depth but stops before running off the sidebar', () => {
    render(Outline, { outline: OUTLINE, activeId: null, onselect: vi.fn() });
    const depth3 = screen.getByRole('button', { name: 'Arguments' });
    const depth4 = screen.getByRole('button', { name: 'width' });
    // P1-019's "collapse deeply nested items": past the cap, entries stay
    // listed and clickable but stop marching rightwards.
    expect(depth3.style.paddingLeft).toBe(depth4.style.paddingLeft);
  });

  it('says so when a page has no headings', () => {
    render(Outline, { outline: [], activeId: null, onselect: vi.fn() });
    expect(screen.getByText('No headings.')).toBeInTheDocument();
  });

  it('is reachable as a landmark', () => {
    render(Outline, { outline: OUTLINE, activeId: null, onselect: vi.fn() });
    expect(screen.getByRole('navigation', { name: 'On this page' })).toBeInTheDocument();
  });
});
