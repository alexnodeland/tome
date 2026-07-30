import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Onboarding from './Onboarding.svelte';
import { invoked, mockResponses } from '../../test/setup';
import type { CatalogueEntry } from '$lib/tauri';

const CATALOGUE: CatalogueEntry[] = [
  {
    id: 'rust-std',
    name: 'Rust Standard Library',
    category: 'Rust',
    homepage: 'https://doc.rust-lang.org/std/',
    licence: 'MIT OR Apache-2.0',
    verified: '2026-07-30',
    installed: false,
  },
  {
    id: 'cargo-book',
    name: 'The Cargo Book',
    category: 'Rust',
    homepage: 'https://doc.rust-lang.org/cargo/',
    licence: 'MIT OR Apache-2.0',
    verified: '2026-07-30',
    installed: true,
  },
  {
    id: 'python',
    name: 'Python 3',
    category: 'Python',
    homepage: 'https://docs.python.org/3/',
    licence: 'PSF-2.0',
    verified: '2026-07-30',
    installed: false,
  },
];

// The event bus is not stubbed centrally: only this component listens, and a
// global stub would hide a component that forgot to unlisten.
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

function setup() {
  const oninstalled = vi.fn();
  const onclose = vi.fn();
  render(Onboarding, { oninstalled, onclose });
  return { oninstalled, onclose, user: userEvent.setup() };
}

describe('Onboarding', () => {
  beforeEach(() => {
    mockResponses.registry_catalogue = CATALOGUE;
    mockResponses.install_registry_source = {
      source_id: 'rust-std',
      pages: 42,
      page_errors: 0,
      capped: false,
    };
  });

  it('opens on the welcome step and can be skipped without touching anything', async () => {
    const { onclose, user } = setup();
    await user.click(screen.getByRole('button', { name: 'Skip' }));
    expect(onclose).toHaveBeenCalled();
    // P5-006: onboarding never blocks the app, and skipping is not an install.
    expect(invoked.some(([cmd]) => cmd === 'install_registry_source')).toBe(false);
  });

  it('offers the bundled catalogue grouped by category', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: 'Add your first source' }));

    expect(await screen.findByText('Rust Standard Library')).toBeInTheDocument();
    expect(screen.getByText('Python 3')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Rust' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Python' })).toBeInTheDocument();
  });

  it('shows the verification date rather than hiding it', async () => {
    // A stale date is the only warning a user gets that a scraper has rotted
    // (RISK-003). Hiding it would make the registry look uniformly fresh.
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: 'Add your first source' }));
    expect(await screen.findAllByText(/verified 2026-07-30/)).toHaveLength(CATALOGUE.length);
  });

  it('does not offer to install something already installed', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: 'Add your first source' }));
    await screen.findByText('The Cargo Book');
    // Two installable entries, and the third says so instead.
    expect(screen.getAllByRole('button', { name: 'Install' })).toHaveLength(2);
    expect(screen.getByText('Installed')).toBeInTheDocument();
  });

  it('installs in one click and tells the shell which source arrived', async () => {
    const { oninstalled, user } = setup();
    await user.click(screen.getByRole('button', { name: 'Add your first source' }));
    await screen.findByText('Rust Standard Library');

    const [first] = screen.getAllByRole('button', { name: 'Install' });
    await user.click(first!);

    await waitFor(() => expect(oninstalled).toHaveBeenCalledWith('rust-std'));
    expect(invoked).toContainEqual(['install_registry_source', { id: 'rust-std' }]);
    // And it moves on to the shortcuts, rather than leaving the user on a
    // list where the thing they just installed now says "Installed".
    expect(await screen.findByRole('heading', { name: 'Ready.' })).toBeInTheDocument();
  });

  it('reports an install failure instead of leaving a spinner', async () => {
    // P5-006: works with no network — explains what it needs rather than
    // failing opaquely.
    mockResponses.install_registry_source = undefined;
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: 'Add your first source' }));
    await screen.findByText('Rust Standard Library');

    const [first] = screen.getAllByRole('button', { name: 'Install' });
    await user.click(first!);

    expect(await screen.findByRole('alert')).toBeInTheDocument();
    // Still on the catalogue, still able to try something else.
    expect(screen.getByText('Python 3')).toBeInTheDocument();
  });

  it('lists only shortcuts that are actually bound', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: 'Add your first source' }));
    await screen.findByText('Rust Standard Library');
    const [first] = screen.getAllByRole('button', { name: 'Install' });
    await user.click(first!);
    await screen.findByRole('heading', { name: 'Ready.' });

    expect(screen.getByText('⌘K')).toBeInTheDocument();
    // ⌘D is Appendix C's bookmark shortcut and nothing binds it. A panel that
    // listed it would teach the user something false.
    expect(screen.queryByText('⌘D')).not.toBeInTheDocument();
  });
});
