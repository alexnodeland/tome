# CI/CD & DevOps

**Platform:** GitHub Actions
**Strategy:** Trunk-based development
**Release:** On-demand

> **What this document claimed versus what its workflows did.** `11-risk-register.md` RISK-008 and
> `12-security-considerations.md` both list `cargo audit`, `npm audit`, and Dependabot as active CI
> mitigations. None of them existed in the workflows below. A mitigation recorded in a risk
> register but absent from CI is worse than an acknowledged gap, because the risk reads as handled.
> Added below, along with token permissions, action pinning, and a fix to the release process,
> which as written could not run against the branch protection this same document specifies.

---

## Branching Strategy

### Trunk-Based Development

```
main (trunk)
  │
  ├── feature/add-rustdoc-scraper     (short-lived, < 2 days)
  │     └── PR → main
  │
  ├── feature/search-ui               (short-lived)
  │     └── PR → main
  │
  └── release/v1.0.0                  (cut from main for release)
        └── hotfix if needed
```

### Branch Rules

| Branch | Purpose | Lifetime |
|--------|---------|----------|
| `main` | Trunk, always deployable | Permanent |
| `feature/*` | Feature development | < 2 days recommended |
| `fix/*` | Bug fixes | < 1 day |
| `release/*` | Release preparation | Until released |
| `hotfix/*` | Critical production fixes | < 1 day |

### Branch Protection (main)

```yaml
# Repository settings
branches:
  main:
    protection:
      required_reviews: 1
      require_code_owner_review: true
      required_status_checks:
        - lint
        - audit          # cargo audit / cargo deny / npm audit / secret scan
        - test-rust
        - test-js
        - test-integration
        - eval           # search relevance + platform detection
        - build
      dismiss_stale_reviews: true
      require_linear_history: true
```

---

## Code Ownership

### CODEOWNERS

```
# .github/CODEOWNERS

# Default owners for everything
* @maintainer

# Rust core
/src-tauri/ @maintainer

# Frontend
/src/ @maintainer

# CI/CD
/.github/ @maintainer

# Documentation
/docs/ @maintainer
```

> Planning documents moved from `.claude/plans/` to `docs/plans/`. A tool-specific dotfile
> directory is invisible in the GitHub UI and excluded by most documentation tooling — the wrong
> home for the primary specification of the project. This document's own table already pointed at
> `/docs/`.

---

## Git Hooks

### Pre-commit Hooks

Using [pre-commit](https://pre-commit.com/) or [husky](https://typicode.github.io/husky/):

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      # Rust
      - id: cargo-fmt
        name: Rust Format
        entry: cargo fmt --check
        language: system
        types: [rust]
        pass_filenames: false

      - id: cargo-clippy
        name: Rust Lint
        entry: cargo clippy --all-targets --all-features -- -D warnings
        language: system
        types: [rust]
        pass_filenames: false

      # JavaScript/TypeScript
      - id: eslint
        name: ESLint
        entry: npm run lint
        language: system
        types: [javascript, typescript, svelte]
        pass_filenames: false

      - id: prettier
        name: Prettier
        entry: npx prettier --check .
        language: system
        types: [javascript, typescript, json, yaml, markdown, svelte]
        pass_filenames: false

      # General
      - id: no-secrets
        name: Check for secrets
        entry: git secrets --scan
        language: system
```

### Commit Message Convention

```
# commitlint.config.js
module.exports = {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [2, 'always', [
      'feat',     // New feature
      'fix',      // Bug fix
      'docs',     // Documentation
      'style',    // Formatting
      'refactor', // Refactoring
      'perf',     // Performance
      'test',     // Tests
      'build',    // Build system
      'ci',       // CI configuration
      'chore',    // Maintenance
    ]],
    'subject-case': [2, 'always', 'lower-case'],
    'subject-max-length': [2, 'always', 72],
  },
};
```

---

## Claude Code Hooks

> **The configuration format below was invented.** Claude Code reads hooks from
> `.claude/settings.json` with event names and matchers — not from a `.claude/config.yaml` with an
> `enabled:`/`script:` shape. As written, none of these hooks would have run. Corrected below.

Hooks are a convenience, not a quality gate: CI is the gate. Keep them fast, and keep anything
slow out of them.

```json
// .claude/settings.json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          { "type": "command", "command": ".claude/hooks/format.sh", "timeout": 30 }
        ]
      }
    ]
  }
}
```

```bash
#!/usr/bin/env bash
# .claude/hooks/format.sh -- fast formatting only
set -uo pipefail
cargo fmt --quiet 2>/dev/null || true
npm run format --silent 2>/dev/null || true
```

**Do not run the test suite in a Stop hook.** The original did, with a 120-second timeout, on a
project whose own testing document budgets "full suite in < 5 minutes" — so it would time out and
be ignored. Run tests deliberately, and let CI enforce them.

---

## GitHub Actions Workflows

### CI Pipeline (on PR and push to main)

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

# Least privilege by default. Individual jobs opt in to more.
# The original workflows had no `permissions` block at all, which means the
# default token -- with write access to the repository -- was available to
# every third-party action in every job.
permissions:
  contents: read

jobs:
  lint:
    name: Lint
    # Linting needs no macOS. GitHub bills macOS runners at 10x Linux; running
    # every job on macos-14 was a large, invisible cost for a project whose
    # funding model is undecided (DEC-003).
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Rust Format
        run: cargo fmt --check

      - name: Rust Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Install JS dependencies
        run: npm ci

      - name: ESLint
        run: npm run lint

      - name: Prettier
        run: npm run format:check

  audit:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: cargo audit
        uses: rustsec/audit-check@v2
        with:
          token: ${{ secrets.GITHUB_TOKEN }}

      - name: cargo deny (licences + duplicate/banned deps)
        uses: EmbarkStudios/cargo-deny-action@v2

      - name: npm audit
        run: npm audit --audit-level=high

      - name: Secret scan
        uses: gitleaks/gitleaks-action@v2

  test-rust:
    name: Test Rust
    # Tome is macOS-only, so the Rust tests do need a macOS runner.
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Install cargo-llvm-cov
        uses: taiki-e/install-action@cargo-llvm-cov

      - name: Run tests with coverage
        run: cargo llvm-cov --all-features --lcov --output-path lcov-rust.info

      - name: Check coverage threshold
        run: cargo llvm-cov --fail-under 90

      - name: Upload coverage
        uses: codecov/codecov-action@v4
        with:
          files: lcov-rust.info
          flags: rust

  test-js:
    name: Test JavaScript
    # jsdom-based; no macOS needed.
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Run tests with coverage
        run: npm test -- --coverage --coverageReporters=lcov

      - name: Check coverage threshold
        run: |
          npm test -- --coverage --coverageThreshold='{"global":{"lines":90,"branches":90,"functions":90}}'

      - name: Upload coverage
        uses: codecov/codecov-action@v4
        with:
          files: coverage/lcov.info
          flags: javascript

  # The Playwright E2E job has been removed. `tauri-driver` does not support
  # macOS, so Playwright cannot drive Tome; the old job installed Playwright's
  # own WebKit build and would have reported green while testing nothing.
  # See 08-testing-strategy.md "End-to-End Testing".
  test-integration:
    name: Backend Integration (Tier B)
    runs-on: macos-14
    needs: [test-rust]
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Run integration tests against fixture servers
        run: cargo test --test '*' -- --include-ignored

      - name: Upload failure artifacts
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: integration-artifacts
          path: target/test-output/

  eval:
    name: Search Relevance + Detection Eval
    runs-on: ubuntu-latest
    needs: [test-rust]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      # P2-019 / P2-020. Offline, deterministic, against committed fixtures.
      # These are the gates that keep search quality from silently regressing.
      - name: Relevance eval
        run: cargo run --bin eval -- relevance --min-recall3 0.90

      - name: Detection eval
        run: cargo run --bin eval -- detection --min-accuracy 0.95

  build:
    name: Build
    runs-on: macos-14
    needs: [lint]
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Install dependencies
        run: npm ci

      - name: Build
        run: npm run tauri build -- --target aarch64-apple-darwin

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: tome-unsigned
          path: src-tauri/target/aarch64-apple-darwin/release/bundle/
```

### Release Pipeline

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: read

jobs:
  build-and-sign:
    name: Build and Sign
    runs-on: macos-14
    permissions:
      contents: write   # only this job creates the release
    # Guard: a tag pushed from a branch that is not an ancestor of main would
    # publish code that never passed review. The release process below keeps
    # tags on main; this enforces it.
    steps:
      - name: Verify tag is on main
        run: |
          git fetch origin main --depth=100
          git merge-base --is-ancestor "$GITHUB_SHA" origin/main \
            || { echo "::error::Tag is not an ancestor of main"; exit 1; }

      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Install dependencies
        run: npm ci

      - name: Import signing certificate
        env:
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
        run: |
          set -euo pipefail
          # Quote the variable: an unquoted `echo $VAR` mangles base64 that
          # contains whitespace, and the failure is a confusing signing error.
          echo "$APPLE_CERTIFICATE" | base64 --decode > certificate.p12
          security create-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
          security default-keychain -s build.keychain
          # Keep the login keychain in the search list, or later tooling that
          # expects it will fail in ways that look unrelated.
          security list-keychains -d user -s build.keychain login.keychain-db
          security unlock-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
          security set-keychain-settings -t 3600 -u build.keychain
          security import certificate.p12 -k build.keychain \
            -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
          security set-key-partition-list -S apple-tool:,apple:,codesign: \
            -s -k "$KEYCHAIN_PASSWORD" build.keychain
          # The p12 must not outlive its use, even on an ephemeral runner.
          rm -f certificate.p12

      - name: Clean up keychain
        if: always()
        run: security delete-keychain build.keychain || true

      - name: Build and sign
        env:
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
        run: |
          npm run tauri build -- --target aarch64-apple-darwin

      - name: Notarize
        env:
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_APP_PASSWORD: ${{ secrets.APPLE_APP_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        run: |
          APP_PATH="src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Tome.app"

          # Create zip for notarization
          ditto -c -k --keepParent "$APP_PATH" Tome.zip

          # Submit for notarization
          xcrun notarytool submit Tome.zip \
            --apple-id "$APPLE_ID" \
            --password "$APPLE_APP_PASSWORD" \
            --team-id "$APPLE_TEAM_ID" \
            --wait

          # Staple
          xcrun stapler staple "$APP_PATH"

      - name: Create DMG
        run: |
          npm install -g appdmg
          appdmg dmg-config.json Tome-${{ github.ref_name }}.dmg

      - name: Notarize DMG
        env:
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_APP_PASSWORD: ${{ secrets.APPLE_APP_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        run: |
          xcrun notarytool submit Tome-${{ github.ref_name }}.dmg \
            --apple-id "$APPLE_ID" \
            --password "$APPLE_APP_PASSWORD" \
            --team-id "$APPLE_TEAM_ID" \
            --wait
          xcrun stapler staple Tome-${{ github.ref_name }}.dmg

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: Tome-${{ github.ref_name }}.dmg
          generate_release_notes: true
          draft: true

  update-homebrew:
    name: Update Homebrew Cask
    runs-on: ubuntu-latest
    needs: build-and-sign
    steps:
      - name: Update Homebrew Cask
        env:
          HOMEBREW_TAP_TOKEN: ${{ secrets.HOMEBREW_TAP_TOKEN }}
        run: |
          # Calculate SHA256 of DMG from release
          VERSION="${{ github.ref_name }}"
          DMG_URL="https://github.com/${{ github.repository }}/releases/download/${VERSION}/Tome-${VERSION}.dmg"
          SHA256=$(curl -sL "$DMG_URL" | shasum -a 256 | cut -d' ' -f1)

          # Update cask formula (in your homebrew tap repo)
          # This is a simplified example - actual implementation depends on your tap setup
```

### Dependency Updates

**Use Dependabot, not a `cargo update` cron.** The plan previously specified *both* — Dependabot in
`16-support-maintenance.md` and this scheduled workflow — which produce competing PRs against the
same lockfiles. It also stated three different review cadences (weekly here, weekly in `16`,
monthly in `09-non-functional-requirements.md`). One mechanism, one cadence: **Dependabot,
weekly.**

```yaml
# .github/dependabot.yml  -- this replaces the cron workflow below
version: 2
updates:
  - package-ecosystem: cargo
    directory: /src-tauri
    schedule: { interval: weekly }
    open-pull-requests-limit: 5
    groups:
      patch-and-minor:
        update-types: [patch, minor]   # batch the noise, review majors alone
  - package-ecosystem: npm
    directory: /
    schedule: { interval: weekly }
    open-pull-requests-limit: 5
    groups:
      patch-and-minor:
        update-types: [patch, minor]
  - package-ecosystem: github-actions   # actions drift too, and they hold secrets
    directory: /
    schedule: { interval: weekly }
```

<details>
<summary>Superseded: scheduled <code>cargo update</code> workflow (kept for reference)</summary>

```yaml
# .github/workflows/dependencies.yml  -- SUPERSEDED by dependabot.yml above
name: Update Dependencies

on:
  schedule:
    - cron: '0 9 * * 1'  # Every Monday at 9 AM
  workflow_dispatch:

jobs:
  update-rust:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Update Cargo dependencies
        run: cargo update

      - name: Run tests
        run: cargo test

      - name: Create PR
        uses: peter-evans/create-pull-request@v5
        with:
          title: 'chore: update Rust dependencies'
          body: 'Automated dependency update'
          branch: deps/rust-update
          commit-message: 'chore: update Rust dependencies'

  update-npm:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Update npm dependencies
        run: npm update

      - name: Run tests
        run: npm test

      - name: Create PR
        uses: peter-evans/create-pull-request@v5
        with:
          title: 'chore: update npm dependencies'
          body: 'Automated dependency update'
          branch: deps/npm-update
          commit-message: 'chore: update npm dependencies'
```

</details>

---

## Required Secrets

Configure these in GitHub repository settings:

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE` | Base64 encoded .p12 certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Certificate password |
| `APPLE_SIGNING_IDENTITY` | "Developer ID Application: Name (TEAMID)" |
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_APP_PASSWORD` | App-specific password |
| `APPLE_TEAM_ID` | Apple Developer Team ID |
| `KEYCHAIN_PASSWORD` | Temporary keychain password |
| `HOMEBREW_TAP_TOKEN` | PAT for updating Homebrew tap |
| `CODECOV_TOKEN` | Codecov upload token |

### Secret handling rules

- **Only the release workflow may access signing secrets.** CI on pull requests must not — a PR
  from a fork that could read `APPLE_CERTIFICATE` is a supply-chain compromise of every future
  release.
- **Pin third-party actions to a commit SHA**, not a tag. Tags are mutable; a compromised action
  in a job with `contents: write` and signing secrets is the highest-value target in this
  repository. Dependabot's `github-actions` ecosystem keeps the pins current.
- Set `permissions:` explicitly in every workflow (done above). The default is broad.
- Never `echo` a secret, even into a file, without quoting — and delete the file afterwards.

---

## Release Process

### Creating a Release

> **The original procedure could not run.** It ended with `git push origin main --tags`, a direct
> push to a branch this same document protects with required reviews, required status checks, and
> linear history. GitHub rejects it. Version bumps go through a PR like everything else.

```bash
# 1. Branch from an up-to-date main
git checkout main && git pull
git checkout -b release/v1.0.0

# 2. Bump the version everywhere it appears
#    Cargo.toml, package.json, tauri.conf.json, the Homebrew cask
#    -- one script, so they cannot drift:
./scripts/set-version.sh 1.0.0

# 3. Update CHANGELOG.md

# 4. Open a PR; CI runs; get the review that branch protection requires
git commit -am "chore: release v1.0.0"
git push -u origin release/v1.0.0
gh pr create --fill

# 5. Merge the PR (squash; linear history is required)

# 6. Tag the merge commit ON MAIN -- the release workflow verifies the tag is
#    an ancestor of main and fails otherwise
git checkout main && git pull
git tag -s v1.0.0 -m "Tome 1.0.0"     # signed: the tag drives a signed build
git push origin v1.0.0

# 7. CI builds, signs, notarizes, creates the DMG, opens a draft release,
#    and updates the tap
# 8. Verify the artifact on a clean machine, then publish the draft
```

**Verify before publishing.** Download the DMG on a machine that has never built Tome and check
`spctl --assess --type execute /Applications/Tome.app`. A build that is signed but not correctly
stapled passes on the build machine and fails for every user — this is the single most common way
a macOS release goes wrong, and only a clean machine catches it.

### Hotfix Process

```bash
# 1. Branch from the released tag
git checkout -b hotfix/v1.0.1 v1.0.0

# 2. Make the minimal fix, bump the patch version, update CHANGELOG
./scripts/set-version.sh 1.0.1
git commit -am "fix: <critical bug>"

# 3. PR the hotfix branch into main. Even under time pressure this is a PR:
#    the review requirement exists precisely for changes made under pressure.
git push -u origin hotfix/v1.0.1
gh pr create --fill --label hotfix

# 4. Merge, then tag on main
git checkout main && git pull
git tag -s v1.0.1 -m "Tome 1.0.1"
git push origin v1.0.1
```

If the hotfix cannot go through main because main has already moved on incompatibly, that is a
*release branch*, and it needs its own protection rules — not a bypass of the existing ones.

---

## Environment Setup

### Development Environment

```bash
# Prerequisites
# - macOS 12+ on Apple Silicon
# - Xcode Command Line Tools
# - Homebrew

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node
brew install node@20

# Clone and setup
git clone https://github.com/yourname/tome.git
cd tome
npm install
cargo build

# Install pre-commit hooks
npm run prepare  # or: pre-commit install

# Run development server
npm run tauri dev
```

### CI Environment (GitHub Actions)

- **Runner:** `macos-14` (M1 Apple Silicon)
- **Rust:** Latest stable via `dtolnay/rust-toolchain`
- **Node:** v20 LTS via `actions/setup-node`
- **Caching:** Cargo cache via `Swatinem/rust-cache`
