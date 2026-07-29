#!/usr/bin/env bash
#
# SPIKE-003 — drive the harness and capture peak RSS from outside it.
#
# Every phase runs as its own process under `/usr/bin/time -l`, whose
# "maximum resident set size" comes from the kernel. Measuring from inside
# would need `unsafe` for getrusage, and a program reporting its own peak
# memory is the measurement most likely to be quietly wrong.
#
#   ./run.sh            # the full run: 10k, 50k, 100k, budgets, incremental
#   ./run.sh quick      # 10k only, for checking the harness works
#
# Output is one `RESULT` line per measurement, so the write-up can quote the
# raw capture rather than a summary of it.

set -uo pipefail
cd "$(dirname "$0")"

MODE="${1:-full}"
WORK="${TMPDIR:-/tmp}/spike003"
BIN=target/release/spike003

[[ -x "$BIN" ]] || { echo "build first: cargo build --release"; exit 1; }

# `/usr/bin/time -l` writes its report to stderr, and so does the program.
# Capture stderr to a file, let stdout through so the harness's own RESULT
# lines are visible, then pull the one line that matters.
#
# macOS reports "maximum resident set size" in BYTES here; Linux reports
# kilobytes. This spike only ever runs on macOS (see the header), but the
# unit is stated because a number that is 1024x wrong looks plausible.
measure() {
  local label="$1"; shift
  local err rss
  err=$(mktemp)
  /usr/bin/time -l "$@" 2>"$err"
  local status=$?
  rss=$(awk '/maximum resident set size/ {print $1; exit}' "$err")
  if [[ $status -ne 0 ]]; then
    echo "FAILED $label (exit $status)"
    tail -3 "$err"
  fi
  rm -f "$err"
  awk -v l="$label" -v r="${rss:-0}" \
    'BEGIN { printf "RESULT rss label=%-34s peak_mb=%.0f\n", l, r/1048576 }'
}

index_size_mb() {
  du -sk "$1" 2>/dev/null | awk '{printf "%.0f", $1/1024}'
}

echo "# SPIKE-003 raw capture"
echo "# $(date -u +%Y-%m-%dT%H:%M:%SZ)  $(uname -sm)  $(sysctl -n machdep.cpu.brand_string 2>/dev/null)"
echo "# tantivy $(grep -m1 '^tantivy' Cargo.toml | tr -d '" ')"
echo

# --- the floor: what a process pays before any index exists ---------------
measure "idle (no index opened)" "$BIN" idle

# --- scale sweep -----------------------------------------------------------
if [[ "$MODE" == quick ]]; then
  SIZES=(10000)
else
  SIZES=(10000 50000 100000)
fi

for n in "${SIZES[@]}"; do
  dir="$WORK/idx-$n"
  rm -rf "$dir"
  echo "── indexing $n pages ──"
  measure "index $n pages (128MB budget)" "$BIN" index --dir "$dir" --pages "$n" --budget-mb 128
  echo "RESULT disk pages=$n index_mb=$(index_size_mb "$dir")"
  measure "open $n-page index" "$BIN" open --dir "$dir"
  echo "── searching $n pages ──"
  "$BIN" search --dir "$dir" --rounds 200 | grep -E '^(RESULT|segments)'
  measure "search $n pages (peak)" "$BIN" search --dir "$dir" --rounds 200
  echo
done

# --- does the writer budget actually control peak RSS? ---------------------
if [[ "$MODE" != quick ]]; then
  echo "── writer budget sweep, 50k pages ──"
  for budget in 50 128 512; do
    dir="$WORK/budget-$budget"
    rm -rf "$dir"
    measure "index 50k budget=${budget}MB" "$BIN" index --dir "$dir" --pages 50000 --budget-mb "$budget"
    echo "RESULT disk budget=$budget index_mb=$(index_size_mb "$dir")"
    rm -rf "$dir"
  done
  echo

  # --- incremental: adding to an index that already exists -----------------
  echo "── incremental on top of 100k ──"
  dir="$WORK/idx-100000"
  if [[ -d "$dir" ]]; then
    measure "incremental +1k onto 100k" "$BIN" index --dir "$dir" --pages 1000 --start-at 100000 --budget-mb 128
    echo "RESULT disk after_incremental index_mb=$(index_size_mb "$dir")"
  else
    echo "SKIP incremental — no 100k index"
  fi
fi

echo
echo "# work directory: $WORK (delete when done)"
