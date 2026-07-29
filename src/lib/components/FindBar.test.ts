import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import FindBar from './FindBar.svelte';

function setup(overrides: Record<string, unknown> = {}) {
  const onfind = vi.fn();
  const onstep = vi.fn();
  const onclose = vi.fn();
  render(FindBar, {
    open: true,
    total: 0,
    index: 0,
    supported: true,
    onfind,
    onstep,
    onclose,
    ...overrides,
  });
  return { onfind, onstep, onclose };
}

describe('FindBar', () => {
  it('renders nothing while closed', () => {
    setup({ open: false });
    expect(screen.queryByLabelText('Find in page')).toBeNull();
  });

  it('asks the frame to search, debounced', async () => {
    const { onfind } = setup();
    await userEvent.type(screen.getByLabelText('Find in page'), 'vec');
    await waitFor(() => expect(onfind).toHaveBeenCalledWith('vec'), { timeout: 2000 });
    // Three characters, fewer than three searches.
    expect(onfind.mock.calls.length).toBeLessThan(3);
  });

  it('shows which match of how many', () => {
    setup({ total: 7, index: 3 });
    expect(screen.getByText(/3 of 7/)).toBeTruthy();
  });

  it('says there are no matches rather than showing 0 of 0', async () => {
    const { onfind } = setup();
    await userEvent.type(screen.getByLabelText('Find in page'), 'zzz');
    await waitFor(() => expect(onfind).toHaveBeenCalled(), { timeout: 2000 });
    expect(screen.getByText(/no matches/)).toBeTruthy();
  });

  it('says find is unavailable rather than silently reporting nothing', () => {
    // The engine has no CSS Custom Highlight API. Reporting "no matches" here
    // would be a lie: the search never ran.
    setup({ supported: false });
    expect(screen.getByText(/unavailable/)).toBeTruthy();
  });

  it('steps forward on Enter and back on Shift+Enter', async () => {
    const { onstep } = setup({ total: 3, index: 1 });
    const input = screen.getByLabelText('Find in page');
    await userEvent.click(input);
    await userEvent.keyboard('{Enter}');
    expect(onstep).toHaveBeenLastCalledWith(1);
    await userEvent.keyboard('{Shift>}{Enter}{/Shift}');
    expect(onstep).toHaveBeenLastCalledWith(-1);
  });

  it('steps with the arrow buttons', async () => {
    const { onstep } = setup({ total: 3, index: 1 });
    await userEvent.click(screen.getByLabelText('Next match'));
    expect(onstep).toHaveBeenLastCalledWith(1);
    await userEvent.click(screen.getByLabelText('Previous match'));
    expect(onstep).toHaveBeenLastCalledWith(-1);
  });

  it('disables the step buttons when there is nothing to step through', () => {
    setup({ total: 0 });
    expect(screen.getByLabelText('Next match')).toBeDisabled();
    expect(screen.getByLabelText('Previous match')).toBeDisabled();
  });

  it('closes on Escape', async () => {
    const { onclose } = setup();
    await userEvent.click(screen.getByLabelText('Find in page'));
    await userEvent.keyboard('{Escape}');
    expect(onclose).toHaveBeenCalled();
  });

  it('closes with the × button', async () => {
    const { onclose } = setup();
    await userEvent.click(screen.getByLabelText('Close find'));
    expect(onclose).toHaveBeenCalled();
  });

  it('announces the count to assistive technology as it changes', () => {
    // The count is the only feedback that the search did anything, and it is
    // otherwise invisible to a screen reader — the highlighting is painted by
    // CSS in a document the reader cannot see.
    setup({ total: 2, index: 1 });
    const count = screen.getByText(/1 of 2/);
    expect(count.getAttribute('aria-live')).toBe('polite');
  });
});
