#!/usr/bin/env bash
# Verify every registry source against its LIVE site (S3-8 / PRD § Source Registry).
#
# This is the mitigation for RISK-003 — scraper rot — and its whole value is
# that it fetches real sites. That is also why it is NOT part of
# ./scripts/check.sh: a gate that fails when someone else's website is down
# teaches everyone to ignore the gate. Run it on a schedule, read the output,
# and open an issue for what broke.
#
# The offline half — configs parse, index agrees with configs, robots and rate
# limits are not overridden — IS in the gate, as
# `cargo test -p tome-core --test registry`.
#
#   ./scripts/verify-registry.sh              # verify every source
#   ./scripts/verify-registry.sh rust-std     # verify one
#   TOME_VERIFY_UPDATE=1 ./scripts/verify-registry.sh   # write back `verified:` dates
#
# Each source is pulled into a THROWAWAY library (a temp TOME_HOME), never the
# user's own: verification must not touch a real library, and a run that
# half-succeeded must leave nothing behind.

set -euo pipefail

cd "$(dirname "$0")/.."
REGISTRY="registry"
INDEX="$REGISTRY/index.yaml"

# A verification pull is bounded: this is a health check, not an ingest. A
# source that produces pages in the first 25 is working; one that produces
# zero is broken, and that is the entire signal being measured.
MAX_PAGES=25

bold=$(tput bold 2>/dev/null || true)
red=$(tput setaf 1 2>/dev/null || true)
green=$(tput setaf 2 2>/dev/null || true)
dim=$(tput dim 2>/dev/null || true)
reset=$(tput sgr0 2>/dev/null || true)

TOME_BIN="${TOME_BIN:-target/release/tome}"
if [[ ! -x "$TOME_BIN" ]]; then
  TOME_BIN="target/debug/tome"
fi
if [[ ! -x "$TOME_BIN" ]]; then
  echo "${red}No tome binary at $TOME_BIN. Run: cargo build${reset}" >&2
  exit 1
fi

# Source ids to verify: the argument, or every id in the index.
if [[ $# -gt 0 ]]; then
  ids=("$@")
else
  # Not `mapfile`: macOS ships bash 3.2, which does not have it, and the
  # failure is a bare "command not found" halfway through a script that has
  # already printed a banner.
  ids=()
  while IFS= read -r line; do
    ids+=("$line")
  done < <(grep -E '^[[:space:]]+- id: ' "$INDEX" | sed 's/.*- id: //')
fi

today=$(date -u +%Y-%m-%d)
failed=()
passed=()

for id in "${ids[@]}"; do
  config="$REGISTRY/sources/$id.yaml"
  if [[ ! -f "$config" ]]; then
    echo "${red}✗ $id — no config at $config${reset}" >&2
    failed+=("$id")
    continue
  fi

  echo "${bold}▸ $id${reset}"
  home=$(mktemp -d)
  # shellcheck disable=SC2064  # expand $home now, not at trap time
  trap "rm -rf '$home'" EXIT
  mkdir -p "$home/state/sources"

  # The config is copied VERBATIM and capped with `--max-pages`, a runtime
  # override. Editing the YAML instead — the first version of this script did,
  # with sed — verifies a file nobody runs, and silently failed to cap
  # rustdoc/mdbook/readthedocs sources at all, because those types carry no
  # `max_pages:` line for a sed to find. That is how a two-minute health check
  # became a full crawl of the Cargo Book.
  cp "$config" "$home/state/sources/$id.yaml"

  output=$(TOME_HOME="$home" "$TOME_BIN" pull "$id" --max-pages "$MAX_PAGES" --json --quiet 2>&1) || {
    echo "${red}✗ $id — pull failed${reset}"
    echo "$output" | sed 's/^/    /'
    failed+=("$id")
    rm -rf "$home"; trap - EXIT
    continue
  }

  pages=$(echo "$output" | sed -n 's/.*"pages":\([0-9]*\).*/\1/p' | head -1)
  pages=${pages:-0}
  if [[ "$pages" -gt 0 ]]; then
    echo "${green}✓ $id — $pages pages${reset}"
    passed+=("$id")
  else
    # Zero pages from a config that parsed and a site that answered is
    # exactly what scraper rot looks like: the selectors no longer match.
    echo "${red}✗ $id — pulled 0 pages (the site answered; the scraper found nothing)${reset}"
    echo "$output" | sed 's/^/    /'
    failed+=("$id")
  fi

  rm -rf "$home"; trap - EXIT
done

echo
echo "${bold}${#passed[@]} verified, ${#failed[@]} failed${reset}"

if [[ "${TOME_VERIFY_UPDATE:-0}" == "1" && ${#passed[@]} -gt 0 ]]; then
  # Write today's date against each source that passed. Only passes are
  # written: a failure must leave the OLD date visible, because "last known
  # good" is the information someone triaging needs.
  for id in "${passed[@]}"; do
    python3 - "$INDEX" "$id" "$today" <<'PY'
import re, sys
path, source_id, today = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(path).read()
# Rewrite the `verified:` line inside this id's block only.
pattern = re.compile(
    r"(- id: " + re.escape(source_id) + r"\b(?:\n(?!  - id: ).*)*?\n\s+verified: )(.*)"
)
new, count = pattern.subn(lambda m: m.group(1) + today, text, count=1)
if count != 1:
    sys.exit(f"could not find a `verified:` line for {source_id}")
open(path, "w").write(new)
PY
  done
  # Single-quoted around the field name: backticks inside a double-quoted
  # string are command substitution, and `verified:` is not a command.
  echo "${dim}Updated 'verified' dates for the sources that passed.${reset}"
  echo "${dim}Read the diff before committing it — it is a claim about other people's sites.${reset}"
fi

if [[ ${#failed[@]} -gt 0 ]]; then
  echo "${red}Broken: ${failed[*]}${reset}" >&2
  exit 1
fi
