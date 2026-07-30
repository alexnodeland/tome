#!/usr/bin/env bash
#
# Everything CI runs, locally, in one command.
#
# This exists because the repository is private and GitHub Actions is not
# available until it goes public. The implementation plan's gates are supposed
# to be machine-checked; without CI, this script IS the gate. Run it before
# every commit and before merging anything.
#
#   ./scripts/check.sh          # the full set
#   ./scripts/check.sh --fast   # skip the app build (~90s faster)
#
# Keep in lockstep with .github/workflows/ci.yml. If they drift, the CI that
# eventually runs will fail on things that passed here, which is the failure
# mode this script exists to prevent.

set -uo pipefail
cd "$(dirname "$0")/.."

FAST=0
[[ "${1:-}" == "--fast" ]] && FAST=1

bold=$(tput bold 2>/dev/null || true); red=$(tput setaf 1 2>/dev/null || true)
green=$(tput setaf 2 2>/dev/null || true); dim=$(tput dim 2>/dev/null || true)
reset=$(tput sgr0 2>/dev/null || true)

FAILED=()

run() {
  local name="$1"; shift
  printf '%s▸ %s%s\n' "$bold" "$name" "$reset"
  local out
  if out=$("$@" 2>&1); then
    printf '  %s✓ pass%s\n' "$green" "$reset"
  else
    printf '  %s✗ FAIL%s\n' "$red" "$reset"
    printf '%s%s%s\n' "$dim" "$(echo "$out" | tail -25)" "$reset"
    FAILED+=("$name")
  fi
}

echo
run "rust: formatting"      cargo fmt --all --check
run "rust: lints"           cargo clippy -p tome-core -p tome-cli -p tome-testkit --all-targets -- -D warnings
# Before any step that compiles tome-app: `bundle.externalBin` makes the CLI a
# sidecar, and src-tauri/build.rs refuses to compile without it staged. Debug,
# because everything else here is debug -- a release CLI would rebuild the
# whole dependency tree for a binary this run only checks the wiring of.
# (`env` rather than a `VAR=x run …` prefix: `run` is a shell function, and an
# assignment prefixed to a function call leaks into the shell in bash.)
run "cli: sidecar staged" env TOME_CLI_PROFILE=debug ./scripts/build-cli-sidecar.sh
# tome-app is included here but NOT in the clippy line above: its tests
# cover the `tome://` asset protocol handler, which is the one place a
# string from page content becomes a filesystem path. Clippy still runs
# without it, matching CI's Linux lint job, which cannot build it.
run "rust: tests"           cargo test -p tome-core -p tome-cli -p tome-testkit -p tome-app
# Type-check only. Fuzzing itself is unbounded and belongs in a scheduled run;
# what rots between those runs is a target that no longer compiles against the
# module it fuzzes, and that is cheap to catch here. Stable is enough for this
# (the nightly toolchain is only needed to *run* a target).
run "fuzz: targets compile" cargo check --manifest-path fuzz/Cargo.toml
run "frontend: types"       npm run check
run "frontend: lint"        npm run lint
run "frontend: tests"       npm run test
run "design: contrast"      node scripts/check-contrast.mjs
# The site is deployed by a workflow that has never run (Actions is blocked at
# the account level), so this is the only thing standing between a broken
# build script and a broken deploy on the day it can.
run "site: builds"          node site/build.mjs
# One version, in two files npm and Cargo each insist on owning. The bundle's
# CFBundleShortVersionString comes from Cargo (tauri.conf.json has no version
# key), so a drift here means the DMG and the CLI disagree.
run "release: versions agree" ./scripts/set-version.sh --check
run "deps: npm advisories"  npm audit --audit-level=high

# Homebrew's own linter, on the cask this repository is the source of truth
# for. It refuses casks outside a tap, so the script stages one; skipped when
# brew is absent, like cargo-deny below.
if command -v brew >/dev/null 2>&1; then
  run "release: cask style" ./scripts/check-cask.sh
else
  printf '%s▸ release: cask style%s\n  %s— skipped (no brew)%s\n' "$bold" "$reset" "$dim" "$reset"
fi

# cargo-deny and cargo-audit are optional locally: they are slow to install and
# CI runs them regardless. Check them if present rather than nagging.
if command -v cargo-deny >/dev/null 2>&1; then
  run "deps: licences and bans" cargo deny check
else
  printf '%s▸ deps: licences and bans%s\n  %s— skipped (cargo install cargo-deny)%s\n' \
    "$bold" "$reset" "$dim" "$reset"
fi
# `cargo audit` was here and has been removed, to stay in lockstep with
# ci.yml. It reads Cargo.lock unscoped, so it fails on unmaintained advisories
# for Tauri's Linux GTK bindings that no macOS build ever compiles — which is
# exactly what broke CI's first real run. `cargo deny check` above reads the
# same RustSec database, is scoped to aarch64-apple-darwin, and keeps dated
# per-advisory ignores so a new one still fails.

if [[ $FAST -eq 0 ]]; then
  run "app: builds and bundles" \
    env TOME_CLI_PROFILE=debug npm run tauri build -- --debug --bundles app
  # Only meaningful on a bundle that was just built, which is why it lives here
  # and not with the other gates: it inspects the artifact a user installs.
  # The path is explicit: a tree that also holds a release bundle would
  # otherwise have the verifier choose between them, and the one it did not
  # choose is the one this run built.
  run "app: bundle ships the CLI" \
    ./scripts/verify-bundle.sh target/debug/bundle/macos/Tome.app
else
  printf '%s▸ app: builds and bundles%s\n  %s— skipped (--fast)%s\n' "$bold" "$reset" "$dim" "$reset"
  printf '%s▸ app: bundle ships the CLI%s\n  %s— skipped (--fast; needs a bundle)%s\n' \
    "$bold" "$reset" "$dim" "$reset"
fi

echo
if [[ ${#FAILED[@]} -eq 0 ]]; then
  printf '%s%sAll checks passed.%s\n\n' "$bold" "$green" "$reset"
  exit 0
fi
printf '%s%s%d check(s) failed:%s\n' "$bold" "$red" "${#FAILED[@]}" "$reset"
printf '  · %s\n' "${FAILED[@]}"
echo
exit 1
