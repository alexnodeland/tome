#!/usr/bin/env bash
#
# Verify a built Tome.app — the checks that pass every unit test and still
# ship a broken release.
#
# S4-9's whole point is that `Tome.app/Contents/MacOS/tome` and the app beside
# it come from ONE build, so they resolve one library (ADR-0002). Nothing about
# that is visible to `cargo test`: the bundle is made by the bundler, after
# every test has already passed. This script is the only thing that looks at
# the artifact a user installs.
#
#   ./scripts/verify-bundle.sh                       # newest bundle under target/
#   ./scripts/verify-bundle.sh path/to/Tome.app      # a specific one
#
# `./scripts/check.sh` runs it after the app build (so not under --fast), and
# the release workflow runs it against the artifact it is about to publish.

set -uo pipefail
cd "$(dirname "$0")/.."

CASK="packaging/homebrew/Casks/tome.rb"
FAILED=0
fail() { printf '  ✗ %s\n' "$1"; FAILED=$((FAILED + 1)); }
pass() { printf '  ✓ %s\n' "$1"; }

APP="${1:-}"
if [[ -z "$APP" ]]; then
  # The MOST RECENTLY BUILT, not release-first. Preferring release meant that
  # a tree with both would verify a bundle nobody had just built -- and then
  # fail the same-build check against the sidecar staged for the other one,
  # which is a false negative that reads exactly like a real defect. Callers
  # that know which bundle they mean should pass it; `check.sh` does.
  NEWEST=0
  for candidate in target/release/bundle/macos/Tome.app target/debug/bundle/macos/Tome.app; do
    [[ -d "$candidate" ]] || continue
    STAMP=$(stat -f %m "$candidate/Contents/MacOS" 2>/dev/null || echo 0)
    if [[ "$STAMP" -gt "$NEWEST" ]]; then NEWEST=$STAMP; APP="$candidate"; fi
  done
fi
if [[ -z "$APP" || ! -d "$APP" ]]; then
  echo "no Tome.app found. Build one first:" >&2
  echo "  npm run tauri build -- --debug --bundles app" >&2
  exit 1
fi
echo "verifying $APP"

CLI="$APP/Contents/MacOS/tome"
APPBIN="$APP/Contents/MacOS/tome-app"
PLIST="$APP/Contents/Info.plist"

# 1. The CLI is there at all. This is the whole ticket: a bundle without it
#    installs cleanly, launches fine, and breaks every integration.
if [[ -x "$CLI" ]]; then
  pass "Contents/MacOS/tome exists and is executable"
else
  fail "Contents/MacOS/tome is missing — the cask symlinks it onto PATH"
  echo; echo "$FAILED check(s) failed."; exit 1
fi

# 2. Same architectures as the app. A cross or universal build that produced a
#    host-only CLI passes every other check here and fails on the user's
#    machine with "bad CPU type".
CLI_ARCHS=$(lipo -archs "$CLI" 2>/dev/null | tr ' ' '\n' | sort | tr '\n' ' ')
APP_ARCHS=$(lipo -archs "$APPBIN" 2>/dev/null | tr ' ' '\n' | sort | tr '\n' ' ')
if [[ "$CLI_ARCHS" == "$APP_ARCHS" && -n "$CLI_ARCHS" ]]; then
  pass "architectures match the app (${CLI_ARCHS% })"
else
  fail "architecture mismatch: cli [${CLI_ARCHS% }] vs app [${APP_ARCHS% }]"
fi

# 3. Byte-identical to the sidecar this tree staged — the "same build" proof.
#    Skipped rather than failed when the sidecar is absent, because the release
#    workflow verifies a downloaded artifact with no build tree beside it.
TRIPLE="$(rustc -vV | sed -n 's/^host: //p' 2>/dev/null)"
STAGED="src-tauri/binaries/tome-${TRIPLE}"
if [[ -n "$TRIPLE" && -f "$STAGED" ]]; then
  if [[ "$(shasum -a 256 <"$CLI")" == "$(shasum -a 256 <"$STAGED")" ]]; then
    pass "identical to the staged sidecar — app and CLI are one build"
  else
    fail "bundled tome differs from $STAGED — the bundle has a stale CLI"
  fi
else
  printf '  — same-build check skipped (no staged sidecar)\n'
fi

# 4. One version number. Two would mean the bundler picked up an old binary.
CLI_VERSION=$(TOME_HOME="$(mktemp -d)" "$CLI" --version | awk '{print $2}')
APP_VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$PLIST" 2>/dev/null)
if [[ "$CLI_VERSION" == "$APP_VERSION" && -n "$CLI_VERSION" ]]; then
  pass "version agrees with Info.plist ($CLI_VERSION)"
else
  fail "version mismatch: tome says '$CLI_VERSION', Info.plist says '$APP_VERSION'"
fi

# 5. The exit gate's own sentence: `tome status` reports the paths the app
#    uses. Run WITHOUT TOME_HOME, because the override is exactly what would
#    hide a wrong default. `status` is read-only and creates nothing.
STATUS=$(env -u TOME_HOME "$CLI" status --json)
read_path() { printf '%s' "$STATUS" | python3 -c "import json,sys;print(json.load(sys.stdin)['$1'])"; }
STATE=$(read_path state)
CACHE=$(read_path cache)
if [[ "$STATE" == "$HOME/Library/Application Support/Tome" ]]; then
  pass "state root is Application Support/Tome"
else
  fail "state root is '$STATE', not ~/Library/Application Support/Tome (ADR-0002)"
fi
if [[ "$CACHE" == "$HOME/Library/Caches/Tome" ]]; then
  pass "cache root is Caches/Tome"
else
  fail "cache root is '$CACHE', not ~/Library/Caches/Tome (ADR-0002)"
fi

# 6. The zap list covers what the binary actually writes. This is the check
#    P5-012 was written for: the original zap named three directories no single
#    version ever used. Comparing against the running binary means the list
#    cannot rot when a path moves.
if [[ -f "$CASK" ]]; then
  ZAP_OK=1
  for p in "$STATE" "$CACHE"; do
    tilde="~${p#"$HOME"}"
    grep -qF "\"$tilde\"" "$CASK" || { fail "cask zap list is missing $tilde"; ZAP_OK=0; }
  done
  [[ $ZAP_OK -eq 1 ]] && pass "cask zap list covers every path tome status reports"

  # 7. The two lines the release workflow rewrites. A reformat that broke
  #    these would publish a cask pinned to 0.0.0 with a checksum for nothing.
  grep -qE '^  version "[0-9]+\.[0-9]+\.[0-9]+"$' "$CASK" \
    && pass "cask version line matches the release rewrite pattern" \
    || fail "cask version line no longer matches the release workflow's pattern"
  grep -qE '^  sha256 "[0-9a-f]{64}"$' "$CASK" \
    && pass "cask sha256 line matches the release rewrite pattern" \
    || fail "cask sha256 line no longer matches the release workflow's pattern"
else
  fail "$CASK is missing — it is the cask's source of truth"
fi

echo
if [[ $FAILED -eq 0 ]]; then
  echo "bundle verified."
  exit 0
fi
echo "$FAILED check(s) failed."
exit 1
