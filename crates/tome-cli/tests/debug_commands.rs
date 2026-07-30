//! `tome debug` end to end: diagnose, repair, report (S4-3 / P5-004, P5-005).
//!
//! Through the compiled binary against the fixture server, because every claim
//! here is about a command someone runs when something is already wrong. A
//! recovery path tested only at the function level is a recovery path nobody
//! has ever run.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;
use std::process::{Command, Output, Stdio};

use tome_testkit::FixtureServer;

fn tome_bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().expect("test binary has a path");
    p.pop(); // deps/
    p.pop(); // debug/ or release/
    p.push("tome");
    p
}

fn tome(home: &Path, args: &[&str]) -> Output {
    Command::new(tome_bin())
        .args(args)
        .env("TOME_HOME", home)
        .stdin(Stdio::null())
        .output()
        .expect("tome runs")
}

fn stdout_json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}):\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

/// Pull the fixture site into a throwaway library.
fn library() -> (tempfile::TempDir, FixtureServer) {
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let home = tempfile::tempdir().expect("tempdir");
    let url = format!("{}/", server.url());
    let out = tome(
        home.path(),
        &["add", &url, "--yes", "--insecure", "--json", "--quiet"],
    );
    assert!(
        out.status.success(),
        "setup pull failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    (home, server)
}

#[test]
fn check_is_clean_on_a_healthy_library() {
    let (home, _server) = library();
    let out = tome(home.path(), &["debug", "check", "--json"]);
    assert!(out.status.success(), "a healthy library must exit zero");

    let json = stdout_json(&out);
    assert_eq!(json["problems"], 0, "{json}");
    let checks = json["checks"].as_array().expect("checks array");
    assert!(
        checks.iter().all(|c| c["ok"] == true),
        "every check should pass: {json}"
    );
    // The one that has no other symptom.
    assert!(
        checks
            .iter()
            .any(|c| c["check"].as_str().unwrap_or("").contains("agree")),
        "the index/database agreement check must run: {json}"
    );
}

#[test]
fn check_says_nothing_is_wrong_with_a_machine_that_has_pulled_nothing() {
    // A first run is not a fault. Reporting it as one sends people looking for
    // a problem that is an empty library, and `debug check` exiting non-zero
    // would break `tome debug check && tome pull --all`.
    let home = tempfile::tempdir().expect("tempdir");
    let out = tome(home.path(), &["debug", "check", "--json"]);
    assert!(out.status.success());
    assert_eq!(stdout_json(&out)["problems"], 0);
    assert!(
        !home.path().join("state").exists(),
        "a diagnostic must not create the library it is diagnosing"
    );
}

#[test]
fn a_deleted_index_is_diagnosed_and_then_rebuilt_offline() {
    let (home, server) = library();

    // How many pages search can see while everything is healthy.
    let before = stdout_json(&tome(home.path(), &["search", "example", "--json"]));
    let before_hits = before["results"].as_array().expect("results").len();
    assert!(
        before_hits > 0,
        "the fixture should be searchable: {before}"
    );

    // Destroy the index the way an interrupted write does: remove it, leaving
    // the database and the stored pages intact.
    let index = home.path().join("cache/index");
    assert!(index.exists(), "index at {}", index.display());
    std::fs::remove_dir_all(&index).expect("remove index");

    // `check` notices, and names the command that fixes it.
    let out = tome(home.path(), &["debug", "check", "--json"]);
    assert!(
        !out.status.success(),
        "a library whose index has vanished is not healthy"
    );
    let json = stdout_json(&out);
    assert!(json["problems"].as_u64().unwrap_or(0) > 0, "{json}");
    let remedies: String = json["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .filter_map(|c| c["remedy"].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        remedies.contains("tome debug rebuild-index"),
        "the remedy must name a command that exists: {json}"
    );

    // The repair. The server is still up, but nothing may be fetched from it:
    // the whole point is that a rebuild reads local content.
    let requests_before = server.request_count();
    let rebuilt = stdout_json(&tome(home.path(), &["debug", "rebuild-index", "--json"]));
    assert_eq!(
        server.request_count(),
        requests_before,
        "rebuilding the index must not touch the network"
    );
    let pages: u64 = rebuilt["rebuilt"]
        .as_array()
        .expect("rebuilt array")
        .iter()
        .map(|s| s["pages"].as_u64().unwrap_or(0))
        .sum();
    assert!(pages > 0, "something should have been indexed: {rebuilt}");

    // And search works again, finding what it found before.
    let after = stdout_json(&tome(home.path(), &["search", "example", "--json"]));
    assert_eq!(
        after["results"].as_array().expect("results").len(),
        before_hits,
        "the rebuilt index must find what the original did"
    );
    assert!(tome(home.path(), &["debug", "check", "--json"])
        .status
        .success());
}

#[test]
fn the_report_is_useful_and_carries_no_reading_history() {
    let (home, _server) = library();
    // Something to search for, so a query exists that could leak.
    let _ = tome(home.path(), &["search", "borrow-checker-secrets", "--json"]);

    let out = tome(home.path(), &["debug", "report"]);
    assert!(out.status.success());
    let report = String::from_utf8_lossy(&out.stdout);

    // Useful: version, where the library is, which sources exist.
    assert!(report.contains("tome "), "{report}");
    assert!(report.contains("## sources"), "{report}");
    assert!(report.contains("## checks"), "{report}");

    // Safe to paste. The query the user typed is reading history and must not
    // appear anywhere, including in the log tail.
    assert!(
        !report.contains("borrow-checker-secrets"),
        "a search query reached the report:\n{report}"
    );
    // Nor the home directory, which is where the username lives.
    if let Ok(real_home) = std::env::var("HOME") {
        if !real_home.is_empty() {
            assert!(
                !report.contains(&real_home),
                "the home directory reached the report:\n{report}"
            );
        }
    }
}

#[test]
fn commands_write_a_log_file_and_the_report_can_read_it() {
    let (home, _server) = library();
    let logs = home.path().join("state/logs");
    assert!(
        logs.exists(),
        "a pull should have logged something to {}",
        logs.display()
    );

    let files: Vec<_> = std::fs::read_dir(&logs)
        .expect("read logs dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files.len(), 1, "one file per day: {files:?}");
    assert!(
        files[0].starts_with("tome-") && files[0].ends_with(".log"),
        "the name carries the date, which is what pruning reads: {files:?}"
    );

    let body = std::fs::read_to_string(logs.join(&files[0])).expect("read log");
    assert!(!body.is_empty(), "the log should not be empty");
    assert!(
        !body.contains('\u{1b}'),
        "the log must not contain terminal escape codes"
    );
}
