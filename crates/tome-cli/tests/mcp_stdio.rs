//! The MCP server, tested as a process (P4-014).
//!
//! The unit tests in `src/mcp.rs` prove the handler; these prove the two
//! properties only the real binary can prove, both prescribed by
//! SPIKE-008's write-up:
//!
//! * **stdout purity.** Every byte the process writes to stdout is part of a
//!   JSON-RPC message — under a scripted session *with logging turned all
//!   the way up*, because a stray diagnostic `println!` is exactly the
//!   defect this test exists to catch, and it only appears when something
//!   logs.
//! * **Clean exit on stdin close.** The client going away must end the
//!   process, not orphan it.
//!
//! The handshake against real Claude Code cannot run in this suite (it
//! requires a logged-in client). It was run by hand before this landed —
//! 2026-07-30, Claude Code 2.1.220 against `tome mcp` over a pulled fixture
//! library: it connected, called `tome_search` and `tome_get_page`, and
//! quoted the page. The command shape is in the SPIKE-008 write-up; re-run
//! it when the protocol handler changes.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use tome_testkit::FixtureServer;

fn tome_bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().expect("test binary has a path");
    p.pop(); // deps/
    p.pop(); // debug/ or release/
    p.push("tome");
    p
}

/// The spike's scripted session: the four-message surface, then EOF.
const SESSION: &[&str] = &[
    r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}"#,
    r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
];

#[test]
fn stdout_is_pure_json_rpc_and_stdin_close_ends_the_process() {
    let home = tempfile::tempdir().expect("tempdir");
    let mut child = Command::new(tome_bin())
        .arg("mcp")
        .env("TOME_HOME", home.path())
        // Maximum logging, deliberately: if anything in the process logs to
        // stdout, this is the configuration that makes it do so.
        .env("RUST_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tome mcp starts");

    {
        let stdin = child.stdin.as_mut().expect("stdin piped");
        for line in SESSION {
            writeln!(stdin, "{line}").expect("write to server");
        }
    } // scope end drops nothing yet — stdin is closed below

    // Closing stdin is the client disconnecting. The process must exit on
    // its own; `wait_with_output` would hang forever if it looped on EOF.
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("server exits");

    assert!(
        out.status.success(),
        "clean exit on stdin close, got {:?}.\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    // Every stdout line parses as a JSON-RPC message with the right id, in
    // order. Anything else on the stream — a log line, a partial write, a
    // reply to a notification — fails here.
    let stdout = String::from_utf8(out.stdout).expect("stdout is UTF-8");
    let replies: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("stdout line is not JSON-RPC ({e}): {line:?}"))
        })
        .collect();

    assert_eq!(replies.len(), 3, "three requests, three replies: {stdout}");
    assert_eq!(replies[0]["id"], 0);
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(replies[0]["jsonrpc"], "2.0");
    assert_eq!(replies[1]["id"], 1);
    assert!(
        replies[1]["result"]["tools"].is_array(),
        "tools/list answers an array: {}",
        replies[1]
    );
    assert_eq!(replies[2]["id"], 2);
}

/// Run a scripted MCP session against the binary; return the replies.
fn mcp_session(home: &Path, requests: &[String]) -> Vec<serde_json::Value> {
    let mut child = Command::new(tome_bin())
        .arg("mcp")
        .env("TOME_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tome mcp starts");
    {
        let stdin = child.stdin.as_mut().expect("stdin piped");
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":0,"method":"initialize","params":{{"protocolVersion":"2025-11-25","capabilities":{{}},"clientInfo":{{"name":"smoke","version":"0"}}}}}}"#
        )
        .expect("initialize");
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
        )
        .expect("initialized");
        for request in requests {
            writeln!(stdin, "{request}").expect("request");
        }
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("server exits");
    assert!(out.status.success(), "clean exit: {:?}", out.status);
    String::from_utf8(out.stdout)
        .expect("stdout is UTF-8")
        .lines()
        .skip(1) // the initialize reply
        .map(|line| serde_json::from_str(line).expect("stdout line is JSON-RPC"))
        .collect()
}

fn call(id: u64, tool: &str, arguments: serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments },
    })
    .to_string()
}

fn text_of(reply: &serde_json::Value) -> &str {
    reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("reply has text content: {reply}"))
}

/// The whole point of the stage: pull a source, then read it over MCP.
/// Mirrors what Claude Code does — search, open a result, look at the TOC.
#[test]
fn a_pulled_library_is_readable_over_mcp() {
    let server = FixtureServer::start("sphinx-example").expect("fixture server");
    let home = tempfile::tempdir().expect("tempdir");
    let url = format!("{}/", server.url());

    // Populate the library through the real CLI.
    let add = Command::new(tome_bin())
        .args([
            "add",
            &url,
            "--yes",
            "--insecure",
            "--quiet",
            "--name",
            "widget",
        ])
        .env("TOME_HOME", home.path())
        .stdin(Stdio::null())
        .output()
        .expect("tome add runs");
    assert!(
        add.status.success(),
        "add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let replies = mcp_session(
        home.path(),
        &[
            call(1, "tome_list_sources", serde_json::json!({})),
            call(2, "tome_search", serde_json::json!({ "query": "widget" })),
            call(
                3,
                "tome_get_toc",
                serde_json::json!({ "source_id": "widget" }),
            ),
            call(
                4,
                "tome_get_page",
                serde_json::json!({ "source_id": "widget", "page_path": "index.html" }),
            ),
            call(
                5,
                "tome_get_page",
                serde_json::json!({ "source_id": "widget", "page_path": "nope.html" }),
            ),
        ],
    );

    // list_sources names the source and its page count.
    let sources = text_of(&replies[0]);
    assert!(sources.contains("pages"), "sources listed: {sources}");

    // search finds the fixture's content and points at a page.
    let search = text_of(&replies[1]);
    assert!(
        search.contains("(") && !search.contains("No results"),
        "search hits: {search}"
    );
    assert_eq!(replies[1]["result"]["isError"], false);

    // The TOC is in navigation order and includes the homepage.
    let toc = text_of(&replies[2]);
    assert!(toc.contains("index.html"), "toc lists pages: {toc}");

    // get_page returns readable markdown: the fixture's h1 and prose.
    let page = text_of(&replies[3]);
    assert!(page.contains("# Widget"), "page renders headings: {page}");
    assert!(
        page.contains("fixture, not a library"),
        "page renders prose: {page}"
    );

    // A missing page is a TOOL error with a remedy, not a protocol error.
    assert_eq!(replies[4]["result"]["isError"], true);
    assert!(replies[4].get("error").is_none());
    assert!(
        text_of(&replies[4]).contains("tome_search"),
        "error names the remedy: {}",
        text_of(&replies[4])
    );
}

/// Against an empty library, tools answer with guidance — never a protocol
/// error, and never a hang on a missing index.
#[test]
fn an_empty_library_answers_with_guidance() {
    let home = tempfile::tempdir().expect("tempdir");
    let replies = mcp_session(
        home.path(),
        &[
            call(1, "tome_search", serde_json::json!({ "query": "anything" })),
            call(2, "tome_list_sources", serde_json::json!({})),
        ],
    );
    assert_eq!(replies[0]["result"]["isError"], true);
    assert!(
        text_of(&replies[0]).contains("pulled"),
        "search names the state: {}",
        text_of(&replies[0])
    );
    assert_eq!(replies[1]["result"]["isError"], false);
    assert!(
        text_of(&replies[1]).contains("empty"),
        "list says the library is empty: {}",
        text_of(&replies[1])
    );
}
