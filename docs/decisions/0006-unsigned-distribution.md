# ADR-0006: Distribute unsigned via an own Homebrew tap; defer the Apple Developer Program

**Status:** Accepted
**Date:** 2026-07-28
**Deciders:** Alex Nodeland
**Resolves:** DEC-003 (deferred, not cancelled)

## Context

The plan assumed notarization from the start: `P5-010 macOS notarization setup` was Critical, and
the Phase 5 exit criterion was "notarized DMG installs from a clean machine and passes `spctl`".

Notarization requires a **Developer ID Application** certificate, which requires **Apple Developer
Program** membership at $99/yr. The only certificate currently on the machine is an *Apple
Development* certificate — that signs builds for registered development devices and **cannot be
used for distribution**, so it does not help.

The project is pre-1.0, has no users, no revenue, and its repository is private until release. A
standing annual fee to notarize software nobody has yet installed is the wrong order of spending.

There is also existing practice to follow: **`alexnodeland/homebrew-tap` already distributes
unsigned macOS apps** (`curio`, `statusbar`, `no-doze`) with an established cask convention —
`version :latest`, `sha256 :no_check`, GitHub `releases/latest/download/`, and `caveats`
documenting the Gatekeeper path.

## Decision

1. **Skip the Apple Developer Program for now.** No Developer ID signing, no notarization.
2. **Distribute through `alexnodeland/homebrew-tap`** as the single channel:
   `brew install --cask alexnodeland/tap/tome`.
3. **Keep the cask's source of truth in this repository** at `dist/homebrew/Casks/tome.rb`,
   mirrored to the tap on release — the same arrangement Curio uses.
4. **Ship the `tome` CLI inside the app bundle** and symlink it from the cask, following
   `statusbar.rb`. One install, one build, therefore one library — which is exactly the invariant
   [ADR-0002](./0002-no-app-sandbox.md) exists to protect.
5. **Keep the entitlements file and `hardenedRuntime: true` in the Tauri config.** They are inert
   without a signing identity, and leaving them in place means enabling notarization later is a
   credentials change rather than a configuration change.

## Consequences

### The real cost, stated plainly

**macOS Gatekeeper will refuse to open the app on first launch.** This is not a warning the user
can click through in one step, and on recent macOS it got harder:

| macOS | What the user must do |
|---|---|
| 12–14 | Control-click the app → *Open* → *Open* again |
| 15+ | The Control-click bypass was **removed**. System Settings → Privacy & Security → *Open Anyway* |
| any | `xattr -dr com.apple.quarantine /Applications/Tome.app` — works everywhere |

Homebrew applies the quarantine attribute by default, so `brew install --cask` does not avoid this;
`--no-quarantine` does, but telling users to pass that flag is asking them to disable a safety
control, so the caveats lead with the explicit `xattr` command instead.

**This will cost adoption.** Some proportion of people who install Tome will hit the Gatekeeper
dialog, conclude the app is broken or unsafe, and delete it. That is a genuine cost of the
decision, not a formality — and it is the main argument for revisiting DEC-003 at v1.0.

It is also completely ordinary for pre-1.0 developer tools distributed via Homebrew, and the target
audience (developers) is the group most likely to know what `xattr` does.

### Knock-on effects

- **`spctl --assess` will not pass.** The Phase 5 exit criterion changes from "passes Gatekeeper" to
  "installs from the tap and launches after the documented quarantine step".
- **iCloud sync also requires the Developer Program.** Deferring the membership reinforces
  [ADR-0005](./0005-agent-driven-build.md), which already deferred sync — consistent, and worth
  noting that if sync is ever wanted, the $99 returns as a hard prerequisite.
- **Mac App Store remains impossible**, which was already true ([ADR-0002](./0002-no-app-sandbox.md)).
- **Ad-hoc signing still happens.** Apple Silicon requires at least an ad-hoc signature to execute
  at all, and the toolchain applies one automatically. The app runs; it is simply not *trusted*.
- **No automatic update mechanism.** `brew upgrade --cask` is the update path, which suits a tool
  whose audience already uses Homebrew.

### What gets better

No annual fee, no certificate management, no notarization step in the release pipeline, and no
Apple-account credentials in CI — which removes the single most sensitive secret the release
workflow would otherwise hold.

## Alternatives considered

**Pay the $99 now.** The straightforwardly better user experience. Rejected as premature: the
product has no users, and the cost recurs annually whether or not it ships. **Revisit at v1.0** —
this is the natural trigger.

**Self-signed certificate.** Provides no Gatekeeper benefit whatsoever; an unknown CA is treated the
same as no signature, and it adds a certificate to manage. Strictly worse than ad-hoc.

**Source-only distribution (`cargo install` / build it yourself).** Higher friction than the
Gatekeeper step for most users, and it gives up the GUI app entirely. Rejected, though the source
build stays available and documented.

**Submit to `homebrew-cask` proper instead of an own tap.** Already rejected in the plan review:
homebrew-cask has notability requirements a brand-new project does not meet, and it would likely
reject an unsigned app besides.

## Reversibility

**Easy, and deliberately kept that way.** Enabling notarization later requires: enrolling in the
program, adding four secrets to CI, and adding a `notarytool submit` + `stapler staple` step to the
release workflow. Nothing about the application, the bundle identifier, or the entitlements
changes — which is why they stay in the config now.

The only user-visible migration is that the caveats text disappears.
