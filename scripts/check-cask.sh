#!/usr/bin/env bash
#
# Lint the cask with Homebrew's own linter.
#
# `brew style` refuses a cask that is not inside a tap ("Homebrew requires
# casks to be in a tap"), and Tome's cask lives in this repository because the
# tap is a mirror. So: stage it in a throwaway tap, lint it there, remove the
# tap. The alternative -- claiming the cask was linted because it looks like
# other casks -- is what the CHANGELOG said before this script existed.
#
#   ./scripts/check-cask.sh          # lint
#   ./scripts/check-cask.sh --fix    # lint and write back the corrections

set -euo pipefail
cd "$(dirname "$0")/.."

CASK="packaging/homebrew/Casks/tome.rb"

if ! command -v brew >/dev/null 2>&1; then
  echo "brew is not installed; skipping the cask lint" >&2
  exit 0
fi

TAP_ROOT="$(brew --repository)/Library/Taps/tome-lint"
TAP="$TAP_ROOT/homebrew-lint"
# The tap directory is inside Homebrew's own tree, so leaving one behind would
# make `brew update` complain about a tap the user never added.
trap 'rm -rf "$TAP_ROOT"' EXIT

rm -rf "$TAP_ROOT"
mkdir -p "$TAP/Casks"
cp "$CASK" "$TAP/Casks/tome.rb"

if [[ "${1:-}" == "--fix" ]]; then
  brew style --fix --cask "$TAP/Casks/tome.rb" || true
  cp "$TAP/Casks/tome.rb" "$CASK"
  echo "corrections written back to $CASK"
  exit 0
fi

brew style --cask "$TAP/Casks/tome.rb"
