# SPIKE-001 — the menu bar, without Swift

**Date:** 2026-07-30 · **Status:** complete · **Verdict:** no Swift shell is needed, and the
spike's stated fallback is the whole answer. The finding that matters is not the one the spike
asked about: **registering a shortcut macOS reserves succeeds, and then never fires.**
**Spec:** [`docs/plans/07-technical-spikes.md`](../plans/07-technical-spikes.md) § SPIKE-001.
**Gates:** S4-6 ([`docs/plans/18`](../plans/18-implementation-plan.md) § Stage 4).

## The question was already half-answered

SPIKE-001 asks: *"Can we achieve native macOS menu bar integration with Tauri while using Swift
for the AppKit shell?"* — and its investigation tasks are all about a Swift plugin and Swift ↔
Rust ↔ JS IPC latency.

**That premise was retired before this ran.** ADR and `CLAUDE.md` both record that Tauri *is* the
application shell; there is no separate Swift/AppKit shell to integrate with. So the question the
spike actually had to answer is the one behind it: **can Tome get an `NSStatusItem` and a
system-wide hotkey without dropping to Objective-C?**

It can. Both are in one file, [`src-tauri/src/tray.rs`](../../src-tauri/src/tray.rs), 170 lines,
no `unsafe`, no bridge, no second process. The spike's own fallback — "pure Tauri menu bar
(limited features)" — turned out not to be limited in any way this product needs.

## What was measured

Built as a debug bundle, launched with a throwaway `TOME_HOME`, and observed through the log file
that S4-3 had just added. This is what it wrote:

```
2026-07-30T17:03:46.860261Z  INFO tome_app_lib: library ready state=/var/folders/…/state
2026-07-30T17:03:47.184935Z  INFO tome_app_lib::tray: menu bar item created
```

| Criterion (from the spike) | Outcome |
|---|---|
| Menu bar icon renders and responds to clicks | **Yes**, via `tauri`'s `tray-icon` feature — `TrayIconBuilder`, which is `NSStatusItem` underneath. |
| Can invoke Rust from the status item | **Yes**, `on_menu_event` and `on_tray_icon_event` are Rust closures. There is nothing to invoke *across*. |
| Can trigger the UI from Rust | **Yes**, `AppHandle::emit`. The frontend listens for `activate` and opens search. |
| IPC latency < 10 ms | **Not applicable, and that is the finding.** There is no Swift ↔ Rust boundary to measure. The remaining hop is Rust → webview, which is the same `emit`/`listen` path SPIKE-002 already measured for page delivery. |
| Working prototype in `/spikes/tauri-swift/` | **Not built.** A prototype of an integration that does not exist would be a prototype of nothing. The implementation is the artifact. |

### The global shortcut

`tauri-plugin-global-shortcut` (`RegisterEventHotKey`). Registering the default:

```
2026-07-30T17:04:46.184472Z  INFO tome_app_lib::tray: global shortcut registered accelerator="CmdOrCtrl+Shift+D"
```

## The finding that matters

The spike's success criteria say nothing about conflicts. P5-009's do: *"conflict detection with
other apps"*. The obvious implementation is to report a failed registration, and the first draft
of `tray.rs` said so in a comment: *"`RegisterEventHotKey` fails when another application already
holds the combination, and that is the only conflict detection macOS offers. The failure IS the
detection."*

**That is wrong, and it took one experiment to find out.** The default was temporarily changed to
`CmdOrCtrl+Space` — Spotlight's — and the app relaunched:

```
2026-07-30T17:05:38.450807Z  INFO tome_app_lib::tray: global shortcut registered accelerator="CmdOrCtrl+Space"
```

It **registered successfully**. The handler never fires, because macOS consumes the keystroke
before any application sees it. `RegisterEventHotKey` refuses a combination held by another
*application's* hotkey; it does not refuse one held by the system, and there is no API that lists
either.

So:

- **"It registered" is not evidence that it works.** The only way a user finds out is by pressing
  the key and watching nothing happen — a silent failure, in the class this repository keeps a
  file about.
- Conflict detection has to be **two-sided**: the registration error (another app's hotkey) plus
  a refusal list for combinations macOS reserves. The second half is in
  [`src/lib/accelerator.ts`](../../src/lib/accelerator.ts), with the reserved set and the
  requirement of at least two modifiers — a global `⌘K` would override the frontmost
  application's own `⌘K` everywhere, for as long as Tome is running.

## What was not measured

- **A real keypress.** Driving the hotkey end to end needs synthesised system events, which need
  Accessibility permission that neither this session nor CI has. The registration is observed; the
  delivery is not. The frontend's half — that the right accelerator reaches Rust — is unit-tested.
- **The icon in a real menu bar.** The template image is asserted to be RGBA with alpha and is
  drawn from a script; whether it reads well beside the system's own icons at 22 points is a
  judgement, and two earlier versions were discarded on exactly that ground (recorded in
  `scripts/make-tray-icon.mjs`).

## Consequences

- **`docs/plans/07`'s SPIKE-001 is answered**, with its question corrected: the Swift half was
  moot before it ran.
- **S4-6 is unblocked and built** in the same change.
- **`tauri` gains the `tray-icon` and `image-png` features**; `tauri-plugin-global-shortcut` joins
  the tree. `png` was already there, so the icon decoder adds a feature rather than a crate.
- **The reserved-shortcut list is a maintenance liability** and should be treated as one: it is a
  snapshot of macOS 26's defaults, and a user who has rebound their own system shortcuts will
  find it both incomplete and occasionally wrong. It is still better than silence.
