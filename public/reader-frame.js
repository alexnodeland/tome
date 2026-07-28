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

  function showPage(message) {
    // `innerHTML` is safe here in the specific sense that matters: the HTML5
    // parser refuses to execute <script> inserted this way, and the CSP stops
    // inline handlers (both measured in SPIKE-002). It is NOT a substitute
    // for the sanitizer — the walls are stacked, per sanitize.rs.
    content.innerHTML = message.html;
    collectHeadings();
    activeHeadingId = null;

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
