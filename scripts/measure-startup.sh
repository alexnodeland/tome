#!/usr/bin/env bash
#
# Measure app startup and idle memory (S4-2, P5-001).
#
# The two numbers the NFRs name — "startup < 500 ms cold", "memory < 200 MB
# idle" — were budgets nobody had ever put a number against. This puts one
# against them, and defines what it is measuring, because "startup" has at
# least three meanings and the flattering one is not the useful one.
#
# **What is measured:** wall clock from `exec` to the first `library_location`
# call. That is the backend up, the window created, the webview loaded, the
# bundle parsed, and the UI's first IPC round trip — everything except the
# paint that follows it. First paint is NOT measured: nothing in the app can
# observe it from outside the webview, and a number derived from a `setTimeout`
# would be a number about the timer.
#
# **What "cold" means here:** not cold. The page cache holds the binary after
# the first run, and a genuinely cold measurement needs `purge`, which needs
# root and evicts the whole system's cache. The first iteration below is
# reported separately for that reason — it is the closest thing to cold this
# gets, and it is consistently the slowest.
#
#   ./scripts/measure-startup.sh          # 5 runs
#   ./scripts/measure-startup.sh 10       # 10 runs

set -euo pipefail
cd "$(dirname "$0")/.."

RUNS="${1:-5}"
APP="target/debug/bundle/macos/Tome.app/Contents/MacOS/tome-app"
[[ -x "$APP" ]] || APP="target/release/bundle/macos/Tome.app/Contents/MacOS/tome-app"
if [[ ! -x "$APP" ]]; then
  echo "no built app. Run: npm run tauri build -- --debug --bundles app" >&2
  exit 1
fi
echo "measuring $APP over $RUNS run(s)"
echo

TIMES=()
RSS=()
for i in $(seq 1 "$RUNS"); do
  HOME_DIR="$(mktemp -d)"
  START=$(python3 -c 'import time; print(time.time())')
  # `tome=debug` reaches the per-command logs; the default filter stops at
  # info, so this is measurement-only verbosity and not what a user's log has.
  RUST_LOG=warn,tome=debug TOME_HOME="$HOME_DIR" "$APP" >/dev/null 2>&1 &
  PID=$!

  # Wait for the first `library_location`, which is the UI's first IPC call.
  READY=""
  for _ in $(seq 1 200); do
    if grep -q "library_location" "$HOME_DIR"/state/logs/*.log 2>/dev/null; then
      READY=$(python3 -c 'import time; print(time.time())')
      break
    fi
    python3 -c 'import time; time.sleep(0.05)'
  done

  if [[ -z "$READY" ]]; then
    echo "run $i: never became ready" >&2
    kill "$PID" 2>/dev/null || true
    rm -rf "$HOME_DIR"
    continue
  fi

  MS=$(python3 -c "print(round(($READY - $START) * 1000))")
  TIMES+=("$MS")

  # Idle memory, after the app has settled. Resident set of the app process
  # only: the webview runs in its own processes and `ps` will not attribute
  # them here, which is stated rather than quietly ignored.
  python3 -c 'import time; time.sleep(2)'
  KB=$(ps -o rss= -p "$PID" 2>/dev/null | tr -d ' ' || echo 0)
  RSS+=("$((KB / 1024))")

  # ${arr[-1]} is a bash 4 feature and macOS ships bash 3.2 -- the same trap
  # that bit `mapfile` in the registry verifier.
  printf 'run %d: ready in %s ms, %s MB resident\n' "$i" "$MS" "${RSS[${#RSS[@]}-1]}"
  kill "$PID" 2>/dev/null || true
  wait "$PID" 2>/dev/null || true
  rm -rf "$HOME_DIR"
done

echo
python3 - "${TIMES[@]}" <<'PY'
import sys
times = [int(t) for t in sys.argv[1:]]
if not times:
    raise SystemExit('no successful runs')
times_sorted = sorted(times)
print(f'startup: first {times[0]} ms · median {times_sorted[len(times_sorted)//2]} ms · '
      f'best {times_sorted[0]} ms · worst {times_sorted[-1]} ms')
PY
python3 - "${RSS[@]}" <<'PY'
import sys
rss = [int(r) for r in sys.argv[1:] if int(r) > 0]
if rss:
    print(f'idle memory (app process): median {sorted(rss)[len(rss)//2]} MB · max {max(rss)} MB')
    print('  (the webview runs in separate processes and is not counted here)')
PY
