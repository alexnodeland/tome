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
run "rust: tests"           cargo test -p tome-core -p tome-cli -p tome-testkit
# Type-check only. Fuzzing itself is unbounded and belongs in a scheduled run;
# what rots between those runs is a target that no longer compiles against the
# module it fuzzes, and that is cheap to catch here. Stable is enough for this
# (the nightly toolchain is only needed to *run* a target).
run "fuzz: targets compile" cargo check --manifest-path fuzz/Cargo.toml
run "frontend: types"       npm run check
run "frontend: lint"        npm run lint
run "frontend: tests"       npm run test
run "design: contrast"      node scripts/check-contrast.mjs
run "deps: npm advisories"  npm audit --audit-level=high

# cargo-deny and cargo-audit are optional locally: they are slow to install and
# CI runs them regardless. Check them if present rather than nagging.
if command -v cargo-deny >/dev/null 2>&1; then
  run "deps: licences and bans" cargo deny check
else
  printf '%s▸ deps: licences and bans%s\n  %s— skipped (cargo install cargo-deny)%s\n' \
    "$bold" "$reset" "$dim" "$reset"
fi
if command -v cargo-audit >/dev/null 2>&1; then
  run "deps: rust advisories" cargo audit
else
  printf '%s▸ deps: rust advisories%s\n  %s— skipped (cargo install cargo-audit)%s\n' \
    "$bold" "$reset" "$dim" "$reset"
fi

if [[ $FAST -eq 0 ]]; then
  run "app: builds and bundles" npm run tauri build -- --debug --bundles app
else
  printf '%s▸ app: builds and bundles%s\n  %s— skipped (--fast)%s\n' "$bold" "$reset" "$dim" "$reset"
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
