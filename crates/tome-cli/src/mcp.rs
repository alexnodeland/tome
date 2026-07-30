//! The MCP stdio server (P4-013/P4-014), built to SPIKE-008's measurements
//! rather than to the current spec revision — the distinction matters:
//!
//! * **The protocol era is legacy, deliberately.** The current MCP revision
//!   (2026-07-28) abolished the `initialize` handshake for per-request
//!   versioning, but the shipping client — Claude Code 2.1.220, tested for
//!   real in [`docs/spikes/008-mcp-protocol.md`] — still opens with the
//!   legacy `2025-11-25` handshake and has **no fall-forward mechanism**
//!   against a modern-only server. A dual-era server is additive later;
//!   nothing here is thrown away by adding one.
//! * **Version negotiation must not be "answer with our newest".** The spike
//!   measured what happens when the server answers a version the client does
//!   not know: a **silent drop** — no error anywhere, the tools simply never
//!   appear. So the server echoes the client's requested version whenever it
//!   is one we support, and only otherwise offers our latest.
//! * **Nothing but protocol messages on stdout.** Everything in this module
//!   writes through [`write_message`]; diagnostics go through `tracing`,
//!   which `main` pointed at stderr before anything else ran. The spike found
//!   Claude Code *currently* skips whole garbage lines, but that is one
//!   client's leniency, not the contract — and a partial write interleaved
//!   into a JSON line is unrecoverable by line-skipping.
//!
//! The serve loop is deliberately synchronous and serial. Claude Code issues
//! requests strictly serially (observed in every spike session), every tool
//! call is a local read measured in microseconds (P2-018), and the one slow
//! operation — opening the index — happens once, lazily, at the first call
//! that needs it rather than during `initialize` (the client is waiting on
//! that reply). If a future client interleaves requests, this is the place a
//! worker pool would go.

use std::io::{BufRead, Write};

use anyhow::Result;
use serde_json::{json, Value};
use tome_core::Paths;

/// Protocol revisions this server implements. All four share the handshake
/// and the `tools/*` surface this server uses; the differences (elicitation,
/// structured tool output) are in capabilities Tome does not advertise.
const SUPPORTED_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What we offer a client whose requested version we do not know. Per the
/// legacy negotiation rules the client then decides whether it can proceed.
const LATEST_SUPPORTED: &str = "2025-11-25";

/// One tool the server exposes. S3-3 provides the implementations; the
/// handler only needs the contract.
///
/// `call` returns `Err(message)` for a **tool** error — reported inside a
/// successful JSON-RPC response with `isError: true`, so the model sees an
/// actionable message. JSON-RPC errors are reserved for protocol-level
/// failures (unknown tool, malformed arguments); the distinction is P4-013's
/// "actionable tool errors rather than transport errors".
pub(crate) trait McpTool {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    /// JSON Schema for the tool's arguments.
    fn input_schema(&self) -> Value;
    fn call(&self, state: &mut McpState, arguments: &Value) -> std::result::Result<String, String>;
}

/// Shared, lazily initialised access to the library.
///
/// The server is spawned per client and short-lived, so it must not hold
/// exclusive locks: the search index opens read-only on first use, and
/// SQLite's own locking covers the database. Deferring the open also keeps
/// `initialize` fast — the client is blocked on it.
pub(crate) struct McpState {
    pub paths: Paths,
    engine: Option<tome_core::search::SearchEngine>,
}

impl McpState {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            engine: None,
        }
    }

    /// The search engine, opened on first use.
    pub fn engine(&mut self) -> tome_core::Result<&tome_core::search::SearchEngine> {
        if self.engine.is_none() {
            self.engine = Some(tome_core::search::SearchEngine::open(&self.paths)?);
        }
        #[allow(clippy::expect_used)] // just assigned above
        Ok(self.engine.as_ref().expect("engine just opened"))
    }
}

/// Serve MCP over this process's stdin/stdout until stdin closes.
pub(crate) fn serve_stdio(paths: &Paths, tools: Vec<Box<dyn McpTool>>) -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    serve(
        McpState::new(paths.clone()),
        tools,
        stdin.lock(),
        stdout.lock(),
    )
}

/// The server loop, generic over its streams so a test can run a scripted
/// session against byte buffers — the spike's test-client pattern.
fn serve(
    mut state: McpState,
    tools: Vec<Box<dyn McpTool>>,
    reader: impl BufRead,
    mut writer: impl Write,
) -> Result<()> {
    for line in reader.lines() {
        // A read error on stdin is the client going away mid-line; exit as
        // cleanly as EOF. Looping on a dead pipe leaves orphan processes.
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }

        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(e) => {
                // Parse errors have no id to echo; the spec's -32700 with a
                // null id is the best that can be said.
                write_message(
                    &mut writer,
                    &error_response(Value::Null, -32700, &format!("parse error: {e}")),
                )?;
                continue;
            }
        };

        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        let Some(id) = id else {
            // A notification. `notifications/initialized` requires no action;
            // unknown notifications are ignored per spec (never answered —
            // a reply to a notification is itself a protocol violation).
            continue;
        };

        let response = match method {
            "initialize" => handle_initialize(&id, &params),
            "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
            "tools/list" => handle_tools_list(&id, &tools),
            "tools/call" => handle_tools_call(&id, &params, &tools, &mut state),
            other => error_response(id, -32601, &format!("Method not found: {other}")),
        };
        write_message(&mut writer, &response)?;
    }
    Ok(()) // stdin closed: exit cleanly, leave no orphan
}

fn handle_initialize(id: &Value, params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Echo the client's version whenever we support it. The spike measured
    // the failure mode this avoids: a version the client does not recognise
    // is a silent drop — tools absent, no diagnostic anywhere.
    let version = if SUPPORTED_VERSIONS.contains(&requested) {
        requested
    } else {
        LATEST_SUPPORTED
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": version,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "tome",
                "title": "Tome",
                "version": env!("CARGO_PKG_VERSION"),
            },
        },
    })
}

fn handle_tools_list(id: &Value, tools: &[Box<dyn McpTool>]) -> Value {
    let tools: Vec<Value> = tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name(),
                "description": tool.description(),
                "inputSchema": tool.input_schema(),
            })
        })
        .collect();
    json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools } })
}

fn handle_tools_call(
    id: &Value,
    params: &Value,
    tools: &[Box<dyn McpTool>],
    state: &mut McpState,
) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let Some(tool) = tools.iter().find(|tool| tool.name() == name) else {
        // Protocol error, not a tool error: the client asked for something
        // `tools/list` never offered.
        return error_response(id.clone(), -32602, &format!("Unknown tool: {name}"));
    };
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    // Tool failures are results with `isError`, so the model reads an
    // actionable message instead of the client surfacing a transport fault.
    let (text, is_error) = match tool.call(state, &arguments) {
        Ok(text) => (text, false),
        Err(message) => (message, true),
    };
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [ { "type": "text", "text": text } ],
            "isError": is_error,
        },
    })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

/// The only path to the protocol stream. One line per message, flushed
/// immediately — the client is blocked on every reply, and a buffered
/// half-message is exactly the corruption the stdout rule exists to prevent.
fn write_message(writer: &mut impl Write, message: &Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, message)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    struct Hello;
    impl McpTool for Hello {
        fn name(&self) -> &'static str {
            "hello"
        }
        fn description(&self) -> &'static str {
            "Say hello."
        }
        fn input_schema(&self) -> Value {
            json!({ "type": "object", "properties": {} })
        }
        fn call(&self, _: &mut McpState, arguments: &Value) -> std::result::Result<String, String> {
            match arguments.get("fail") {
                Some(_) => Err("deliberate tool failure".into()),
                None => Ok("hello back".into()),
            }
        }
    }

    /// Run a scripted session; every stdout line must parse as JSON.
    fn session(input: &str) -> Vec<Value> {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state = McpState::new(Paths::under_root(tmp.path()));
        let mut output = Vec::new();
        serve(state, vec![Box::new(Hello)], input.as_bytes(), &mut output).expect("serve runs");
        String::from_utf8(output)
            .expect("stdout is UTF-8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("every stdout line is JSON"))
            .collect()
    }

    fn initialize_with(version: &str) -> String {
        format!(
            "{}\n",
            json!({
                "jsonrpc": "2.0", "id": 0, "method": "initialize",
                "params": { "protocolVersion": version, "capabilities": {},
                            "clientInfo": { "name": "test", "version": "0" } },
            })
        )
    }

    #[test]
    fn initialize_echoes_a_supported_version() {
        // The spike's central negotiation finding: answer the client's own
        // version when supported, because an unknown answer is a silent drop.
        for version in ["2025-11-25", "2025-06-18"] {
            let replies = session(&initialize_with(version));
            assert_eq!(replies[0]["result"]["protocolVersion"], version);
            assert_eq!(replies[0]["id"], 0);
        }
    }

    #[test]
    fn initialize_offers_latest_for_an_unknown_version() {
        let replies = session(&initialize_with("1900-01-01"));
        assert_eq!(replies[0]["result"]["protocolVersion"], LATEST_SUPPORTED);
    }

    #[test]
    fn notifications_get_no_reply() {
        // A reply to a notification is itself a protocol violation — and the
        // spike showed Claude Code sends `notifications/initialized` on every
        // session, so one bad reply would corrupt every session.
        let input = format!(
            "{}{}\n{}\n",
            initialize_with("2025-11-25"),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            json!({ "jsonrpc": "2.0", "method": "notifications/unknown" }),
        );
        let replies = session(&input);
        assert_eq!(replies.len(), 1, "only initialize is answered: {replies:?}");
    }

    #[test]
    fn tools_list_and_call_round_trip() {
        let input = format!(
            "{}{}\n{}\n",
            initialize_with("2025-11-25"),
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": { "name": "hello", "arguments": {} } }),
        );
        let replies = session(&input);
        assert_eq!(replies[1]["result"]["tools"][0]["name"], "hello");
        assert_eq!(replies[2]["result"]["content"][0]["text"], "hello back");
        assert_eq!(replies[2]["result"]["isError"], false);
    }

    #[test]
    fn tool_failure_is_a_result_not_a_protocol_error() {
        let input = format!(
            "{}{}\n",
            initialize_with("2025-11-25"),
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": { "name": "hello", "arguments": { "fail": true } } }),
        );
        let replies = session(&input);
        assert!(replies[1].get("error").is_none(), "no JSON-RPC error");
        assert_eq!(replies[1]["result"]["isError"], true);
        assert_eq!(
            replies[1]["result"]["content"][0]["text"],
            "deliberate tool failure"
        );
    }

    #[test]
    fn unknown_tool_is_a_protocol_error() {
        let input = format!(
            "{}{}\n",
            initialize_with("2025-11-25"),
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": { "name": "nope", "arguments": {} } }),
        );
        let replies = session(&input);
        assert_eq!(replies[1]["error"]["code"], -32602);
    }

    #[test]
    fn unknown_method_and_bad_json_get_spec_errors() {
        let input = format!(
            "{}{}\nnot json at all\n",
            initialize_with("2025-11-25"),
            json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" }),
        );
        let replies = session(&input);
        assert_eq!(replies[1]["error"]["code"], -32601);
        assert_eq!(replies[2]["error"]["code"], -32700);
        assert_eq!(replies[2]["id"], Value::Null);
    }

    #[test]
    fn ping_answers_empty_object() {
        let input = format!(
            "{}{}\n",
            initialize_with("2025-11-25"),
            json!({ "jsonrpc": "2.0", "id": 7, "method": "ping" }),
        );
        let replies = session(&input);
        assert_eq!(replies[1]["result"], json!({}));
    }

    #[test]
    fn eof_ends_the_session_cleanly() {
        // `session` already asserts serve() returns Ok on EOF; this pins the
        // empty-session case — a client that connects and immediately goes
        // away must not produce output or an error.
        assert!(session("").is_empty());
    }
}
