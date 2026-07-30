import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Preferences from './Preferences.svelte';
import { loadAppearance } from '$lib/appearance';
import { preferences } from '$lib/stores/preferences';
import { invoked } from '../../test/setup';

const LOCATION = {
  bundle_id: 'com.alexnodeland.tome',
  version: '0.1.0',
  state_root: '/Users/test/Library/Application Support/Tome',
  cache_root: '/Users/test/Library/Caches/Tome',
  initialised: true,
};

function setup(open = true) {
  const onappearance = vi.fn();
  const onclose = vi.fn();
  render(Preferences, { open, location: LOCATION, onappearance, onclose });
  return { onappearance, onclose, user: userEvent.setup() };
}

describe('Preferences', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.removeAttribute('data-text-size');
  });

  it('renders nothing when closed', () => {
    setup(false);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('applies a change immediately, with no OK button to press', async () => {
    const { onappearance, user } = setup();
    await user.click(screen.getByRole('radio', { name: 'Dark' }));

    // Three things, all at once: the DOM, the store, and the shell (which
    // forwards to the reader frame). A preference that changed only one of
    // them would look correct in the panel and be wrong in the page.
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(loadAppearance().theme).toBe('dark');
    expect(onappearance).toHaveBeenCalledWith(expect.objectContaining({ theme: 'dark' }));

    expect(screen.queryByRole('button', { name: /^(OK|Apply|Save)$/ })).not.toBeInTheDocument();
  });

  it('reset returns every appearance preference to its default', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('radio', { name: 'Dark' }));
    await user.click(screen.getByRole('radio', { name: 'Extra large' }));
    await user.click(screen.getByRole('button', { name: 'Reset to defaults' }));

    expect(loadAppearance()).toEqual({
      theme: 'system',
      textSize: 'default',
      measure: 'default',
      lineNumbers: false,
    });
    // System means no attribute, so the media query decides again.
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('stores the general preferences it shows', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('tab', { name: 'General' }));
    await user.click(screen.getByRole('checkbox', { name: 'Ask before removing a source' }));
    expect(preferences.confirmBeforeRemove.load()).toBe(false);
  });

  it('shows where the library lives, so the CLI and the app can be compared', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('tab', { name: 'Library' }));
    expect(screen.getByText(LOCATION.state_root)).toBeInTheDocument();
    expect(screen.getByText(LOCATION.cache_root)).toBeInTheDocument();
  });

  it('says shortcuts are not customisable rather than pretending', async () => {
    // P5-007 asked for a customisation tab. Nothing rebinds shortcuts, and
    // fields that discard what is typed into them are worse than a reference.
    const { user } = setup();
    await user.click(screen.getByRole('tab', { name: 'Keyboard' }));
    expect(screen.getByText(/not customisable/i)).toBeInTheDocument();
    expect(screen.getByText('⌘K')).toBeInTheDocument();
  });

  it('has no Sync tab, because there is no sync', async () => {
    setup();
    expect(screen.queryByRole('tab', { name: 'Sync' })).not.toBeInTheDocument();
  });

  it('records a keystroke as an accelerator and registers it', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('tab', { name: 'General' }));
    await user.click(screen.getByRole('checkbox', { name: 'Summon Tome from anywhere' }));

    // Enabling registers immediately, with the stored default.
    expect(invoked).toContainEqual(['set_global_shortcut', { accelerator: 'CmdOrCtrl+Shift+D' }]);

    await user.click(screen.getByRole('button', { name: 'Global shortcut' }));
    expect(screen.getByRole('button', { name: 'Global shortcut' })).toHaveTextContent(
      'Press keys…',
    );

    await user.keyboard('{Meta>}{Alt>}t{/Alt}{/Meta}');
    expect(invoked).toContainEqual(['set_global_shortcut', { accelerator: 'CmdOrCtrl+Alt+T' }]);
    // Displayed the way the key caps read, not the way Tauri parses it.
    expect(screen.getByRole('button', { name: 'Global shortcut' })).toHaveTextContent('⌘⌥T');
  });

  it('clears the shortcut rather than leaving it live when switched off', async () => {
    // Registering a replacement without releasing the previous one leaves both
    // working, with no way to discover why.
    preferences.globalShortcutEnabled.save(true);
    const { user } = setup();
    await user.click(screen.getByRole('tab', { name: 'General' }));
    await user.click(screen.getByRole('checkbox', { name: 'Summon Tome from anywhere' }));
    expect(invoked).toContainEqual(['set_global_shortcut', { accelerator: null }]);
  });

  it('shows why a shortcut could not be registered', async () => {
    // macOS reports a clash only by refusing to register, so this message is
    // the entire conflict-detection story.
    render(Preferences, {
      open: true,
      location: LOCATION,
      shortcutError: '`CmdOrCtrl+Shift+D` could not be registered',
    });
    await userEvent.setup().click(screen.getAllByRole('tab', { name: 'General' })[0]!);
    expect(screen.getAllByRole('alert')[0]).toHaveTextContent(/could not be registered/);
  });

  it('closes on Escape', async () => {
    const { onclose, user } = setup();
    await user.keyboard('{Escape}');
    expect(onclose).toHaveBeenCalled();
  });
});
