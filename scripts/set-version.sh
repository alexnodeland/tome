#!/usr/bin/env bash
#
# Set the release version everywhere it is written down.
#
# `Cargo.toml`'s `[workspace.package] version` is the source of truth: the
# `tome` binary reports it, and `tauri.conf.json` deliberately has NO `version`
# key so that the bundler falls back to the src-tauri crate's version and
# `CFBundleShortVersionString` cannot disagree with `tome --version`.
# `scripts/verify-bundle.sh` fails if they ever do.
#
# `package.json` still carries one, because npm requires it. It is checked
# against Cargo's by --check below, and by scripts/check.sh.
#
#   ./scripts/set-version.sh 0.1.0     # write it
#   ./scripts/set-version.sh --check   # assert the two agree

set -euo pipefail
cd "$(dirname "$0")/.."

cargo_version() {
  sed -n '/^\[workspace\.package\]/,/^\[/p' Cargo.toml |
    sed -n 's/^version = "\(.*\)"$/\1/p' | head -1
}
npm_version() {
  sed -n 's/^  "version": "\(.*\)",$/\1/p' package.json | head -1
}

if [[ "${1:-}" == "--check" ]]; then
  c="$(cargo_version)"; n="$(npm_version)"
  if [[ -z "$c" || -z "$n" ]]; then
    echo "could not read a version: Cargo.toml='$c' package.json='$n'" >&2
    exit 1
  fi
  if [[ "$c" != "$n" ]]; then
    echo "version mismatch: Cargo.toml says $c, package.json says $n" >&2
    echo "run ./scripts/set-version.sh $c" >&2
    exit 1
  fi
  echo "version $c"
  exit 0
fi

VERSION="${1:-}"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 <major.minor.patch> | --check" >&2
  exit 1
fi

# Section by section, never a global substitution: `version = "0.0.0"` also
# appears on the third-party dependencies, where rewriting it would pin every
# crate in the tree to Tome's release number.
python3 - "$VERSION" <<'PY'
import re, sys
version = sys.argv[1]

cargo = open('Cargo.toml').read()
start = cargo.index('[workspace.package]')
end = cargo.index('[', start + 1)
section = cargo[start:end]
section, n = re.subn(r'^version = ".*"$', f'version = "{version}"', section, count=1, flags=re.M)
if n != 1:
    raise SystemExit('no version line under [workspace.package]')
cargo = cargo[:start] + section + cargo[end:]

# The path dependencies on our own crates carry a version requirement too, and
# it has to move with the package version or the workspace stops resolving:
# `failed to select a version for the requirement tome-testkit = "^0.0.0"`.
for crate in ('tome-core', 'tome-testkit'):
    cargo, n = re.subn(
        rf'^({re.escape(crate)} = \{{ path = "[^"]+", version = )"[^"]*"',
        rf'\g<1>"{version}"',
        cargo, count=1, flags=re.M)
    if n != 1:
        raise SystemExit(f'no workspace dependency line for {crate}')

open('Cargo.toml', 'w').write(cargo)

pkg = open('package.json').read()
pkg, n = re.subn(r'^  "version": ".*",$', f'  "version": "{version}",', pkg, count=1, flags=re.M)
if n != 1:
    raise SystemExit('no version line in package.json')
open('package.json', 'w').write(pkg)
PY

# Refresh Cargo.lock, or the next build does it and leaves the tree dirty
# after the release commit.
cargo metadata --format-version 1 >/dev/null

echo "version set to $VERSION"
