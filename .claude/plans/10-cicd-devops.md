# CI/CD & DevOps

**Platform:** GitHub Actions
**Strategy:** Trunk-based development
**Release:** On-demand

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
        - test-rust
        - test-js
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
/.claude/plans/ @maintainer
```

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

### PostToolUse Hook

Runs after Claude Code makes changes:

```bash
#!/bin/bash
# .claude/hooks/post-tool-use.sh

# Format code
cargo fmt --check 2>/dev/null || cargo fmt
npm run format 2>/dev/null || true

# Quick lint check
cargo clippy --quiet 2>/dev/null
npm run lint --quiet 2>/dev/null
```

### Stop Hook

Runs when Claude Code session ends:

```bash
#!/bin/bash
# .claude/hooks/stop.sh

# Run full test suite
echo "Running tests before commit..."
cargo test --quiet
npm test --silent

# Check for uncommitted changes
if [[ -n $(git status --porcelain) ]]; then
  echo "Warning: Uncommitted changes detected"
  git status --short
fi
```

### Configuration

```yaml
# .claude/config.yaml
hooks:
  postToolUse:
    enabled: true
    script: .claude/hooks/post-tool-use.sh
    timeout: 30s
  stop:
    enabled: true
    script: .claude/hooks/stop.sh
    timeout: 120s
```

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

jobs:
  lint:
    name: Lint
    runs-on: macos-14  # M1 runner
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

  test-rust:
    name: Test Rust
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
    runs-on: macos-14
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

  test-e2e:
    name: E2E Tests
    runs-on: macos-14
    needs: [test-rust, test-js]
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Install dependencies
        run: npm ci

      - name: Install Playwright
        run: npx playwright install --with-deps webkit

      - name: Build app
        run: npm run build

      - name: Run E2E tests
        run: npx playwright test

      - name: Upload test results
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: playwright-report
          path: playwright-report/

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

jobs:
  build-and-sign:
    name: Build and Sign
    runs-on: macos-14
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

      - name: Import signing certificate
        env:
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
        run: |
          echo $APPLE_CERTIFICATE | base64 --decode > certificate.p12
          security create-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
          security default-keychain -s build.keychain
          security unlock-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
          security import certificate.p12 -k build.keychain -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
          security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASSWORD" build.keychain

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
        uses: softprops/action-gh-release@v1
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

```yaml
# .github/workflows/dependencies.yml
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

---

## Release Process

### Creating a Release

```bash
# 1. Ensure main is up to date
git checkout main
git pull

# 2. Run final checks
cargo test
npm test
npm run build

# 3. Update version
# Edit Cargo.toml, package.json, tauri.conf.json

# 4. Commit version bump
git add -A
git commit -m "chore: bump version to 1.0.0"

# 5. Create and push tag
git tag v1.0.0
git push origin main --tags

# 6. GitHub Actions will:
#    - Build and sign
#    - Notarize
#    - Create DMG
#    - Create draft release
#    - Update Homebrew cask

# 7. Review and publish draft release on GitHub
```

### Hotfix Process

```bash
# 1. Create hotfix branch from release tag
git checkout -b hotfix/v1.0.1 v1.0.0

# 2. Make fix
# ...

# 3. Commit and tag
git commit -m "fix: critical bug"
git tag v1.0.1
git push origin hotfix/v1.0.1 --tags

# 4. After release, merge back to main
git checkout main
git merge hotfix/v1.0.1
git push
```

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
