# Swift Shell

Native macOS shell providing system integration.

## What Belongs Here

- **Menu bar** presence and controls
- **System notifications** for sync status
- **Global keyboard shortcuts** registration
- **Native dialogs** and system UI integration

## What Does NOT Belong Here

- UI rendering (use Svelte in src/)
- Business logic (use Rust in src-tauri/)
- Data storage (use Rust storage module)

## Building

```bash
# Build the package
swift build

# Run tests
swift test

# Build for release
swift build -c release
```

## Integration with Tauri

The Swift shell is optional and provides enhanced macOS integration.
The main Tauri application works without it.

When present, the shell is loaded via the Tauri plugin system.

## Linting

```bash
# Run SwiftLint
swiftlint

# Auto-fix issues
swiftlint --fix
```
