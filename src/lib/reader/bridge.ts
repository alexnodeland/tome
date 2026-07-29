/**
 * The app half of the reader bridge (S1-13).
 *
 * ```text
 * Rust ──invoke──▶ app webview ──postMessage──▶ sandboxed iframe (reader)
 * ```
 *
 * This module owns the second leg. Everything it does follows from
 * `docs/spikes/002-reader-iframe-bridge.md`, which measured the first
 * implementation of it:
 *
 * - **One message per page.** A 500 KB page round-trips in ~0.1 ms; splitting
 *   it into 8×64 KB chunks measured *slower*. There is no streaming protocol
 *   and there should never be one.
 * - **The frame's bootstrap is an external script.** The srcdoc document
 *   inherits the app CSP, which has no `unsafe-inline` for scripts.
 * - **The frame's own CSP names the app origin explicitly.** `'self'` inside
 *   an opaque-origin document matches nothing.
 * - **No `allow-same-origin`.** That is what keeps `__TAURI_INTERNALS__` out
 *   of reach of page content.
 */

/** What the frame sends back. */
export type FrameMessage =
  | { type: 'ready'; tauriReachable: boolean }
  | { type: 'loaded'; token: number; height: number }
  | { type: 'navigate'; href: string; modified: boolean }
  | { type: 'scrolledTo'; id: string; found: boolean }
  | {
      type: 'findResults';
      total: number;
      /** 1-based, or 0 when there is no current match. */
      index: number;
      /** Whether the frame's engine can paint highlights at all. */
      supported: boolean;
    }
  | {
      type: 'scroll';
      top: number;
      height: number;
      viewport: number;
      activeId: string | null;
      activeChanged: boolean;
    };

export interface ReaderFrameHandlers {
  onReady?: () => void;
  onNavigate?: (href: string, modified: boolean) => void;
  onScroll?: (state: {
    top: number;
    height: number;
    viewport: number;
    activeId: string | null;
  }) => void;
  onLoaded?: (height: number) => void;
  onFindResults?: (state: { total: number; index: number; supported: boolean }) => void;
}

/**
 * The document the frame starts as.
 *
 * `appOrigin` is interpolated rather than written as `'self'` because inside
 * a sandboxed frame the origin is opaque and `'self'` matches nothing — the
 * stylesheets and the bootstrap script would both be blocked. It comes from
 * `location.origin`, so it is `tauri://localhost` in a built app and
 * `http://localhost:1420` under `vite dev` with no branch here.
 *
 * The `<meta>` policy **stacks on top of** the inherited app CSP rather than
 * replacing it; both are enforced, and a violation is reported twice. It is
 * here so the frame's own rules are stated at the frame rather than being an
 * accident of what the app happens to allow.
 */
export function frameDocument(appOrigin: string): string {
  const csp = [
    "default-src 'none'",
    `script-src ${appOrigin}`,
    `style-src ${appOrigin}`,
    // Localized assets only. No http, no https, and no `data:` — a `data:`
    // image is a known SVG-script vector, and S1-10 rewrites every real image
    // to a `tome:` URL, so nothing legitimate needs it.
    'img-src tome:',
    "font-src 'none'",
    // The frame talks to the app through postMessage and to nothing else.
    "connect-src 'none'",
    "object-src 'none'",
    "frame-src 'none'",
    "base-uri 'none'",
    "form-action 'none'",
  ].join('; ');

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<link rel="stylesheet" href="${appOrigin}/tokens.css">
<link rel="stylesheet" href="${appOrigin}/reader.css">
</head>
<body>
<div id="tome-content"></div>
<script src="${appOrigin}/reader-frame.js"></script>
</body>
</html>`;
}

/**
 * Wraps one `<iframe>` and speaks the reader protocol over it.
 *
 * Construct it with the element, call {@link ReaderFrame.attach} once the
 * element is in the document, and {@link ReaderFrame.destroy} when it leaves.
 */
export class ReaderFrame {
  private readonly frame: HTMLIFrameElement;
  private readonly handlers: ReaderFrameHandlers;
  private readonly origin: string;
  private ready = false;
  /** Sent before the frame reported ready; replayed on ready. */
  private queued: unknown[] = [];
  private token = 0;
  private listener?: (event: MessageEvent) => void;

  constructor(frame: HTMLIFrameElement, handlers: ReaderFrameHandlers, origin = location.origin) {
    this.frame = frame;
    this.handlers = handlers;
    this.origin = origin;
  }

  attach(): void {
    this.listener = (event: MessageEvent) => this.receive(event);
    window.addEventListener('message', this.listener);
    // `sandbox` is set here rather than in the template so that the one place
    // it can be weakened is in code, next to the reason it must not be.
    // allow-scripts and NOTHING else: no allow-same-origin (the frame would
    // gain the app's origin and its IPC), no allow-popups, no
    // allow-top-navigation, no allow-modals.
    this.frame.setAttribute('sandbox', 'allow-scripts');
    this.frame.setAttribute('title', 'Documentation');
    this.frame.srcdoc = frameDocument(this.origin);
  }

  destroy(): void {
    if (this.listener) window.removeEventListener('message', this.listener);
    this.listener = undefined;
    this.ready = false;
    this.queued = [];
  }

  /** Replace the displayed page. One message, per SPIKE-002. */
  showPage(html: string, options: { fragment?: string; scrollTop?: number } = {}): number {
    this.token += 1;
    this.send({
      type: 'page',
      html,
      fragment: options.fragment ?? null,
      scrollTop: options.scrollTop ?? 0,
      token: this.token,
    });
    return this.token;
  }

  /**
   * Find text in the displayed page (S2-8, P2-007).
   *
   * The search runs **inside the frame**, and it has to: the frame is
   * `sandbox="allow-scripts"` with no `allow-same-origin`, so its origin is
   * opaque and the app cannot read its document at all. `window.find()` in
   * the app would search the app's chrome and find nothing.
   *
   * The result comes back asynchronously through `onFindResults` rather than
   * as a return value, because every leg of this bridge is `postMessage`.
   */
  find(query: string): void {
    this.send({ type: 'find', query });
  }

  /** Step to the next (`1`) or previous (`-1`) match. Wraps at both ends. */
  findStep(direction: 1 | -1): void {
    this.send({ type: 'findStep', direction });
  }

  findClear(): void {
    this.send({ type: 'findClear' });
  }

  scrollTo(id: string, behavior: ScrollBehavior = 'smooth'): void {
    this.send({ type: 'scrollTo', id, behavior });
  }

  scrollToTop(top = 0): void {
    this.send({ type: 'scrollTop', top });
  }

  /**
   * Set attributes on the frame's `<html>`. Theme, text size, and line
   * numbers are all CSS-driven, so changing them is one attribute and no
   * re-render — which is what makes a theme switch free even on a page with
   * two hundred highlighted code blocks.
   */
  settings(attributes: Record<string, string | null>): void {
    this.send({ type: 'settings', attributes });
  }

  private send(message: unknown): void {
    if (!this.ready) {
      this.queued.push(message);
      return;
    }
    // targetOrigin '*': the frame's origin is opaque and there is no origin
    // string that would target it. The protection is `receive`'s source
    // check in the other direction, plus the fact that only our own content
    // is ever inside the frame.
    this.frame.contentWindow?.postMessage(message, '*');
  }

  private receive(event: MessageEvent): void {
    // The check that makes '*' above acceptable: a message is only ours if it
    // came from this exact frame's window. `event.origin` is "null" for an
    // opaque-origin sender and cannot distinguish one frame from another.
    if (event.source !== this.frame.contentWindow) return;

    const message = event.data as FrameMessage;
    if (!message || typeof message !== 'object') return;

    switch (message.type) {
      case 'ready':
        this.ready = true;
        if (message.tauriReachable) {
          // Not a thrown error — that would take the app down over a
          // condition the user cannot act on — but it must be loud. If this
          // ever fires, page content can reach the IPC layer.
          console.error(
            'Reader frame can reach __TAURI_INTERNALS__. The sandbox attribute is wrong.',
          );
        }
        for (const queued of this.queued.splice(0)) {
          this.frame.contentWindow?.postMessage(queued, '*');
        }
        this.handlers.onReady?.();
        break;
      case 'navigate':
        this.handlers.onNavigate?.(message.href, message.modified);
        break;
      case 'scroll':
        this.handlers.onScroll?.({
          top: message.top,
          height: message.height,
          viewport: message.viewport,
          activeId: message.activeId,
        });
        break;
      case 'loaded':
        // Ignore an ack for a page that has already been superseded, so a
        // fast sequence of navigations does not restore the wrong scroll
        // position from an earlier one.
        if (message.token === this.token) this.handlers.onLoaded?.(message.height);
        break;
      case 'findResults':
        this.handlers.onFindResults?.({
          total: message.total,
          index: message.index,
          supported: message.supported,
        });
        break;
      case 'scrolledTo':
        break;
    }
  }
}
