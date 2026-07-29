/**
 * In-page find, exercised by running the real frame bootstrap (S2-8, P2-007).
 *
 * `public/reader-frame.js` is a plain script served to the sandboxed iframe,
 * not a module, so it is normally invisible to tests — `bridge.test.ts` says
 * as much and asserts only the protocol. But the substantive part of find is
 * in that file: building ranges over a text index that spans element
 * boundaries, and locating an offset by binary search. Both are exactly the
 * kind of thing that fails on a page shape nobody tried by hand.
 *
 * So the file is evaluated here against a jsdom document. `window.parent` is
 * `window` in jsdom, so the frame's replies arrive as ordinary messages on the
 * same window and can be collected.
 *
 * **What this cannot cover:** `CSS.highlights` does not exist in jsdom, so the
 * painting is inert and `supported` comes back `false`. That is deliberately
 * useful — it exercises the degraded path — but it means the *appearance* of a
 * highlight is not tested here. It is asserted in `reader.css` by the design
 * gate and in the app by looking at it.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

// `process.cwd()` rather than `import.meta.url`: under jsdom the module URL is
// an http: URL served by Vite, and `fileURLToPath` rejects it. Vitest runs
// from the project root.
const SCRIPT = readFileSync(join(process.cwd(), 'public/reader-frame.js'), 'utf8');

interface FindResults {
  type: 'findResults';
  total: number;
  index: number;
  supported: boolean;
}

let replies: unknown[] = [];

/**
 * Evaluate the bootstrap exactly once.
 *
 * Per-test evaluation stacks a `message` listener and a content root each
 * time, so the second test sees two frames answering and the third sees
 * three. The frame is designed to live as long as its document, and the test
 * mirrors that: one boot, many pages.
 */
beforeAll(() => {
  document.body.innerHTML = '<div id="tome-content"></div>';
  window.addEventListener('message', (event: MessageEvent) => replies.push(event.data));
  // The frame bootstrap is a plain script, not a module: there is nothing to
  // import. Evaluating it is the only way to test what it actually does.
  new Function(SCRIPT)();
  document.dispatchEvent(new Event('DOMContentLoaded'));
});

/** Hand the frame a page, as a navigation would. */
async function boot(html: string): Promise<void> {
  await post({ type: 'page', html, fragment: null, scrollTop: 0, token: 1 });
  replies = [];
}

/**
 * Deliver a message as the parent would, and wait for the reply.
 *
 * The await is load-bearing: the frame answers with `window.parent.postMessage`,
 * which is asynchronous — jsdom queues it as a task — so a synchronous
 * assertion after this call reads the state from *before* the message was
 * handled. Every test here would have passed vacuously.
 */
async function post(message: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data: message, source: window }));
  await new Promise((resolve) => setTimeout(resolve, 0));
}

/** The most recent find report. */
function lastFind(): FindResults {
  const found = [...replies].reverse().find((m) => (m as FindResults)?.type === 'findResults');
  if (!found) throw new Error(`no findResults in ${JSON.stringify(replies)}`);
  return found as FindResults;
}

beforeEach(() => {
  // jsdom implements neither, and an unimplemented-method error from a
  // *cosmetic* scroll would fail a test about counting.
  vi.stubGlobal('scrollTo', vi.fn());
  Element.prototype.scrollIntoView = vi.fn();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('in-page find', () => {
  it('counts every occurrence and starts on the first', async () => {
    await boot('<p>alpha beta alpha gamma alpha</p>');
    await post({ type: 'find', query: 'alpha' });
    expect(lastFind()).toMatchObject({ total: 3, index: 1 });
  });

  it('matches across element boundaries', async () => {
    // The reason the frame concatenates text first. The syntax highlighter
    // emits a <span> per token, so `read_to_string` in a code block is three
    // text nodes; searching node by node would never find it.
    await boot('<p>let s = read_<span>to</span>_string(path);</p>');
    await post({ type: 'find', query: 'read_to_string' });
    expect(lastFind()).toMatchObject({ total: 1, index: 1 });
  });

  it('is case-insensitive', async () => {
    await boot('<p>Environment ENVIRONMENT environment</p>');
    await post({ type: 'find', query: 'environment' });
    expect(lastFind().total).toBe(3);
  });

  it('matches substrings, unlike a search snippet', async () => {
    // Deliberately the opposite rule: a snippet highlight claims "this is why
    // the page matched", so it marks whole terms. Find-in-page claims only
    // "this is what you typed", and someone typing `env` expects to land on
    // `environment`.
    await boot('<p>environment</p>');
    await post({ type: 'find', query: 'env' });
    expect(lastFind().total).toBe(1);
  });

  it('steps forward and backward, wrapping at both ends', async () => {
    // Without wrapping, pressing Enter at the last match does nothing and
    // looks like the key stopped working.
    await boot('<p>a a a</p>');
    await post({ type: 'find', query: 'a' });
    expect(lastFind().index).toBe(1);

    await post({ type: 'findStep', direction: 1 });
    expect(lastFind().index).toBe(2);
    await post({ type: 'findStep', direction: 1 });
    expect(lastFind().index).toBe(3);
    await post({ type: 'findStep', direction: 1 });
    expect(lastFind().index).toBe(1);

    await post({ type: 'findStep', direction: -1 });
    expect(lastFind().index).toBe(3);
  });

  it('reports nothing for a query that does not occur', async () => {
    await boot('<p>alpha beta</p>');
    await post({ type: 'find', query: 'gamma' });
    expect(lastFind()).toMatchObject({ total: 0, index: 0 });
  });

  it('reports nothing for an empty query rather than matching everywhere', async () => {
    // An empty needle matches at every position; without the guard, a page of
    // 50 000 characters would build 50 000 ranges the moment the field is
    // cleared.
    await boot('<p>alpha beta</p>');
    await post({ type: 'find', query: '' });
    expect(lastFind()).toMatchObject({ total: 0, index: 0 });
  });

  it('clears on request', async () => {
    await boot('<p>alpha alpha</p>');
    await post({ type: 'find', query: 'alpha' });
    expect(lastFind().total).toBe(2);
    await post({ type: 'findClear' });
    expect(lastFind()).toMatchObject({ total: 0, index: 0 });
  });

  it('drops its matches when the page is replaced', async () => {
    // The ranges point into a document that no longer exists. Keeping them
    // would report a count for text that is gone.
    await boot('<p>alpha alpha</p>');
    await post({ type: 'find', query: 'alpha' });
    expect(lastFind().total).toBe(2);

    await post({ type: 'page', html: '<p>beta</p>', fragment: null, scrollTop: 0, token: 2 });
    await post({ type: 'findStep', direction: 1 });
    expect(lastFind()).toMatchObject({ total: 0, index: 0 });
  });

  it('stepping with no matches does nothing rather than throwing', async () => {
    await boot('<p>alpha</p>');
    await post({ type: 'findStep', direction: 1 });
    expect(lastFind()).toMatchObject({ total: 0, index: 0 });
  });

  it('finds matches spread across many blocks', async () => {
    const paragraphs = Array.from({ length: 200 }, (_, i) => `<p>line ${i} needle here</p>`).join(
      '',
    );
    await boot(paragraphs);
    await post({ type: 'find', query: 'needle' });
    // P2-007's "works with 1000+ matches" in miniature: the offset lookup is a
    // binary search precisely so this does not become quadratic.
    expect(lastFind().total).toBe(200);
  });

  it('handles multi-byte text without slicing a character apart', async () => {
    await boot('<p>日本語のテキスト needle 日本語のテキスト</p>');
    await post({ type: 'find', query: 'needle' });
    expect(lastFind().total).toBe(1);

    await boot('<p>日本語 日本語</p>');
    await post({ type: 'find', query: '日本語' });
    expect(lastFind().total).toBe(2);
  });

  it('reports whether the engine can paint highlights at all', async () => {
    // jsdom has no CSS.highlights, so this is the degraded path — and the
    // point is that it is *reported* rather than silently finding nothing.
    await boot('<p>alpha</p>');
    await post({ type: 'find', query: 'alpha' });
    expect(lastFind().supported).toBe(false);
  });

  it('ignores messages that did not come from the parent', async () => {
    // The frame's whole security posture is that only its parent talks to it.
    await boot('<p>alpha alpha</p>');
    window.dispatchEvent(
      new MessageEvent('message', { data: { type: 'find', query: 'alpha' }, source: null }),
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(replies.some((m) => (m as FindResults)?.type === 'findResults')).toBe(false);
  });
});
