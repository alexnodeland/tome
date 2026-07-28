/**
 * SPIKE-002 runner: measures the reader bridge from the app-webview side.
 *
 * Activated only when the app is launched with `TOME_SPIKE_002=1` (see
 * `src/main.ts` and `src-tauri/src/spike002.rs`). Everything it learns goes
 * to stdout through the `spike002_report` command so a scripted run can
 * capture it; the write-up lives in `docs/spikes/`. Remove with the rest of
 * the harness when S1-13 lands.
 *
 * The bridge has two legs, measured separately:
 *   1. Rust ⇄ app webview — Tauri `invoke` (pull) and events (push)
 *   2. app webview ⇄ sandboxed iframe — `postMessage`
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/** Everything the frame can post back. One bag type; fields by message. */
interface FrameMsg {
  type: string;
  id?: number;
  bytes?: number;
  origin?: string;
  inlineScriptRan?: boolean;
  tauriIpcReachable?: boolean;
  violationsAtLoad?: string[];
  parseMs?: number;
  elements?: number;
  scriptExecuted?: boolean;
  handlerExecuted?: boolean;
  violations?: string[];
  ok?: boolean;
  why?: string;
  quoteLength?: number;
  quote?: string;
  prefix?: string;
  suffix?: string;
  crossesElementBoundary?: boolean;
}

const report = (line: string) => invoke('spike002_report', { line: `SPIKE002 | ${line}` });

/** Empty input yields Infinity, which fails any "< N ms" criterion loudly. */
function percentile(samples: number[], q: number): number {
  const s = [...samples].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor(q * s.length))] ?? Infinity;
}

function stats(samples: number[]): string {
  const mean = samples.reduce((a, b) => a + b, 0) / samples.length;
  return (
    `n=${samples.length} mean=${mean.toFixed(2)}ms p50=${percentile(samples, 0.5).toFixed(2)}ms ` +
    `p95=${percentile(samples, 0.95).toFixed(2)}ms max=${percentile(samples, 1).toFixed(2)}ms`
  );
}

/** Resolvers keyed by `type` or `type:id`; fed by the window message listener. */
const pending = new Map<string, (m: FrameMsg) => void>();

function waitFor(type: string, id?: number, timeoutMs = 15000): Promise<FrameMsg> {
  const key = id === undefined ? type : `${type}:${id}`;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(key);
      reject(new Error(`timed out waiting for '${key}'`));
    }, timeoutMs);
    pending.set(key, (m) => {
      clearTimeout(timer);
      resolve(m);
    });
  });
}

function makeFrame(target: HTMLElement): HTMLIFrameElement {
  const frame = document.createElement('iframe');
  // The production reader posture: scripts yes, same-origin NO — the frame
  // gets an opaque origin and can only talk to the app via postMessage.
  frame.setAttribute('sandbox', 'allow-scripts');
  frame.style.cssText = 'width: 800px; height: 600px; border: 1px solid #888';
  // A second, stricter CSP layered inside the frame, on top of the inherited
  // app CSP (both are enforced). 'self' is useless here — the origin is
  // opaque — so the app origin is spelled out for the bootstrap script.
  const csp =
    `default-src 'none'; script-src ${location.origin}; ` +
    `img-src data:; style-src 'unsafe-inline'`;
  frame.srcdoc =
    `<!doctype html><html><head>` +
    `<meta http-equiv="Content-Security-Policy" content="${csp}"></head><body>` +
    // Inline probe: expected to be BLOCKED by the inherited CSP. If it runs,
    // CSP inheritance into srcdoc is not happening and the isolation story
    // needs rethinking.
    `<script>window.__probe_inline_srcdoc = true;</script>` +
    `<script src="/spike002-frame.js"></script>` +
    `<div id="content"></div></body></html>`;
  target.appendChild(frame);
  return frame;
}

export async function runSpike002(target: HTMLElement): Promise<void> {
  target.innerHTML = '<h1>SPIKE-002 running…</h1><p>Results go to stdout.</p>';
  let failed = false;
  const fail = async (line: string) => {
    failed = true;
    await report(`FAIL ${line}`);
  };

  window.addEventListener('message', (e: MessageEvent) => {
    const m = e.data as FrameMsg;
    if (!m || typeof m !== 'object' || typeof m.type !== 'string') return;
    const key = m.id === undefined ? m.type : `${m.type}:${m.id}`;
    const resolver = pending.get(key);
    if (resolver) {
      pending.delete(key);
      resolver(m);
    }
  });

  try {
    await report(`app origin = ${location.origin}`);

    // ---- Leg 1: Rust ⇄ app webview -------------------------------------

    // invoke round-trip, minimal payload
    await invoke('spike002_echo', { payload: 'warmup' });
    const rtts: number[] = [];
    for (let i = 0; i < 200; i++) {
      const t0 = performance.now();
      await invoke('spike002_echo', { payload: 'x' });
      rtts.push(performance.now() - t0);
    }
    await report(`invoke round-trip (tiny): ${stats(rtts)}`);

    // invoke pulling the 500 KB page from Rust
    const pulls: number[] = [];
    let pageHtml = '';
    for (let i = 0; i < 10; i++) {
      const t0 = performance.now();
      pageHtml = await invoke<string>('spike002_page_html');
      pulls.push(performance.now() - t0);
    }
    await report(`page size = ${pageHtml.length} bytes`);
    await report(
      `invoke pull 500KB (Rust→JS): cold=${(pulls[0] ?? Infinity).toFixed(2)}ms ${stats(pulls)}`,
    );

    // Rust→JS event push: 180 events (3 s of 60 Hz traffic) emitted at speed.
    // listen() is permission-gated: without core:event in a capability file it
    // rejects, and the first run of this spike proved that failure is silent
    // unless awaited. See src-tauri/capabilities/default.json.
    const EVENTS = 180;
    let eventCount = 0;
    let firstEvent = 0;
    let lastEvent = 0;
    let resolveEvents: () => void = () => undefined;
    const eventsDone = new Promise<void>((resolve) => {
      resolveEvents = resolve;
    });
    try {
      await listen('spike002-tick', () => {
        if (eventCount === 0) firstEvent = performance.now();
        eventCount += 1;
        lastEvent = performance.now();
        if (eventCount === EVENTS) resolveEvents();
      });
      await invoke('spike002_emit', { n: EVENTS });
      await Promise.race([eventsDone, new Promise((r) => setTimeout(r, 10000))]);
      if (eventCount === EVENTS) {
        const span = lastEvent - firstEvent;
        await report(
          `event push (Rust→JS): ${EVENTS} events in ${span.toFixed(2)}ms ` +
            `(${((EVENTS / span) * 1000).toFixed(0)}/s)`,
        );
      } else {
        await fail(`event push: only ${eventCount}/${EVENTS} events arrived`);
      }
    } catch (e) {
      await fail(`event listen/emit: ${e instanceof Error ? e.message : String(e)}`);
    }

    // ---- Leg 2: app webview ⇄ sandboxed iframe --------------------------

    const readyWait = waitFor('ready');
    const frame = makeFrame(target);
    const ready = await readyWait;
    const frameWindow = frame.contentWindow;
    if (!frameWindow) throw new Error('iframe has no contentWindow');
    const toFrame = (msg: object) => frameWindow.postMessage(msg, '*');

    await report(`frame origin = ${ready.origin} (opaque expected: "null")`);
    if (ready.tauriIpcReachable) {
      await fail('frame can reach __TAURI_INTERNALS__ — sandbox does NOT isolate the IPC layer');
    } else {
      await report('frame cannot reach __TAURI_INTERNALS__ (IPC isolated from content)');
    }
    if (ready.inlineScriptRan) {
      await fail('inline <script> in srcdoc EXECUTED — parent CSP is not inherited');
    } else {
      await report('inline <script> in srcdoc blocked (parent CSP inherited into srcdoc)');
    }
    await report(`frame CSP violations at load = ${JSON.stringify(ready.violationsAtLoad)}`);

    // postMessage 500 KB, single message, round trip
    let msgId = 0;
    const single: number[] = [];
    for (let i = 0; i < 10; i++) {
      const id = msgId++;
      const t0 = performance.now();
      const ackWait = waitFor('echo-ack', id);
      toFrame({ type: 'echo', id, payload: pageHtml });
      await ackWait;
      single.push(performance.now() - t0);
    }
    await report(`postMessage 500KB single, round-trip: ${stats(single)}`);

    // postMessage 500 KB as 8 × 64 KB chunks, round trip for the full set
    const CHUNK = 64 * 1024;
    const chunked: number[] = [];
    for (let i = 0; i < 10; i++) {
      const id = msgId++;
      const t0 = performance.now();
      const ackWait = waitFor('chunks-ack', id);
      for (let off = 0; off < pageHtml.length; off += CHUNK) {
        toFrame({
          type: 'chunk',
          id,
          payload: pageHtml.slice(off, off + CHUNK),
          last: off + CHUNK >= pageHtml.length,
        });
      }
      await ackWait;
      chunked.push(performance.now() - t0);
    }
    await report(`postMessage 500KB as 8×64KB chunks, round-trip: ${stats(chunked)}`);

    // Inject the page for real: parse time inside the frame; paint timing is
    // reported only if the compositor is actually ticking (rAF is suspended
    // for occluded windows, so a headless run must not depend on it).
    const injectedWait = waitFor('injected', undefined, 30000);
    toFrame({ type: 'inject', html: pageHtml });
    const injected = await injectedWait;
    await report(
      `inject 500KB: innerHTML=${injected.parseMs?.toFixed(2)}ms ` +
        `elements=${injected.elements}`,
    );
    await report(`frame CSP violations after inject = ${JSON.stringify(injected.violations)}`);
    await waitFor('painted', undefined, 3000).then(
      (p: FrameMsg & { sinceInjectMs?: number }) =>
        report(`first paint after inject = ${p.sinceInjectMs?.toFixed(2)}ms`),
      () => report('no rAF tick in frame within 3s (window occluded — paint timing unavailable)'),
    );

    // Selection / anchoring probe
    const selectionWait = waitFor('selection');
    toFrame({ type: 'select' });
    const sel = await selectionWait;
    if (sel.ok) {
      await report(
        `selection: ok, quote ${sel.quoteLength} chars, ` +
          `crosses element boundary = ${sel.crossesElementBoundary}`,
      );
      await report(`selection quote = ${JSON.stringify(sel.quote)}`);
      await report(`selection prefix = ${JSON.stringify(sel.prefix)}`);
      await report(`selection suffix = ${JSON.stringify(sel.suffix)}`);
    } else {
      await fail(`selection probe failed: ${sel.why ?? 'no detail'}`);
    }

    // Sustained 60 Hz ping over the bridge for 3 s. Driven by a timer, not
    // rAF — rAF is suspended when the window is occluded, and a scripted run
    // must produce numbers regardless. rAF is *observed* concurrently so the
    // pacing evidence is still there when the window is actually visible.
    const pingRtts: number[] = [];
    const frameGaps: number[] = [];
    let sent = 0;
    let rafStopped = false;
    {
      let lastTs: number | null = null;
      const rafProbe = (ts: number) => {
        if (lastTs !== null) frameGaps.push(ts - lastTs);
        lastTs = ts;
        if (!rafStopped) requestAnimationFrame(rafProbe);
      };
      requestAnimationFrame(rafProbe);
    }
    const t0ping = performance.now();
    while (performance.now() - t0ping < 3000) {
      const id = msgId++;
      sent += 1;
      const sentAt = performance.now();
      void waitFor('pong', id, 5000).then(
        () => pingRtts.push(performance.now() - sentAt),
        () => undefined, // late pong after the run ends: not a failure
      );
      toFrame({ type: 'ping', id });
      await new Promise((r) => setTimeout(r, 16));
    }
    rafStopped = true;
    // Let stragglers land before reading the numbers.
    await new Promise((r) => setTimeout(r, 250));
    await report(
      `60Hz sustained 3s: sent=${sent} answered=${pingRtts.length} rtt: ${stats(pingRtts)}`,
    );
    if (frameGaps.length > 0) {
      const dropped = frameGaps.filter((g) => g > 25).length;
      await report(
        `frame pacing during 60Hz test: ${frameGaps.length} rAF ticks, ` +
          `${dropped} gaps >25ms, worst=${Math.max(...frameGaps).toFixed(2)}ms`,
      );
    } else {
      await report('frame pacing: rAF never ticked (window occluded); pacing unmeasured this run');
    }

    // Final probe readback: the hostile-content verdicts. Read at the end
    // because the img error path (and the handler it would fire) is async.
    const probesWait = waitFor('probes');
    toFrame({ type: 'probes' });
    const probes = await probesWait;
    if (probes.scriptExecuted) {
      await fail('<script> in injected content EXECUTED');
    } else {
      await report('<script> in injected content did not execute');
    }
    if (probes.handlerExecuted) {
      await fail('inline onerror handler in injected content EXECUTED — CSP did not block it');
    } else {
      await report('inline onerror handler in injected content did not execute');
    }
    await report(`frame CSP violations at end = ${JSON.stringify(probes.violations)}`);

    // ---- Verdicts against the SPIKE-002 success criteria ----------------
    const invokeP95 = percentile(rtts, 0.95);
    const pingP95 = percentile(pingRtts, 0.95);
    if (invokeP95 >= 5)
      await fail(`criterion: invoke round-trip p95 ${invokeP95.toFixed(2)}ms >= 5ms`);
    if (pingP95 >= 5) await fail(`criterion: bridge ping p95 ${pingP95.toFixed(2)}ms >= 5ms`);
    if (pingRtts.length < sent * 0.98) await fail('criterion: >2% of 60Hz messages unanswered');

    await report(failed ? 'RESULT: FAIL (see FAIL lines above)' : 'RESULT: PASS');
  } catch (e) {
    await fail(`unhandled: ${e instanceof Error ? e.message : String(e)}`).catch(() => undefined);
  }
  await invoke('spike002_done', { failed }).catch(() => undefined);
}
