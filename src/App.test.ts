import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import App from './App.svelte';
import { invoked, mockResponses } from './test/setup';

/**
 * These assert the *contract* with Rust — which commands the shell invokes,
 * with what arguments — not Rust's behaviour, which Tier B integration tests
 * cover (`crates/tome-core/tests/reader_offline.rs`).
 */
const READ_PAGE = {
  source_id: 'fixture',
  path: 'index.html',
  title: 'Widget',
  html: '<div class="tome-page"><h1 id="widget">Widget</h1></div>',
  outline: [{ id: 'widget', title: 'Widget', level: 1, children: [] }],
};

describe('App', () => {
  beforeEach(() => {
    localStorage.clear();
    // jsdom's default 1024 is exactly the right-sidebar breakpoint.
    window.innerWidth = 1400;
    mockResponses.list_sources = [
      {
        id: 'fixture',
        name: 'Fixture Docs',
        category: 'Uncategorized',
        page_count: 2,
        last_synced: null,
      },
    ];
    mockResponses.list_pages = [
      { path: 'index.html', title: 'Widget' },
      { path: 'api/reference.html', title: 'API reference' },
    ];
    mockResponses.read_page = { ...READ_PAGE };
  });

  it('loads the first source and its first page on launch', async () => {
    render(App);
    await screen.findByTitle('Documentation');

    expect(invoked).toContainEqual(['list_sources', undefined]);
    expect(invoked).toContainEqual(['list_pages', { sourceId: 'fixture' }]);
    expect(invoked).toContainEqual(['read_page', { sourceId: 'fixture', path: 'index.html' }]);
  });

  it('shows the three panels', async () => {
    render(App);
    await screen.findByTitle('Documentation');

    expect(screen.getByRole('navigation', { name: 'Documentation sources' })).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'On this page' })).toBeInTheDocument();
  });

  it('draws the outline the renderer produced', async () => {
    render(App);
    await screen.findByTitle('Documentation');

    // The outline comes from Rust with every entry carrying an anchor; the
    // sidebar is only allowed to draw it, never to derive it. Scoped to the
    // outline landmark because the page is also called "Widget" in the
    // library list, and an unscoped query would match either.
    const outline = screen.getByRole('navigation', { name: 'On this page' });
    expect(within(outline).getByRole('button', { name: 'Widget' })).toBeInTheDocument();
  });

  it('opens a page when one is chosen in the library', async () => {
    const user = userEvent.setup();
    render(App);
    await screen.findByTitle('Documentation');
    invoked.length = 0;

    await user.click(screen.getByRole('button', { name: 'API reference' }));
    expect(invoked).toContainEqual([
      'read_page',
      { sourceId: 'fixture', path: 'api/reference.html' },
    ]);
  });

  it('toggles the sidebars with the documented shortcuts', async () => {
    // PRD Appendix C: Cmd+1 library, Cmd+2 outline, Cmd+backslash both.
    const user = userEvent.setup();
    render(App);
    await screen.findByTitle('Documentation');

    await user.keyboard('{Meta>}1{/Meta}');
    expect(
      screen.queryByRole('navigation', { name: 'Documentation sources' }),
    ).not.toBeInTheDocument();

    await user.keyboard('{Meta>}1{/Meta}');
    expect(screen.getByRole('navigation', { name: 'Documentation sources' })).toBeInTheDocument();

    await user.keyboard('{Meta>}2{/Meta}');
    expect(screen.queryByRole('navigation', { name: 'On this page' })).not.toBeInTheDocument();
  });

  it('puts the reader in a sandboxed frame with no same-origin access', async () => {
    // The isolation the whole design rests on, asserted at the level someone
    // would actually break it: by editing the markup.
    render(App);
    const frame = await screen.findByTitle('Documentation');

    expect(frame.tagName).toBe('IFRAME');
    expect(frame.getAttribute('sandbox')).toBe('allow-scripts');
  });

  it('never puts page HTML into the app document', async () => {
    // The app webview holds the IPC bridge. Page HTML belongs in the frame
    // and only in the frame; if this fails, the sandbox has been bypassed by
    // the app rather than by an attacker.
    render(App);
    await screen.findByTitle('Documentation');

    expect(document.body.innerHTML).not.toContain('tome-page');
    expect(document.body.innerHTML).not.toContain('<h1 id="widget">');
  });

  it('says what to do when the library is empty rather than showing nothing', async () => {
    mockResponses.list_sources = [];
    render(App);

    expect(await screen.findByText(/library is empty/i)).toBeInTheDocument();
    expect(await screen.findByText(/tome pull/)).toBeInTheDocument();
  });

  it('surfaces a backend error instead of failing silently', async () => {
    delete mockResponses.list_pages;
    render(App);

    expect(await screen.findByRole('alert')).toHaveTextContent(/list_pages/);
  });

  it('has a reachable heading and labelled landmarks', async () => {
    // Queried by role and accessible name, not by test id: if these fail, the
    // shell is not navigable by a screen reader either.
    render(App);
    expect(screen.getByRole('heading', { level: 1 })).toBeInTheDocument();
    expect(await screen.findByRole('searchbox', { name: /filter/i })).toBeInTheDocument();
    expect(screen.getByRole('complementary', { name: 'Library' })).toBeInTheDocument();
    expect(screen.getByRole('complementary', { name: 'On this page' })).toBeInTheDocument();
  });

  it('starts with both history buttons disabled', async () => {
    render(App);
    await screen.findByTitle('Documentation');

    expect(screen.getByRole('button', { name: 'Back' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Forward' })).toBeDisabled();
  });

  it('goes back and forward through visited pages', async () => {
    const user = userEvent.setup();
    render(App);
    await screen.findByTitle('Documentation');

    await user.click(screen.getByRole('button', { name: 'API reference' }));
    mockResponses.read_page = { ...READ_PAGE, path: 'api/reference.html', title: 'API reference' };
    const back = screen.getByRole('button', { name: 'Back' });
    expect(back).toBeEnabled();

    invoked.length = 0;
    await user.click(back);
    expect(invoked).toContainEqual(['read_page', { sourceId: 'fixture', path: 'index.html' }]);
    expect(screen.getByRole('button', { name: 'Forward' })).toBeEnabled();

    invoked.length = 0;
    await user.click(screen.getByRole('button', { name: 'Forward' }));
    expect(invoked).toContainEqual([
      'read_page',
      { sourceId: 'fixture', path: 'api/reference.html' },
    ]);
  });

  it('binds back and forward to the documented shortcuts', async () => {
    // PRD Appendix C: Cmd+[ and Cmd+]. Dispatched directly rather than
    // through userEvent.keyboard, whose mini-language reserves `[` and `]`
    // for key codes — escaping them would obscure which keys are meant.
    const user = userEvent.setup();
    render(App);
    await screen.findByTitle('Documentation');
    await user.click(screen.getByRole('button', { name: 'API reference' }));

    const press = async (key: string) => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key, metaKey: true, bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 0));
    };

    invoked.length = 0;
    await press('[');
    expect(invoked).toContainEqual(['read_page', { sourceId: 'fixture', path: 'index.html' }]);

    invoked.length = 0;
    await press(']');
    expect(invoked).toContainEqual([
      'read_page',
      { sourceId: 'fixture', path: 'api/reference.html' },
    ]);
  });

  it('hands an external link to Rust rather than deciding itself', async () => {
    // The frontend classifies; `open_external` validates against an
    // allowlist. Nothing here judges whether a URL is safe to hand to the OS.
    render(App);
    await screen.findByTitle('Documentation');
    mockResponses.open_external = null;

    window.dispatchEvent(
      new MessageEvent('message', {
        data: { type: 'navigate', href: 'https://example.com/x', modified: false },
        source: (document.querySelector('iframe') as HTMLIFrameElement).contentWindow,
      }),
    );
    await Promise.resolve();

    expect(invoked).toContainEqual(['open_external', { url: 'https://example.com/x' }]);
  });

  it('titles the window bar with the page, not with the app name', async () => {
    render(App);
    expect(await screen.findByRole('heading', { level: 1, name: 'Widget' })).toBeInTheDocument();
  });
});
