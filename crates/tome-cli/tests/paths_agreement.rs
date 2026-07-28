//! Cross-binary path agreement.
//!
//! `tome-core` already unit-tests that `Paths::resolve()` is deterministic. That
//! is necessary but not sufficient: the invariant that matters is that the
//! **separately compiled `tome` binary** resolves the same library as the code
//! the desktop app links. If they ever diverge, bookmarks added in the app are
//! invisible to the CLI and to the MCP server, and the integration story that
//! Phase 4 exists to deliver breaks silently.
//!
//! This test runs the real binary and compares its output to the library's own
//! resolution. See `docs/decisions/0002-no-app-sandbox.md`.

// The workspace denies `expect`/`unwrap` so that panicking paths cannot reach
// production code. In a test, panicking on setup failure IS the correct
// behaviour -- a failed tempdir or a missing binary is a broken test run, not a
// condition to handle.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::process::Command;

use tome_core::Paths;

/// Path to the `tome` binary Cargo just built for this test run.
fn tome_bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().expect("test binary has a path");
    p.pop(); // deps/
    p.pop(); // debug/ or release/
    p.push("tome");
    p
}

fn status_with_home(home: &std::path::Path) -> String {
    let out = Command::new(tome_bin())
        .arg("status")
        .env("TOME_HOME", home)
        .output()
        .expect("`tome status` runs");

    assert!(
        out.status.success(),
        "`tome status` failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    String::from_utf8(out.stdout).expect("stdout is UTF-8")
}

#[test]
fn cli_binary_resolves_the_same_paths_as_the_library() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let expected = Paths::under_root(tmp.path());

    let stdout = status_with_home(tmp.path());

    for (label, path) in [
        ("state", expected.state_root().to_path_buf()),
        ("cache", expected.cache_root().to_path_buf()),
        ("db", expected.database_file()),
        ("index", expected.index_dir()),
    ] {
        let display = path.display().to_string();
        assert!(
            stdout.contains(&display),
            "`tome status` did not report the expected {label} path.\n\
             expected to find: {display}\n\
             actual output:\n{stdout}"
        );
    }
}

#[test]
fn cli_reports_the_shared_bundle_identifier() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let stdout = status_with_home(tmp.path());

    // One constant, used by the app bundle, the Keychain service, the iCloud
    // container, and the Homebrew zap list. See ADR-0004.
    assert!(
        stdout.contains(tome_core::BUNDLE_ID),
        "expected bundle id {} in:\n{stdout}",
        tome_core::BUNDLE_ID
    );
}

#[test]
fn unimplemented_commands_fail_loudly() {
    // A scaffold that exits 0 on an unimplemented command is worse than one that
    // errors: scripts and CI would treat it as success.
    let out = Command::new(tome_bin())
        .arg("list")
        .output()
        .expect("`tome list` runs");

    assert!(
        !out.status.success(),
        "unimplemented command must exit non-zero"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("not implemented"),
        "error should say what is missing"
    );
}
