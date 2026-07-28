import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import App from './App.svelte';
import { invoked, mockResponses } from './test/setup';

/**
 * These assert the *contract* with Rust — which commands the shell invokes,
 * with what arguments — not Rust's behaviour, which Tier B integration tests
 * cover (`crates/tome-core/tests/reader_offline.rs`).
 */
describe('App', () => {
  beforeEach(() => {
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
    mockResponses.read_page = {
      source_id: 'fixture',
      path: 'index.html',
      title: 'Widget',
      html: '<div class="tome-page"><h1 id="widget">Widget</h1></div>',
      outline: [{ id: 'widget', title: 'Widget', level: 1, children: [] }],
    };
  });

  it('loads the first source and its first page on launch', async () => {
    render(App);
    await screen.findByRole('combobox', { name: 'Page' });

    expect(invoked).toContainEqual(['list_sources', undefined]);
    expect(invoked).toContainEqual(['list_pages', { sourceId: 'fixture' }]);
    expect(invoked).toContainEqual(['read_page', { sourceId: 'fixture', path: 'index.html' }]);
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

  it('has a reachable heading and labelled controls', async () => {
    // Queried by role and accessible name, not by test id: if these fail, the
    // shell is not navigable by a screen reader either.
    render(App);
    expect(screen.getByRole('heading', { level: 1, name: 'Tome' })).toBeInTheDocument();
    expect(await screen.findByRole('combobox', { name: 'Source' })).toBeInTheDocument();
    expect(await screen.findByRole('combobox', { name: 'Page' })).toBeInTheDocument();
  });
});
