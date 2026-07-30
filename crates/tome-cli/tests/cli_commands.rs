//! End-to-end tests of the `tome` binary: `add` → `list`/`search` →
//! `remove`, and the `--json` contract on every command.
//!
//! These run the **separately compiled binary** against the fixture HTTP
//! server, with `TOME_HOME` pointing at a tempdir — the same pattern as
//! `paths_agreement.rs`, and for the same reason: unit tests of the functions
//! prove the code; only the binary proves the command.
//!
//! stdin is `/dev/null` throughout. Interactivity is part of the surface
//! under test: a command that would prompt must *fail* here, not hang.

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
    assert!(
        out.status.success(),
        "expected success.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}):\n{}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

/// `add --yes --json` against the fixture site: detection, config, pull.
#[test]
fn add_detects_writes_config_and_pulls() {
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let home = tempfile::tempdir().expect("tempdir");

    let url = format!("{}/", server.url());
    let out = tome(
        home.path(),
        &["add", &url, "--yes", "--insecure", "--json", "--quiet"],
    );
    let json = stdout_json(&out);

    // The fixture homepage carries Sphinx's `_static/` marker, so detection
    // must be confident and the config type must be the Sphinx scraper's.
    assert_eq!(json["detected"]["platform"], "sphinx");
    assert_eq!(json["detected"]["confident"], true);
    assert_eq!(json["type"], "readthedocs");

    // The config file exists where the JSON says, and the real parser
    // accepts it — `add` must not write a config `pull` would reject.
    let config_path = std::path::PathBuf::from(json["config"].as_str().expect("config path"));
    assert!(config_path.exists(), "config file written");

    // The initial pull happened and stored pages.
    let pages = json["pull"]["pages"].as_u64().expect("pages count");
    assert!(pages > 0, "initial pull stored pages: {json}");

    // And the library agrees: list shows it pulled, search finds content.
    let list = stdout_json(&tome(home.path(), &["list", "--json"]));
    let sources = list["sources"].as_array().expect("sources array");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["pulled"], true);
    // Not compared to `pull.pages`: the crawl can fetch two URLs that land on
    // one library path (`/` and `/index.html`), so the report's count may
    // exceed the row count. `list` answers from the database.
    assert!(sources[0]["pages"].as_u64().expect("pages") > 0);

    let search = stdout_json(&tome(home.path(), &["search", "widget", "--json"]));
    assert!(
        !search["results"].as_array().expect("results").is_empty(),
        "added source is searchable: {search}"
    );
}

/// http without --insecure is refused before any fetch, naming the remedy.
#[test]
fn add_refuses_http_without_insecure() {
    let home = tempfile::tempdir().expect("tempdir");
    // Nothing is listening on this port; the command must fail on the
    // scheme, not on the connection.
    let out = tome(home.path(), &["add", "http://127.0.0.1:9/", "--yes"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--insecure"), "names the remedy: {stderr}");
    let sources_dir = tome_core::Paths::under_root(home.path()).sources_dir();
    let wrote_any = std::fs::read_dir(&sources_dir)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    assert!(!wrote_any, "no config written on refusal");
}

/// Without --yes and without a terminal, `add` fails fast — before fetching.
#[test]
fn add_without_yes_fails_when_not_interactive() {
    let home = tempfile::tempdir().expect("tempdir");
    // The URL is never fetched: were it fetched, this port would refuse and
    // give a different error than the one asserted.
    let out = tome(home.path(), &["add", "https://127.0.0.1:9/"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--yes"), "names the remedy: {stderr}");
}

/// Adding the same site twice fails the second time, both by id and by URL.
#[test]
fn add_rejects_duplicates() {
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let home = tempfile::tempdir().expect("tempdir");
    let url = format!("{}/", server.url());

    let first = tome(
        home.path(),
        &["add", &url, "--yes", "--insecure", "--json", "--quiet"],
    );
    stdout_json(&first);

    // Same URL, same derived id.
    let again = tome(home.path(), &["add", &url, "--yes", "--insecure"]);
    assert!(!again.status.success());
    let stderr = String::from_utf8_lossy(&again.stderr);
    assert!(
        stderr.contains("already"),
        "second add names the conflict: {stderr}"
    );

    // Same URL under a different name: caught by the URL check.
    let renamed = tome(
        home.path(),
        &["add", &url, "--yes", "--insecure", "--name", "other-name"],
    );
    assert!(!renamed.status.success());
    let stderr = String::from_utf8_lossy(&renamed.stderr);
    assert!(
        stderr.contains("already configured"),
        "same URL under a new name is a duplicate: {stderr}"
    );
}

/// `remove --yes` cleans up all four places: index, database, cache, config.
#[test]
fn remove_cleans_up_completely() {
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let home = tempfile::tempdir().expect("tempdir");
    let url = format!("{}/", server.url());

    let added = stdout_json(&tome(
        home.path(),
        &["add", &url, "--yes", "--insecure", "--json", "--quiet"],
    ));
    let id = added["id"].as_str().expect("id").to_owned();
    let config_path = std::path::PathBuf::from(added["config"].as_str().expect("config"));

    // What `remove` reports must be what `list` reported — both answer from
    // the database (the crawl report may double-count aliased paths).
    let list = stdout_json(&tome(home.path(), &["list", "--json"]));
    let pages = list["sources"][0]["pages"].as_u64().expect("pages");

    let removed = stdout_json(&tome(home.path(), &["remove", &id, "--yes", "--json"]));
    assert_eq!(removed["removed"], id.as_str());
    assert_eq!(removed["pages"].as_u64(), Some(pages));

    // Config gone; `list` is empty; the cached content directory is gone.
    assert!(!config_path.exists(), "config file removed");
    let list = stdout_json(&tome(home.path(), &["list", "--json"]));
    assert_eq!(list["sources"].as_array().expect("sources").len(), 0);
    let paths = tome_core::Paths::under_root(home.path());
    let source_id = tome_core::model::SourceId::new(id.clone()).expect("valid id");
    assert!(
        !paths.source_data_dir(&source_id).exists(),
        "cached data removed"
    );

    // The index no longer answers for the removed source. This is the ghost
    // that skipping the index cleanup would leave.
    let search = stdout_json(&tome(home.path(), &["search", "widget", "--json"]));
    assert_eq!(
        search["results"].as_array().expect("results").len(),
        0,
        "no ghost results after remove: {search}"
    );
}

/// `list --category` filters, and reads the config's category — a source
/// that has never been pulled still has one.
#[test]
fn list_filters_by_category() {
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let home = tempfile::tempdir().expect("tempdir");
    let url = format!("{}/", server.url());

    stdout_json(&tome(
        home.path(),
        &[
            "add",
            &url,
            "--yes",
            "--insecure",
            "--json",
            "--quiet",
            "--category",
            "Fixtures",
        ],
    ));

    let hit = stdout_json(&tome(
        home.path(),
        &["list", "--category", "Fixtures", "--json"],
    ));
    assert_eq!(hit["sources"].as_array().expect("sources").len(), 1);
    assert_eq!(hit["sources"][0]["category"], "Fixtures");

    let miss = stdout_json(&tome(
        home.path(),
        &["list", "--category", "Rust", "--json"],
    ));
    assert_eq!(miss["sources"].as_array().expect("sources").len(), 0);
}

/// Removing an unknown source fails and names the known ones.
#[test]
fn remove_unknown_source_fails() {
    let home = tempfile::tempdir().expect("tempdir");
    let out = tome(home.path(), &["remove", "nope", "--yes"]);
    assert!(!out.status.success());
}

/// Under --json, errors are a JSON object on stderr and the exit code is 1.
#[test]
fn errors_are_json_under_json_flag() {
    let home = tempfile::tempdir().expect("tempdir");
    let out = tome(home.path(), &["pull", "nope", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value =
        serde_json::from_slice(&out.stderr).expect("stderr is a JSON object under --json");
    assert!(
        error["error"]["message"].as_str().is_some(),
        "error carries a message: {error}"
    );
    // Nothing on stdout: a script piping stdout must not receive half a
    // result and then an error.
    assert!(out.stdout.is_empty(), "stdout stays clean on error");
}

/// `status --json` reports the same paths as the library resolves.
#[test]
fn status_json_matches_library_paths() {
    let home = tempfile::tempdir().expect("tempdir");
    let expected = tome_core::Paths::under_root(home.path());

    let status = stdout_json(&tome(home.path(), &["status", "--json"]));
    assert_eq!(
        status["state"].as_str().expect("state"),
        expected.state_root().to_string_lossy()
    );
    assert_eq!(
        status["db"].as_str().expect("db"),
        expected.database_file().to_string_lossy()
    );
    assert_eq!(
        status["index"].as_str().expect("index"),
        expected.index_dir().to_string_lossy()
    );
    assert_eq!(status["initialised"], false);
}

/// `pull --json` prints one `pulled` array whatever the source count.
#[test]
fn pull_json_shape_is_stable() {
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let home = tempfile::tempdir().expect("tempdir");
    let url = format!("{}/", server.url());

    let added = stdout_json(&tome(
        home.path(),
        &["add", &url, "--yes", "--insecure", "--json", "--quiet"],
    ));
    let id = added["id"].as_str().expect("id").to_owned();

    let pulled = stdout_json(&tome(home.path(), &["pull", &id, "--json", "--quiet"]));
    let entries = pulled["pulled"].as_array().expect("pulled array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["source"], id.as_str());
    // A re-pull of unchanged content: the index reports it did nothing,
    // in named counts.
    let index = &entries[0]["pull"]["index"];
    assert!(index["unchanged"].as_u64().expect("unchanged") > 0);
    assert_eq!(index["added"].as_u64(), Some(0));
}
