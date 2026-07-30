#!/usr/bin/env bash
#
# Build the `tome` CLI and stage it where Tauri expects a sidecar.
#
# The cask symlinks `Tome.app/Contents/MacOS/tome` onto `PATH`, so `brew
# install --cask tome` has to deliver the app AND the CLI from the same build.
# That is what makes them resolve the same library (ADR-0002): two binaries
# built at different times against different code would agree on the *paths*
# and disagree on the *schema*, which fails at read time and not at install
# time.
#
# Tauri's `externalBin` mechanism wants `<name>-<target-triple>` next to the
# config and copies it into `Contents/MacOS/<name>`, dropping the triple. The
# triple suffix is not decoration -- it is how a universal or cross build picks
# the right binary -- so it is computed from rustc, never hard-coded.
#
#   ./scripts/build-cli-sidecar.sh            # release (what a DMG ships)
#   TOME_CLI_PROFILE=debug ./scripts/…        # debug (what the gate builds)

set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE="${TOME_CLI_PROFILE:-release}"
# `TOME_CLI_TARGET` mirrors whatever `tauri build --target` was given. Tauri
# looks for the sidecar under the TARGET triple, not the host one, so on any
# cross build (and on the release workflow, which names its target explicitly)
# the host triple would stage a file the bundler never finds.
TRIPLE="${TOME_CLI_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"

if [[ -z "$TRIPLE" ]]; then
  echo "could not determine the target triple from rustc -vV" >&2
  exit 1
fi

# Bash 3.2 (what macOS ships) treats `"${arr[@]}"` on an EMPTY array as an
# unbound variable under `set -u`, so the expansion below is guarded rather
# than written the obvious way.
TARGET_ARGS=()
OUT_DIR="target"
if [[ -n "${TOME_CLI_TARGET:-}" ]]; then
  TARGET_ARGS=(--target "$TOME_CLI_TARGET")
  OUT_DIR="target/$TOME_CLI_TARGET"
fi

case "$PROFILE" in
  release) cargo build --release -p tome-cli ${TARGET_ARGS[@]+"${TARGET_ARGS[@]}"}; BUILT="$OUT_DIR/release/tome" ;;
  debug)   cargo build -p tome-cli ${TARGET_ARGS[@]+"${TARGET_ARGS[@]}"};           BUILT="$OUT_DIR/debug/tome"   ;;
  *) echo "TOME_CLI_PROFILE must be 'release' or 'debug', got '$PROFILE'" >&2; exit 1 ;;
esac

mkdir -p src-tauri/binaries
DEST="src-tauri/binaries/tome-${TRIPLE}"
# `cp` rather than a symlink: the bundler copies what it finds, and a symlink
# into `target/` would put a dangling link inside the .app on any machine that
# then cleaned the build directory.
cp "$BUILT" "$DEST"
chmod +x "$DEST"

echo "staged $PROFILE CLI → $DEST"
