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
    // errors: scripts and CI would treat it as success. (`list` was the subject
    // first, then `search`; S1-13 and S2-3 implemented them in turn, so this
    // moves to one that is still a stub rather than being deleted.)
    let out = Command::new(tome_bin())
        .arg("remove")
        .arg("anything")
        .output()
        .expect("`tome remove` runs");

    assert!(
        !out.status.success(),
        "unimplemented command must exit non-zero"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("not implemented"),
        "error should say what is missing"
    );
}

#[test]
fn search_on_an_empty_library_succeeds_with_no_results() {
    // `search` is read-only, so on a machine that has pulled nothing it must
    // print "no results" and exit 0 — not error, and not create the library.
    // A non-zero exit here would make `tome search … || echo none` misreport an
    // empty library as a failure.
    let home = tempfile::tempdir().expect("tempdir");
    let out = Command::new(tome_bin())
        .arg("search")
        .arg("anything")
        .env("TOME_HOME", home.path())
        .output()
        .expect("`tome search` runs");

    assert!(
        out.status.success(),
        "search on an empty library must exit zero:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !home.path().join("state").exists(),
        "a read-only command must not create the library"
    );
}

#[test]
fn list_says_where_sources_go_when_there_are_none() {
    // An empty library must not print nothing. The one thing someone in this
    // state needs is the directory to put a configuration in.
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::new(tome_bin())
        .arg("list")
        .env("TOME_HOME", tmp.path())
        .output()
        .expect("`tome list` runs");

    assert!(out.status.success(), "an empty library is not an error");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("sources"), "{stdout}");
}

#[test]
fn list_json_has_one_shape_even_when_empty() {
    // `tome list --json | jq` must not need an empty-library special case.
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::new(tome_bin())
        .arg("list")
        .arg("--json")
        .env("TOME_HOME", tmp.path())
        .output()
        .expect("`tome list --json` runs");

    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        r#"{"sources":[]}"#
    );
}

#[test]
fn pull_without_a_source_says_which_are_available() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::new(tome_bin())
        .arg("pull")
        .arg("nothing-here")
        .env("TOME_HOME", tmp.path())
        .output()
        .expect("`tome pull` runs");

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Names the directory, so the next step is obvious rather than guessed.
    assert!(stderr.contains("sources"), "{stderr}");
}

#[test]
fn search_reports_the_typo_it_corrected() {
    // End-to-end through the real binary, because the JSON shape is a
    // contract: `tome search --json | jq '.suggestions'` must work, and must
    // work on a query with no typo in it too. Building the index through
    // `tome-core` here rather than crawling keeps the test offline.
    use tome_core::model::{ContentHash, Node, Page, PagePath, SourceId};

    let home = tempfile::tempdir().expect("tempdir");
    let paths = Paths::under_root(home.path());
    paths.ensure_created().expect("create library");

    {
        let engine = tome_core::search::SearchEngine::open(&paths).expect("open index");
        let mut session = engine.session().expect("session");
        session
            .add_page(
                &Page::new(
                    SourceId::new("cargo").expect("source id"),
                    PagePath::new("reference/environment-variables.html").expect("page path"),
                    "Environment variables",
                    ContentHash::new("0".repeat(64)).expect("hash"),
                ),
                "Rust",
                &Node::Document {
                    children: vec![Node::Paragraph {
                        children: vec![Node::Text {
                            value: "Cargo reads a number of environment variables, and each \
                                    environment variable is documented here."
                                .to_owned(),
                        }],
                    }],
                },
            )
            .expect("add page");
        session.commit().expect("commit");
    }

    let run = |query: &str| {
        let out = Command::new(tome_bin())
            .args(["search", query, "--json"])
            .env("TOME_HOME", home.path())
            .output()
            .expect("`tome search` runs");
        assert!(
            out.status.success(),
            "search failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
        value
    };

    let corrected = run("enviroment");
    assert_eq!(
        corrected["suggestions"],
        serde_json::json!([{ "typed": "enviroment", "meant": "environment" }]),
        "a misspelling should be reported, not silently corrected"
    );
    assert_eq!(
        corrected["results"][0]["path"], "reference/environment-variables.html",
        "and it should still find the page"
    );

    // Always present, never null: `jq '.suggestions[]'` must not need an
    // empty-query special case.
    let clean = run("environment");
    assert_eq!(clean["suggestions"], serde_json::json!([]));
    assert_eq!(
        clean["results"][0]["path"],
        "reference/environment-variables.html"
    );
}
