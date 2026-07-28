# SPIKE-002 — the reader iframe bridge, measured

**Date:** 2026-07-28 · **Status:** complete · **Verdict:** the design works; numbers below.
**Spec:** [`docs/plans/07-technical-spikes.md`](../plans/07-technical-spikes.md) § SPIKE-002. The
spec predates the plan review and frames the question as "WKWebView bridge"; the settled
architecture (Tauri is the shell, the reader is a sandboxed `<iframe>` in the primary webview —
see `docs/PRD.md` § Technical Architecture) reframes it as two legs:

```
Rust ──invoke / event──▶ app webview ──postMessage──▶ sandboxed iframe (reader)
```

## Method

A spike mode built into the app itself, gated on `TOME_SPIKE_002=1`:
`src-tauri/src/spike002.rs` (commands), `src/spike/spike002.ts` (runner),
`public/spike002-frame.js` (the script inside the frame). Results go to stdout so the run is
capturable headlessly.

**The harness was removed when S1-13 landed (2026-07-28), as planned. This document is the part
that stays**, and its findings live on in the real reader: `src/lib/reader/bridge.ts` (the app
half), `public/reader-frame.js` (the frame half), and `src/lib/reader/bridge.test.ts`, which
asserts the posture below — sandbox attribute, frame CSP, message source check — so that a
one-word edit cannot quietly undo what this spike cost to learn.

Run against the **debug** build — the numbers below are a ceiling, not a floor — on the
environment recorded in `.claude/continuation.md` (macOS 26.5, arm64). The frame is
`sandbox="allow-scripts"` with **no** `allow-same-origin` (opaque origin), `srcdoc` carrying its
own `<meta>` CSP on top of the app CSP from `tauri.conf.json`. The injected page is a
deterministic 513 200-byte document (6 692 elements) with three hostile probes embedded: a
`<script>`, an inline `onerror` handler, and an `<img>` pointing at a network address.

## Raw output

Third run, byte-identical in every verdict to the second (first run failed; see finding 6):

```
SPIKE002 | app origin = tauri://localhost
SPIKE002 | invoke round-trip (tiny): n=200 mean=0.29ms p50=0.00ms p95=1.00ms max=5.00ms
SPIKE002 | page size = 513200 bytes
SPIKE002 | invoke pull 500KB (Rust→JS): cold=14.00ms n=10 mean=14.50ms p50=14.00ms p95=20.00ms max=20.00ms
SPIKE002 | event push (Rust→JS): 180 events in 20.00ms (9000/s)
SPIKE002 | frame origin = null (opaque expected: "null")
SPIKE002 | frame cannot reach __TAURI_INTERNALS__ (IPC isolated from content)
SPIKE002 | inline <script> in srcdoc blocked (parent CSP inherited into srcdoc)
SPIKE002 | frame CSP violations at load = []
SPIKE002 | postMessage 500KB single, round-trip: n=10 mean=0.10ms p50=0.00ms p95=1.00ms max=1.00ms
SPIKE002 | postMessage 500KB as 8×64KB chunks, round-trip: n=10 mean=0.30ms p50=0.00ms p95=1.00ms max=1.00ms
SPIKE002 | inject 500KB: innerHTML=4.00ms elements=6692
SPIKE002 | frame CSP violations after inject = []
SPIKE002 | no rAF tick in frame within 3s (window occluded — paint timing unavailable)
SPIKE002 | selection: ok, quote 75 chars, crosses element boundary = true
SPIKE002 | selection quote = "1 of section 1. The quick brown fox jumps over the lazy dog "
SPIKE002 | selection prefix = " by character offset.\nParagraph "
SPIKE002 | selection suffix = " type resolves ~/Library/Applica"
SPIKE002 | 60Hz sustained 3s: sent=13 answered=13 rtt: n=13 mean=0.23ms p50=0.00ms p95=2.00ms max=2.00ms
SPIKE002 | frame pacing: rAF never ticked (window occluded); pacing unmeasured this run
SPIKE002 | <script> in injected content did not execute
SPIKE002 | inline onerror handler in injected content did not execute
SPIKE002 | frame CSP violations at end = ["img-src:http://127.0.0.1:9/pixel.png","img-src:http://127.0.0.1:9/pixel.png","script-src-attr:inline","script-src-attr:inline"]
SPIKE002 | RESULT: PASS
```

## Findings

**1. One postMessage carries the whole page; do not build a streaming protocol.** A 500 KB string
round-trips parent↔frame in ~0.1 ms; chunking it 8×64 KB makes it *slower* (0.3 ms). Same-process
`postMessage` string transfer is effectively free in WebKit. `innerHTML` of the full page costs
4 ms for 6 692 elements. S1-13's protocol is: one `{type: "page", html}` message.

**2. The Rust→JS leg is the only leg that costs anything, and it costs little.** Pulling the
500 KB page over `invoke` is ~14 ms in a debug build (IPC serialisation, not the webview, is the
cost). Event push moved 180 events in 20 ms (9 000/s) — two orders of magnitude past the 60/s
requirement. Round-trip p95 for a small `invoke` is 1 ms against the < 5 ms criterion.

**3. CSP: the srcdoc frame inherits the app CSP, and a second `<meta>` policy stacks on top.**
Both were enforced simultaneously — each violation in the log appears **twice, once per policy**.
Consequences for S1-13:

- The frame's bootstrap script **cannot be inline** (the app CSP has no `unsafe-inline` for
  scripts, and that is load-bearing — see next finding). It must be an external file served from
  the app origin.
- `'self'` is meaningless inside the frame — the origin is opaque (`null`) — so the frame's own
  CSP must name the app origin explicitly (`tauri://localhost` in production; build it from
  `location.origin`).

**4. All three hostile probes died, each by a different mechanism — all three are needed.** The
`<script>` never executed: the HTML5 parser refuses to run scripts inserted via `innerHTML`. The
inline `onerror` handler never executed: `innerHTML` does **not** neutralise event-handler
attributes; only CSP (`script-src-attr:inline` violations in the log) stops those. The network
`<img>` never fetched: `img-src data:` blocked it at the policy layer, before any socket. A
sanitizer (S1-9) is still required — CSP is the second wall, not a substitute — but the walls hold.

**5. Selection works in the sandbox; annotation anchoring is viable.** `window.getSelection()`
inside the opaque-origin frame accepts a programmatic `Range` spanning an element boundary
(text → `<code>`), and quote + prefix/suffix (the anchor format — never character offsets) fall
out of two auxiliary Ranges. The prefix in the log even captures a newline across a `<p>` break.

**6. Tauri's event API is deny-by-default, and the failure is silent.** The scaffold had no
`src-tauri/capabilities/` directory, so `listen()` was rejected and run 1 lost all 180 events
without an error surfacing — the runner had fired `listen()` without awaiting it.
`src-tauri/capabilities/default.json` now grants `core:event:default` to the main window and
nothing else. Anything that adds a Tauri plugin later must extend that file deliberately.

**7. The sandbox seals the IPC layer.** `window.__TAURI_INTERNALS__` is not reachable from the
frame, so injected page content cannot invoke Rust commands. This was the single most important
boolean in the run.

**8. Occluded windows lose rAF entirely and have timers clamped to ~230 ms.** Both headless runs
had the window occluded: `requestAnimationFrame` never fired (in frame or parent) and the 60 Hz
timer loop managed 13 iterations in 3 s. Two consequences: the reader must never gate correctness
on rAF (paint timing is telemetry — of the local, logged kind — not a dependency), and frame-drop
behaviour under bridge traffic was **not measured here**.

## Against the spec's success criteria

| Criterion | Result |
|---|---|
| Single message round-trip < 5 ms | **Pass** — invoke p95 1 ms; bridge ping p95 2 ms |
| 60 msg/s sustained without frame drops | **Throughput proven** (9 000 events/s; 0.2 ms/message ≈ 1 % of a 16.7 ms frame budget). Frame pacing itself unmeasured — occluded windows suspend rAF, and a focus-stealing workaround was not worth it. Verify interactively in S1-13 acceptance. |
| Memory stable under load | Not measured — WKWebView exposes no `performance.memory`; deferred to the P2-018-style benchmarks with the real reader. |

## What S1-13 inherits

- Protocol: single `postMessage` per page; scroll/selection events as small messages. No batching,
  no shared memory, no streaming — the fallbacks in the spec are all unnecessary.
- Frame posture: `sandbox="allow-scripts"` only; external bootstrap script; frame CSP names the
  app origin explicitly and allows `img-src data:` (asset localisation will decide the final
  image story).
- `capabilities/default.json` exists now; extend it only deliberately.
- An interactive frame-pacing check belongs in S1-13's acceptance criteria, since it is the one
  criterion this spike could not close headlessly.
