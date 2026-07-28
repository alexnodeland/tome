import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Layout from './Layout.test.svelte';

/**
 * `Layout` takes three snippets, which cannot be passed from a `.ts` test —
 * snippets are compile-time constructs. `Layout.test.svelte` is a thin
 * harness that supplies them.
 */
describe('Layout', () => {
  beforeEach(() => {
    localStorage.clear();
    // jsdom reports innerWidth 1024 by default, which is exactly the
    // right-sidebar breakpoint. Widen it so the default state is both open.
    window.innerWidth = 1400;
  });

  it('shows all three panels', () => {
    render(Layout);
    expect(screen.getByRole('complementary', { name: 'Library' })).toBeInTheDocument();
    expect(screen.getByRole('main')).toBeInTheDocument();
    expect(screen.getByRole('complementary', { name: 'On this page' })).toBeInTheDocument();
  });

  it('collapses a sidebar to zero width without unmounting it', async () => {
    // Unmounting would throw away scroll position and focus, and there is
    // nothing to animate. Width is the animated property.
    const user = userEvent.setup();
    render(Layout);
    const library = screen.getByRole('complementary', { name: 'Library' });

    expect(library.style.width).not.toBe('0px');
    await user.click(screen.getByRole('button', { name: 'toggle left' }));
    expect(library.style.width).toBe('0px');
    expect(library).toBeInTheDocument();
  });

  it('hides a collapsed panel from the tab order and the accessibility tree', async () => {
    // A zero-width panel whose buttons are still focusable is a keyboard trap
    // that nothing on screen explains.
    const user = userEvent.setup();
    render(Layout);
    await user.click(screen.getByRole('button', { name: 'toggle left' }));

    expect(screen.queryByRole('complementary', { name: 'Library' })).not.toBeInTheDocument();
  });

  it('remembers what was collapsed', async () => {
    const user = userEvent.setup();
    const first = render(Layout);
    await user.click(screen.getByRole('button', { name: 'toggle right' }));
    first.unmount();

    render(Layout);
    expect(screen.queryByRole('complementary', { name: 'On this page' })).not.toBeInTheDocument();
  });

  it('resizes with the keyboard, so the panels work with no pointer', async () => {
    const user = userEvent.setup();
    render(Layout);
    const handle = screen.getByRole('separator', { name: 'Resize library' });
    const before = Number(handle.getAttribute('aria-valuenow'));

    handle.focus();
    await user.keyboard('{ArrowRight}');
    expect(Number(handle.getAttribute('aria-valuenow'))).toBeGreaterThan(before);

    await user.keyboard('{ArrowLeft}');
    expect(Number(handle.getAttribute('aria-valuenow'))).toBe(before);
  });

  it('clamps the width to something the panel can actually render at', async () => {
    const user = userEvent.setup();
    render(Layout);
    const handle = screen.getByRole('separator', { name: 'Resize library' });
    const min = Number(handle.getAttribute('aria-valuemin'));

    handle.focus();
    for (let i = 0; i < 40; i++) await user.keyboard('{ArrowLeft}');
    expect(Number(handle.getAttribute('aria-valuenow'))).toBe(min);
  });

  it('auto-collapses the outline on a narrow window without forgetting the preference', async () => {
    render(Layout);
    expect(screen.getByRole('complementary', { name: 'On this page' })).toBeInTheDocument();

    window.innerWidth = 900;
    window.dispatchEvent(new Event('resize'));
    await Promise.resolve();
    expect(screen.queryByRole('complementary', { name: 'On this page' })).not.toBeInTheDocument();

    // Widening restores what the user chose, not what the narrow window
    // forced on them.
    window.innerWidth = 1400;
    window.dispatchEvent(new Event('resize'));
    await Promise.resolve();
    expect(screen.getByRole('complementary', { name: 'On this page' })).toBeInTheDocument();
  });
});
