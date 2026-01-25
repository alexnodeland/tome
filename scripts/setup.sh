#!/bin/bash
# Tome Development Setup Script
#
# This script sets up a fresh development environment.
# Target: Complete setup in under 5 minutes.
#
# Usage: ./scripts/setup.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🚀 Tome Development Setup"
echo "========================="
echo ""

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to print status
status() {
    echo -e "${GREEN}✓${NC} $1"
}

warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1"
    exit 1
}

# === Check Prerequisites ===

echo "Checking prerequisites..."

# Check macOS
if [[ "$(uname)" != "Darwin" ]]; then
    error "This project requires macOS"
fi

# Check Apple Silicon
if [[ "$(uname -m)" != "arm64" ]]; then
    warning "This project targets Apple Silicon. Performance may vary on Intel."
fi

# Check Xcode Command Line Tools
if ! xcode-select -p &>/dev/null; then
    echo "Installing Xcode Command Line Tools..."
    xcode-select --install
    echo "Please complete the installation and re-run this script."
    exit 1
fi
status "Xcode Command Line Tools installed"

# Check Homebrew (optional but recommended)
if command_exists brew; then
    status "Homebrew installed"
else
    warning "Homebrew not found. Some dependencies may need manual installation."
fi

# === Install Rust ===

if command_exists rustc; then
    RUST_VERSION=$(rustc --version | cut -d' ' -f2)
    status "Rust installed (${RUST_VERSION})"
else
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    status "Rust installed"
fi

# Ensure Rust toolchain is up to date
echo "Updating Rust toolchain..."
rustup update stable >/dev/null 2>&1
rustup default stable

# Install required components
rustup component add rustfmt clippy >/dev/null 2>&1
status "Rust components installed (rustfmt, clippy)"

# Add aarch64 target
rustup target add aarch64-apple-darwin >/dev/null 2>&1
status "Rust target added (aarch64-apple-darwin)"

# === Install Node.js ===

if command_exists node; then
    NODE_VERSION=$(node --version)
    status "Node.js installed (${NODE_VERSION})"

    # Check version is 20+
    NODE_MAJOR=$(echo "$NODE_VERSION" | cut -d'.' -f1 | tr -d 'v')
    if [[ "$NODE_MAJOR" -lt 20 ]]; then
        warning "Node.js 20+ recommended. Current: ${NODE_VERSION}"
    fi
else
    echo "Node.js not found. Please install Node.js 20+:"
    echo "  brew install node@20"
    echo "  or download from https://nodejs.org/"
    exit 1
fi

# === Install Tauri CLI ===

if ! command_exists cargo-tauri 2>/dev/null; then
    echo "Installing Tauri CLI..."
    cargo install tauri-cli >/dev/null 2>&1
    status "Tauri CLI installed"
else
    status "Tauri CLI already installed"
fi

# === Install npm dependencies ===

echo "Installing npm dependencies..."
npm ci
status "npm dependencies installed"

# === Build Rust project ===

echo "Building Rust project (this may take a few minutes)..."
cd src-tauri
cargo build 2>&1 | tail -1
cd ..
status "Rust project built"

# === Setup Git Hooks ===

echo "Setting up Git hooks..."
npm run prepare 2>/dev/null || npx husky install 2>/dev/null || true

# Make husky hooks executable
if [ -d ".husky" ]; then
    chmod +x .husky/pre-commit 2>/dev/null || true
    chmod +x .husky/pre-push 2>/dev/null || true
fi
status "Git hooks configured"

# === Install pre-commit (optional) ===

if command_exists pip3; then
    if ! command_exists pre-commit; then
        pip3 install pre-commit >/dev/null 2>&1 || true
    fi
    if command_exists pre-commit; then
        pre-commit install >/dev/null 2>&1 || true
        status "pre-commit hooks installed"
    fi
fi

# === Run Initial Checks ===

echo ""
echo "Running initial checks..."

# Format check
if npm run format:check >/dev/null 2>&1; then
    status "Code formatting valid"
else
    warning "Some files need formatting. Run 'npm run format' to fix."
fi

# Lint check (may fail on empty project)
npm run lint >/dev/null 2>&1 && status "Linting passed" || warning "Some lint issues found. Run 'npm run lint:fix' to fix."

# Type check
npm run typecheck >/dev/null 2>&1 && status "Type checking passed" || warning "Some type issues found."

# === Print Summary ===

echo ""
echo "=============================="
echo -e "${GREEN}✅ Setup complete!${NC}"
echo "=============================="
echo ""
echo "Quick start commands:"
echo "  npm run dev        - Start development server"
echo "  npm run build      - Build for production"
echo "  npm run check      - Run all checks"
echo "  npm test           - Run tests"
echo ""
echo "Useful resources:"
echo "  CLAUDE.md          - Project documentation"
echo "  .claude/plans/     - Project planning documents"
echo ""
echo "Happy coding! 🎉"
