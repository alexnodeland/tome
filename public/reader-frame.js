// The script that runs *inside* the sandboxed reader iframe (S1-13).
//
// It is served from the app origin and loaded by the frame's srcdoc via
// <script src="/reader-frame.js">. **It cannot be inline.** A srcdoc document
// inherits the parent document's CSP (local-scheme inheritance, measured in
// SPIKE-002), and the app CSP has no 'unsafe-inline' for scripts — which is
// the point, since the same policy is what stops inline event handlers in
// injected page content from running.
//
// The frame is sandbox="allow-scripts" with NO allow-same-origin, so its
// origin is opaque: no parent DOM access, no cookies, no storage, and
// window.__TAURI_INTERNALS__ is unreachable (SPIKE-002 finding 7 — the single
// most important boolean that spike produced). Everything crosses by
// postMessage.
//
// Three things here are not obvious:
//
//  1. **Nothing gates on requestAnimationFrame.** An occluded WKWebView
//     window suspends rAF entirely and clamps timers to ~230 ms (SPIKE-002
//     finding 8). Scroll reporting throttles on performance.now() deltas
//     inside a passive listener instead, so it degrades to "less often"
//     rather than to "never".
//  2. **Every link is intercepted.** The frame has no allow-top-navigation
//     and no allow-popups, so a click would either do nothing or navigate the
//     frame to a page Tome does not control. The app decides where a link
//     goes (S1-15); the frame only reports it.
//  3. **The scroll-spy runs here, not in the app.** The app cannot read this
//     document — that is the whole point of the sandbox — so the frame
//     computes which heading is current and posts it.
//  4. **In-page find (S2-8) runs here for the same reason**, and uses the CSS
//     Custom Highlight API rather than wrapping matches in <mark>. Wrapping
//     mutates the DOM: `Range.surroundContents` throws whenever a range
//     partially covers a node — which is the normal case, since a match
//     crosses the <span>s the syntax highlighter emits — and undoing the
//     wrapping afterwards has to restore a tree that was never meant to
//     change. `CSS.highlights` paints ranges without touching the document at
//     all, which also keeps the standing rule that highlighting is a *render*
//     concern rather than a mutation.

(() => {
  'use strict';

  /** Messages are only accepted from the parent that created this frame. */
  const fromParent = (event) => event.source === window.parent;

  // targetOrigin '*' is not laziness: this frame's origin is opaque ("null"),
  // and there is no origin string the parent could target it with. The
  // protection is the other direction — the parent checks event.source
  // against its own iframe's contentWindow, and nothing but our own content
  // is ever in here.
  const send = (message) => window.parent.postMessage(message, '*');

  let content = null;
  /** Heading elements, in document order, refreshed on each page load. */
  let headings = [];
  let lastScrollReport = 0;
  let activeHeadingId = null;

  /** In-page find state (S2-8). Ranges in document order. */
  let findRanges = [];
  let findIndex = -1;
  /** Whether this engine can paint ranges without mutating the document. */
  const canHighlight =
    typeof CSS !== 'undefined' &&
    typeof CSS.highlights !== 'undefined' &&
    typeof Highlight === 'function';

  function collectHeadings() {
    headings = Array.from(content.querySelectorAll('h1[id], h2[id], h3[id], h4[id]'));
  }

  /**
   * The heading the reader is currently "in": the last one whose top has
   * passed a line a little below the viewport top.
   *
   * The offset matters. Using the exact top means a heading is only ever
   * active for the single pixel it sits at, and the sidebar flickers between
   * two entries as the user scrolls.
   */
  function currentHeading() {
    const line = window.innerHeight * 0.2;
    let current = headings.length > 0 ? headings[0] : null;
    for (const heading of headings) {
      if (heading.getBoundingClientRect().top <= line) {
        current = heading;
      } else {
        break;
      }
    }
    return current ? current.id : null;
  }

  function reportScroll(force) {
    const now = performance.now();
    // ~60 Hz ceiling. A throttle rather than rAF, per the note above.
    if (!force && now - lastScrollReport < 16) return;
    lastScrollReport = now;

    const active = currentHeading();
    const changed = active !== activeHeadingId;
    activeHeadingId = active;

    send({
      type: 'scroll',
      top: window.scrollY,
      height: document.documentElement.scrollHeight,
      viewport: window.innerHeight,
      activeId: active,
      activeChanged: changed,
    });
  }

  // ------------------------------------------------------------ in-page find

  /**
   * Every text node under the content root, with the running offset at which
   * each begins in the concatenated text.
   *
   * Concatenating first is what lets a match span element boundaries:
   * `read_<span>to</span>_string` is one word to a reader and three text
   * nodes to the DOM, and searching node by node would never find it.
   */
  function textIndex() {
    const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        // Skip nothing structural — the renderer emits no <script> or <style>
        // into content — but an empty node contributes nothing and only makes
        // the offset table longer.
        return node.nodeValue.length > 0 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
      },
    });

    const nodes = [];
    let text = '';
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      nodes.push({ node, start: text.length });
      text += node.nodeValue;
    }
    return { nodes, text };
  }

  /** The text node containing global offset `offset`, and the local offset. */
  function locate(nodes, offset) {
    // Binary search: a long page has tens of thousands of text nodes, and a
    // linear scan per match end is what turns "1000+ matches" from the
    // ticket's success metric into a freeze.
    let low = 0;
    let high = nodes.length - 1;
    let found = 0;
    while (low <= high) {
      const middle = (low + high) >> 1;
      if (nodes[middle].start <= offset) {
        found = middle;
        low = middle + 1;
      } else {
        high = middle - 1;
      }
    }
    const entry = nodes[found];
    return { node: entry.node, offset: offset - entry.start };
  }

  /**
   * Find every occurrence of `query` and paint it.
   *
   * Case-insensitive, and **substring** rather than whole-word: this is
   * find-in-page, where a user typing `env` expects to land on `environment`.
   * That is the opposite of the search-result snippet rule, deliberately —
   * there a highlight claims "this is why the page matched", here it claims
   * only "this is what you typed".
   */
  function runFind(query) {
    clearFind();
    const needle = (query || '').toLowerCase();
    if (needle.length === 0) return;

    const { nodes, text } = textIndex();
    const haystack = text.toLowerCase();
    // `toLowerCase` can change a string's length for a few characters, which
    // would slide every offset after it. Fall back to case-sensitive matching
    // rather than highlighting the wrong text.
    const usable = haystack.length === text.length ? haystack : text;
    const target = usable === text ? query : needle;
    if (target.length === 0) return;

    for (let at = usable.indexOf(target); at !== -1; at = usable.indexOf(target, at + 1)) {
      const from = locate(nodes, at);
      const to = locate(nodes, at + target.length);
      const range = document.createRange();
      try {
        range.setStart(from.node, from.offset);
        range.setEnd(to.node, to.offset);
      } catch {
        // An offset past a node's length means the index and the DOM have
        // disagreed. Skip the match rather than abandoning the search.
        continue;
      }
      findRanges.push(range);
    }

    findIndex = findRanges.length > 0 ? 0 : -1;
    paintFind();
    if (findIndex >= 0) scrollToMatch();
  }

  function paintFind() {
    if (!canHighlight) return;
    if (findRanges.length === 0) {
      CSS.highlights.delete('tome-find');
      CSS.highlights.delete('tome-find-current');
      return;
    }
    CSS.highlights.set('tome-find', new Highlight(...findRanges));
    // The current match is its own highlight rather than a class, because
    // there is no element to put a class on — that is the point of not
    // mutating the DOM. Registration order decides which paints on top.
    if (findIndex >= 0) {
      CSS.highlights.set('tome-find-current', new Highlight(findRanges[findIndex]));
    } else {
      CSS.highlights.delete('tome-find-current');
    }
  }

  function scrollToMatch() {
    const range = findRanges[findIndex];
    // Guarded, because scrolling is cosmetic and reporting the count is not:
    // an engine without `Range.getBoundingClientRect` (jsdom has no layout)
    // would otherwise throw out of the message handler *before* the reply is
    // sent, and the find bar would sit at "no matches" for a page full of
    // them. Never let a nicety take the answer down with it.
    if (!range || typeof range.getBoundingClientRect !== 'function') return;
    const rect = range.getBoundingClientRect();
    // Only scroll when the match is out of view, so stepping through matches
    // that share a paragraph does not jerk the page on every press.
    if (rect.top >= 0 && rect.bottom <= window.innerHeight) return;
    window.scrollTo({
      top: window.scrollY + rect.top - window.innerHeight / 3,
      behavior: 'auto',
    });
  }

  function stepFind(direction) {
    if (findRanges.length === 0) return;
    // Wraps at both ends: without it, pressing Enter at the last match does
    // nothing and looks like the key stopped working.
    findIndex = (findIndex + direction + findRanges.length) % findRanges.length;
    paintFind();
    scrollToMatch();
  }

  function clearFind() {
    findRanges = [];
    findIndex = -1;
    if (canHighlight) {
      CSS.highlights.delete('tome-find');
      CSS.highlights.delete('tome-find-current');
    }
  }

  function reportFind() {
    send({
      type: 'findResults',
      total: findRanges.length,
      // 1-based for display, 0 when there is nothing to show. The app should
      // not have to know that -1 means "none".
      index: findIndex >= 0 ? findIndex + 1 : 0,
      supported: canHighlight,
    });
  }

  function showPage(message) {
    // `innerHTML` is safe here in the specific sense that matters: the HTML5
    // parser refuses to execute <script> inserted this way, and the CSP stops
    // inline handlers (both measured in SPIKE-002). It is NOT a substitute
    // for the sanitizer — the walls are stacked, per sanitize.rs.
    content.innerHTML = message.html;
    collectHeadings();
    activeHeadingId = null;
    // Ranges point into the document that has just been replaced. Keeping
    // them would paint nothing and report a count for text that is gone.
    clearFind();

    // Restore position before reporting, so the app never sees a transient
    // "scrolled to top" for a page it asked to restore.
    if (message.fragment) {
      scrollToId(message.fragment, 'auto');
    } else {
      window.scrollTo({ top: message.scrollTop || 0, behavior: 'auto' });
    }

    send({ type: 'loaded', token: message.token, height: document.documentElement.scrollHeight });
    reportScroll(true);
  }

  function scrollToId(id, behavior) {
    // Not `querySelector('#' + id)`: ids in real documentation contain
    // characters that are valid in an id and not in a CSS selector —
    // `impl-From<T>-for-T` from rustdoc, `json.dumps` from Sphinx. A naive
    // selector build throws on those and the deep link silently does nothing.
    const target = document.getElementById(id);
    if (!target) return false;
    target.scrollIntoView({ behavior: behavior || 'smooth', block: 'start' });
    return true;
  }

  window.addEventListener('message', (event) => {
    if (!fromParent(event)) return;
    const message = event.data;
    if (!message || typeof message !== 'object') return;

    switch (message.type) {
      case 'page':
        showPage(message);
        break;
      case 'scrollTo': {
        const found = scrollToId(message.id, message.behavior);
        send({ type: 'scrolledTo', id: message.id, found });
        break;
      }
      case 'scrollTop':
        window.scrollTo({ top: message.top || 0, behavior: 'auto' });
        reportScroll(true);
        break;
      case 'find':
        runFind(message.query);
        reportFind();
        break;
      case 'findStep':
        stepFind(message.direction === -1 ? -1 : 1);
        reportFind();
        break;
      case 'findClear':
        clearFind();
        reportFind();
        break;
      case 'settings':
        // Theme, text size, and line numbers are all attributes on <html>,
        // read by CSS. No re-render, no re-highlight, no JavaScript styling.
        for (const [name, value] of Object.entries(message.attributes || {})) {
          if (value === null) {
            document.documentElement.removeAttribute(name);
          } else {
            document.documentElement.setAttribute(name, String(value));
          }
        }
        break;
    }
  });

  document.addEventListener('DOMContentLoaded', () => {
    content = document.getElementById('tome-content');

    document.addEventListener(
      'click',
      (event) => {
        const anchor = event.target.closest && event.target.closest('a[href]');
        if (!anchor) return;
        // Always. Even a same-page fragment goes through the app, so that
        // history (S1-15) records it and the back button works the way a
        // reader expects.
        event.preventDefault();
        send({
          type: 'navigate',
          href: anchor.getAttribute('href'),
          // Not `anchor.href`: in an opaque-origin document a relative href
          // resolves against about:srcdoc and comes back as an unusable
          // absolute URL. The attribute is what the renderer wrote — a
          // library path for internal links, a full URL for external ones.
          modified: event.metaKey || event.ctrlKey || event.shiftKey,
        });
      },
      true,
    );

    window.addEventListener('scroll', () => reportScroll(false), { passive: true });
    window.addEventListener('resize', () => reportScroll(true), { passive: true });

    send({
      type: 'ready',
      // Reported so a regression is visible in the console rather than
      // silent: if this is ever true, page content can reach the IPC layer.
      tauriReachable: typeof window.__TAURI_INTERNALS__ !== 'undefined',
    });
  });
})();
