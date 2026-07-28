/**
 * The reader bridge's security posture, asserted (S1-13).
 *
 * These are not "does it work" tests — jsdom does not execute the frame's
 * bootstrap, so end-to-end behaviour belongs to an interactive run. They
 * assert the properties that would fail *silently* and that SPIKE-002 paid
 * to discover: the sandbox attribute, the frame's own CSP, and the source
 * check on incoming messages. Each of these can be weakened by a one-word
 * edit with no visible consequence until it matters.
 */
import { describe, it, expect, vi } from 'vitest';
import { frameDocument, ReaderFrame } from './bridge';

const ORIGIN = 'tauri://localhost';

describe('frameDocument', () => {
  const document_ = frameDocument(ORIGIN);

  it('names the app origin instead of using self', () => {
    // `'self'` inside a sandboxed frame matches nothing — the origin is
    // opaque — so a policy written with it blocks the stylesheets and the
    // bootstrap script it was meant to allow. SPIKE-002 finding 3.
    const csp = cspOf(document_);
    expect(csp).not.toContain("'self'");
    expect(csp).toContain(`script-src ${ORIGIN}`);
    expect(csp).toContain(`style-src ${ORIGIN}`);
  });

  it('permits images only over the local asset protocol', () => {
    // Not http, not https, and not data: — a data: image is a known
    // SVG-script vector, and S1-10 rewrites every real image to a tome: URL,
    // so nothing legitimate needs it. This is the CSP half of the offline
    // guarantee.
    const csp = cspOf(document_);
    expect(csp).toContain('img-src tome:');
    expect(csp).not.toMatch(/img-src[^;]*https?:/);
    expect(csp).not.toMatch(/img-src[^;]*data:/);
  });

  it('lets the frame reach nothing by default', () => {
    const csp = cspOf(document_);
    expect(csp).toContain("default-src 'none'");
    expect(csp).toContain("connect-src 'none'");
    expect(csp).toContain("object-src 'none'");
    expect(csp).toContain("frame-src 'none'");
    expect(csp).toContain("base-uri 'none'");
    expect(csp).toContain("form-action 'none'");
  });

  it('loads its bootstrap as an external script, never inline', () => {
    // The app CSP has no 'unsafe-inline' for scripts, deliberately: the same
    // policy is what stops inline event handlers in injected page content.
    // An inline bootstrap would be blocked and the frame would sit dead.
    expect(document_).toContain(`<script src="${ORIGIN}/reader-frame.js"></script>`);
    expect(document_).not.toMatch(/<script(?![^>]*\ssrc=)/);
  });

  it('links the shared token stylesheet, so the frame and the app agree', () => {
    expect(document_).toContain(`href="${ORIGIN}/tokens.css"`);
    expect(document_).toContain(`href="${ORIGIN}/reader.css"`);
  });

  function cspOf(html: string): string {
    const match = /content="([^"]*)"/.exec(html);
    expect(match, 'the frame document must carry a meta CSP').not.toBeNull();
    return match?.[1] ?? '';
  }
});

describe('ReaderFrame', () => {
  function harness() {
    const frame = document.createElement('iframe');
    document.body.append(frame);
    const posted: unknown[] = [];
    // jsdom gives the iframe a contentWindow but will not run its scripts, so
    // the frame's side is faked. What is under test is the app's side.
    // `as unknown as Window`: a two-method stand-in is not structurally a
    // Window and TypeScript is right to say so. What is under test is the
    // app's side of the bridge, which only ever calls postMessage on it.
    const contentWindow = {
      postMessage: (m: unknown) => {
        posted.push(m);
      },
    } as unknown as Window;
    Object.defineProperty(frame, 'contentWindow', { value: contentWindow });
    return { frame, posted, contentWindow };
  }

  function ready(frame: HTMLIFrameElement, contentWindow: Window) {
    window.dispatchEvent(
      new MessageEvent('message', {
        data: { type: 'ready', tauriReachable: false },
        source: contentWindow,
      }),
    );
    return frame;
  }

  it('sandboxes the frame with allow-scripts and nothing else', () => {
    // allow-same-origin would give the frame the app's origin and with it
    // __TAURI_INTERNALS__ — page content could then invoke Rust commands.
    // SPIKE-002 finding 7 is that this attribute is the whole isolation.
    const { frame } = harness();
    new ReaderFrame(frame, {}, ORIGIN).attach();
    expect(frame.getAttribute('sandbox')).toBe('allow-scripts');
  });

  it('ignores messages that did not come from its own frame', () => {
    // event.origin is "null" for every opaque-origin sender and cannot tell
    // one frame from another, so the source check is the only thing standing
    // between the app and a message from anywhere else in the page.
    const { frame, contentWindow } = harness();
    const onNavigate = vi.fn();
    const bridge = new ReaderFrame(frame, { onNavigate }, ORIGIN);
    bridge.attach();
    ready(frame, contentWindow);

    window.dispatchEvent(
      new MessageEvent('message', {
        data: { type: 'navigate', href: 'evil.html', modified: false },
        source: window,
      }),
    );
    expect(onNavigate).not.toHaveBeenCalled();

    window.dispatchEvent(
      new MessageEvent('message', {
        data: { type: 'navigate', href: 'real.html', modified: false },
        source: contentWindow,
      }),
    );
    expect(onNavigate).toHaveBeenCalledWith('real.html', false);
    bridge.destroy();
  });

  it('holds messages until the frame reports ready, then replays them', () => {
    // The bootstrap is an external script, so there is a real window between
    // srcdoc being set and the frame being able to receive anything. Without
    // the queue the first page silently vanishes.
    const { frame, posted, contentWindow } = harness();
    const bridge = new ReaderFrame(frame, {}, ORIGIN);
    bridge.attach();

    bridge.showPage('<p>hello</p>');
    expect(posted).toHaveLength(0);

    ready(frame, contentWindow);
    expect(posted).toHaveLength(1);
    expect(posted[0]).toMatchObject({ type: 'page', html: '<p>hello</p>' });
    bridge.destroy();
  });

  it('sends a whole page in one message', () => {
    // SPIKE-002 finding 1: 500 KB round-trips in ~0.1 ms and chunking it
    // measured slower. If this ever becomes more than one message, someone
    // has reintroduced a streaming protocol the numbers rule out.
    const { frame, posted, contentWindow } = harness();
    const bridge = new ReaderFrame(frame, {}, ORIGIN);
    bridge.attach();
    ready(frame, contentWindow);

    bridge.showPage('x'.repeat(500_000));
    expect(posted).toHaveLength(1);
    bridge.destroy();
  });

  it('drops a load ack for a page that has been superseded', () => {
    // Two fast navigations: the first page's ack must not restore its scroll
    // position over the second page.
    const { frame, contentWindow } = harness();
    const onLoaded = vi.fn();
    const bridge = new ReaderFrame(frame, { onLoaded }, ORIGIN);
    bridge.attach();
    ready(frame, contentWindow);

    const first = bridge.showPage('<p>a</p>');
    const second = bridge.showPage('<p>b</p>');

    const ack = (token: number) =>
      window.dispatchEvent(
        new MessageEvent('message', {
          data: { type: 'loaded', token, height: 100 },
          source: contentWindow,
        }),
      );

    ack(first);
    expect(onLoaded).not.toHaveBeenCalled();
    ack(second);
    expect(onLoaded).toHaveBeenCalledOnce();
    bridge.destroy();
  });

  it('shouts if the frame can reach the IPC layer', () => {
    const { frame, contentWindow } = harness();
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    const bridge = new ReaderFrame(frame, {}, ORIGIN);
    bridge.attach();

    window.dispatchEvent(
      new MessageEvent('message', {
        data: { type: 'ready', tauriReachable: true },
        source: contentWindow,
      }),
    );
    expect(error).toHaveBeenCalledWith(expect.stringContaining('__TAURI_INTERNALS__'));
    error.mockRestore();
    bridge.destroy();
  });

  it('stops listening once destroyed', () => {
    const { frame, contentWindow } = harness();
    const onNavigate = vi.fn();
    const bridge = new ReaderFrame(frame, { onNavigate }, ORIGIN);
    bridge.attach();
    ready(frame, contentWindow);
    bridge.destroy();

    window.dispatchEvent(
      new MessageEvent('message', {
        data: { type: 'navigate', href: 'x.html', modified: false },
        source: contentWindow,
      }),
    );
    expect(onNavigate).not.toHaveBeenCalled();
  });
});
