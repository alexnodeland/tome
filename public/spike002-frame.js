// SPIKE-002: the script that runs *inside* the sandboxed reader iframe.
//
// This file is served from the app origin and loaded by the iframe's srcdoc
// via <script src="/spike002-frame.js">. It cannot be inline: a srcdoc
// document inherits the parent document's CSP (local-scheme inheritance), and
// the app CSP has no 'unsafe-inline' for scripts — which is the point, since
// the same policy is what stops inline handlers in injected page content.
//
// The frame is sandbox="allow-scripts" with NO allow-same-origin, so its
// origin is opaque: no parent DOM access, no cookies, and every exchange with
// the app goes through postMessage. That is the production reader design
// (PRD § Technical Architecture); this file is its measurement double.
// Remove together with the rest of the SPIKE-002 harness when S1-13 lands.

(() => {
  'use strict';

  const send = (msg) => window.parent.postMessage(msg, '*');

  const violations = [];
  document.addEventListener('securitypolicyviolation', (e) => {
    violations.push(`${e.effectiveDirective}:${e.blockedURI || 'inline'}`);
  });

  let chunks = [];
  let content = null;

  function inject(html) {
    content = document.getElementById('content');
    const t0 = performance.now();
    content.innerHTML = html;
    const parseMs = performance.now() - t0;
    // Ack synchronously: WKWebView suspends rAF when the window is occluded,
    // and a headless capture run must not hang on a compositor that never
    // ticks. Paint timing arrives separately, if and when rAF fires.
    send({
      type: 'injected',
      parseMs,
      elements: content.querySelectorAll('*').length,
      violations: violations.slice(),
    });
    requestAnimationFrame(() => {
      send({ type: 'painted', sinceInjectMs: performance.now() - t0 });
    });
  }

  // The annotation-anchoring probe: does window.getSelection() work in an
  // opaque-origin sandboxed frame well enough to derive quote + prefix/suffix
  // (the anchor format — never character offsets)? Selects from mid-paragraph
  // across an element boundary into a <code> span, the awkward case.
  function selectionProbe() {
    const paragraphs = content ? content.querySelectorAll('section p') : [];
    if (paragraphs.length < 2) {
      send({ type: 'selection', ok: false, why: 'no content injected' });
      return;
    }
    const p = paragraphs[1];
    const textNode = p.firstChild; // "Paragraph 1 of section 1..."
    const code = p.querySelector('code');
    const range = document.createRange();
    range.setStart(textNode, 10);
    range.setEnd(code.firstChild, 5);
    const sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(range);

    const live = sel.rangeCount === 1 ? sel.getRangeAt(0) : null;
    const quote = sel.toString();

    // Prefix/suffix from ranges anchored to the content root, the same way
    // the real anchoring code will do it.
    const before = document.createRange();
    before.setStart(content, 0);
    before.setEnd(range.startContainer, range.startOffset);
    const after = document.createRange();
    after.setStart(range.endContainer, range.endOffset);
    after.setEnd(content, content.childNodes.length);

    send({
      type: 'selection',
      ok: live !== null && quote.length > 0,
      quoteLength: quote.length,
      quote: quote.slice(0, 60),
      prefix: before.toString().slice(-32),
      suffix: after.toString().slice(0, 32),
      crossesElementBoundary: range.startContainer !== range.endContainer,
    });
  }

  window.addEventListener('message', (e) => {
    const m = e.data;
    if (!m || typeof m !== 'object') return;
    switch (m.type) {
      case 'echo':
        send({ type: 'echo-ack', id: m.id, bytes: m.payload.length });
        break;
      case 'chunk':
        chunks.push(m.payload);
        if (m.last) {
          const total = chunks.join('').length;
          chunks = [];
          send({ type: 'chunks-ack', id: m.id, bytes: total });
        }
        break;
      case 'inject':
        inject(m.html);
        break;
      case 'select':
        selectionProbe();
        break;
      case 'ping':
        send({ type: 'pong', id: m.id });
        break;
      case 'probes':
        // Asked at the end of the run: the img error event (and any handler
        // it would have fired) is async, so probe flags read right after
        // injection could be false merely because nothing has happened yet.
        send({
          type: 'probes',
          scriptExecuted: window.__probe_script_executed === true,
          handlerExecuted: window.__probe_handler_executed === true,
          violations: violations.slice(),
        });
        break;
    }
  });

  // Report the environment as found. __TAURI_INTERNALS__ leaking into this
  // frame would mean untrusted page content can reach the IPC layer — the
  // single most important boolean this spike produces.
  send({
    type: 'ready',
    origin: String(window.origin),
    inlineScriptRan: window.__probe_inline_srcdoc === true,
    tauriIpcReachable: typeof window.__TAURI_INTERNALS__ !== 'undefined',
    violationsAtLoad: violations.slice(),
  });
})();
