import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import App from './App.svelte';
import { invoked } from './test/setup';

describe('App', () => {
  it('asks the backend where the library lives', async () => {
    render(App);
    await screen.findByText('com.alexnodeland.tome');

    expect(invoked).toContainEqual(['library_location', undefined]);
  });

  it('shows both roots, so state and cache are visibly separate', async () => {
    render(App);

    expect(await screen.findByText(/Application Support\/Tome/)).toBeInTheDocument();
    expect(await screen.findByText(/Caches\/Tome/)).toBeInTheDocument();
  });

  it('has a reachable heading structure', async () => {
    render(App);

    // Queried by role, not by test id: if this fails, the markup is not
    // navigable by a screen reader either.
    expect(screen.getByRole('heading', { level: 1, name: 'Tome' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 2 })).toBeInTheDocument();
  });
});
