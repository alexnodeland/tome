import { describe, it, expect } from 'vitest';
import { classifyLink, NavigationHistory, type HistoryEntry } from './navigation';

function entry(path: string, over: Partial<HistoryEntry> = {}): HistoryEntry {
  return { sourceId: 'demo', path, fragment: null, scrollTop: 0, ...over };
}

describe('NavigationHistory', () => {
  it('starts empty and goes nowhere', () => {
    const history = new NavigationHistory();
    expect(history.current).toBeNull();
    expect(history.canGoBack()).toBe(false);
    expect(history.canGoForward()).toBe(false);
    expect(history.back()).toBeNull();
    expect(history.forward()).toBeNull();
  });

  it('moves back and forward through what was visited', () => {
    const history = new NavigationHistory();
    history.push(entry('a.html'));
    history.push(entry('b.html'));
    history.push(entry('c.html'));

    expect(history.canGoForward()).toBe(false);
    expect(history.back()?.path).toBe('b.html');
    expect(history.back()?.path).toBe('a.html');
    expect(history.canGoBack()).toBe(false);
    expect(history.forward()?.path).toBe('b.html');
    expect(history.forward()?.path).toBe('c.html');
    expect(history.canGoForward()).toBe(false);
  });

  it('truncates the forward branch when you go somewhere new', () => {
    // Browser behaviour, and the reason `push` is not just an append: after
    // going back, the pages ahead are a future that no longer happens.
    const history = new NavigationHistory();
    history.push(entry('a.html'));
    history.push(entry('b.html'));
    history.push(entry('c.html'));
    history.back();
    history.push(entry('d.html'));

    expect(history.canGoForward()).toBe(false);
    expect(history.snapshot().map((e) => e.path)).toEqual(['a.html', 'b.html', 'd.html']);
  });

  it('replaces rather than duplicating when you go where you already are', () => {
    // Sphinx puts a permalink on every heading. Without this, following one
    // would push a duplicate and make the back button undo nothing visible.
    const history = new NavigationHistory();
    history.push(entry('a.html'));
    history.push(entry('a.html'));

    expect(history.length).toBe(1);
    expect(history.canGoBack()).toBe(false);
  });

  it('treats a different fragment on the same page as a different place', () => {
    const history = new NavigationHistory();
    history.push(entry('a.html'));
    history.push(entry('a.html', { fragment: 'section' }));

    expect(history.length).toBe(2);
    expect(history.back()?.fragment).toBeNull();
  });

  it('treats the same path in a different source as a different place', () => {
    // Cross-source navigation: `(source, path)` is the identity everywhere
    // else in the system, and history has to agree.
    const history = new NavigationHistory();
    history.push(entry('index.html', { sourceId: 'rust' }));
    history.push(entry('index.html', { sourceId: 'python' }));

    expect(history.length).toBe(2);
    expect(history.back()?.sourceId).toBe('rust');
  });

  it('remembers where each page was scrolled to', () => {
    const history = new NavigationHistory();
    history.push(entry('a.html'));
    history.recordScroll(1200);
    history.push(entry('b.html'));
    history.recordScroll(40);

    expect(history.back()?.scrollTop).toBe(1200);
    expect(history.forward()?.scrollTop).toBe(40);
  });

  it('does not turn scrolling into navigation', () => {
    // A history entry per scroll event would make the back button useless
    // within a page.
    const history = new NavigationHistory();
    history.push(entry('a.html'));
    for (let i = 0; i < 100; i++) history.recordScroll(i * 10);

    expect(history.length).toBe(1);
  });

  it('keeps working past its limit by dropping the oldest, not the newest', () => {
    // P1-020's metric is "history survives 1000+ entries". A reader who has
    // visited that many wants back to keep working, not to be told the
    // history is full.
    const history = new NavigationHistory(3);
    for (const path of ['a', 'b', 'c', 'd', 'e']) history.push(entry(`${path}.html`));

    expect(history.length).toBe(3);
    expect(history.current?.path).toBe('e.html');
    expect(history.snapshot().map((e) => e.path)).toEqual(['c.html', 'd.html', 'e.html']);
    expect(history.back()?.path).toBe('d.html');
  });

  it('hands out copies, so a caller cannot corrupt the stack', () => {
    const history = new NavigationHistory();
    const original = entry('a.html');
    history.push(original);
    original.path = 'mutated.html';

    expect(history.current?.path).toBe('a.html');
  });
});

describe('classifyLink', () => {
  it('recognises a same-page fragment', () => {
    expect(classifyLink('#widget.Widget')).toEqual({
      kind: 'fragment',
      fragment: 'widget.Widget',
    });
  });

  it('recognises a library page, with and without a fragment', () => {
    expect(classifyLink('api/reference.html')).toEqual({
      kind: 'page',
      path: 'api/reference.html',
      fragment: null,
    });
    expect(classifyLink('api/reference.html#resize')).toEqual({
      kind: 'page',
      path: 'api/reference.html',
      fragment: 'resize',
    });
  });

  it('recognises an external link', () => {
    for (const url of [
      'https://example.com/x',
      'http://example.com/x',
      'mailto:someone@example.org',
    ]) {
      expect(classifyLink(url).kind).toBe('external');
    }
  });

  it('treats a protocol-relative URL as external', () => {
    // `//host/x` has no scheme, so a naive scheme test calls it a page — and
    // it would be looked up as a library path that cannot exist. It is
    // external navigation whatever the renderer thought.
    expect(classifyLink('//cdn.example/x')).toEqual({
      kind: 'external',
      url: '//cdn.example/x',
    });
  });

  it('treats anything script-capable as external, for Rust to refuse', () => {
    // The frontend does not decide what is safe to hand to the OS — it
    // classifies, and `open_external` in Rust validates against an
    // allowlist. Calling these "pages" would be worse: they would be looked
    // up as library paths.
    expect(classifyLink('javascript:alert(1)').kind).toBe('external');
    expect(classifyLink('data:text/html,x').kind).toBe('external');
  });

  it('ignores surrounding whitespace', () => {
    expect(classifyLink('  a.html  ')).toEqual({ kind: 'page', path: 'a.html', fragment: null });
  });

  it('treats an empty fragment as no fragment', () => {
    expect(classifyLink('a.html#')).toEqual({ kind: 'page', path: 'a.html', fragment: null });
  });
});
