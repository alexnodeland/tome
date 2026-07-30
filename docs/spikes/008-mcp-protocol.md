# SPIKE-008 — MCP protocol, tested against the real client

**Date:** 2026-07-30 · **Status:** complete · **Verdict:** stdio works end-to-end against
Claude Code; the minimal message set is four methods; the protocol just changed underneath us
and the change does not affect what Tome must build first.
**Spec:** [`docs/plans/07-technical-spikes.md`](../plans/07-technical-spikes.md) § SPIKE-008.
**Gates:** Stage 3 entry ([`docs/plans/18`](../plans/18-implementation-plan.md) § Stage 3).

## Result against the spec's success criteria

| Criterion | Outcome |
|---|---|
| Trivial server reachable from Claude Code end-to-end | **Yes.** A ~90-line hand-rolled stdio server answered a real `tools/call` from Claude Code 2.1.220 on the first attempt. Raw trace below. |
| Known-correct minimal message set for v1 | **Four methods:** `initialize`, `notifications/initialized`, `tools/list`, `tools/call`; `-32601` for everything else. Claude Code sent nothing else in any session. |
| Protocol revision(s) to support, with negotiation behaviour | **`2025-11-25`, handshake-based.** That is what Claude Code 2.1.220 requests. See "The spec moved" below for why the *current* spec revision (2026-07-28) is not what to build. |
| Confirmed stdout discipline requirement | **Confirmed, with a nuance:** Claude Code skips whole non-JSON lines rather than disconnecting. The invariant stands — that is one client's leniency, not a spec guarantee — but the failure mode is subtler than "client disconnects": a *partial* write interleaved into a JSON line still corrupts it. |

## The spec moved, and which era to build

The **current** MCP revision is **`2026-07-28`** — published two days before this spike ran. It
is a breaking redesign: no `initialize` handshake at all; every request carries its protocol
version in `_meta`, servers answer unknown versions with `UnsupportedProtocolVersionError`
(-32022), and a mandatory `server/discover` RPC replaces capability exchange. The spec calls
these eras **modern** (2026-07-28+) and **legacy** (2025-11-25 and earlier, handshake-based).

**Claude Code 2.1.220 — released after that revision — is a legacy client.** It opens with
`initialize` requesting `2025-11-25` and never probes `server/discover`. The spec's own
compatibility matrix says a legacy client against a modern-only server **fails with no
fall-forward mechanism**.

So the era decision writes itself:

- **Tome implements the legacy handshake (`2025-11-25`)**, which per the matrix also serves
  every older client.
- **Modern-era support is deferred until a client that speaks it exists.** A dual-era server is
  explicitly permitted ("selects its behavior from how the client opens"), so adding it later is
  additive: answer `server/discover` and per-request `_meta` when they appear, keep answering
  `initialize` when it does. Nothing built now is thrown away.
- One modern-spec **SHOULD** is worth honouring immediately because it costs three lines: a
  server rejecting an `initialize` it cannot serve should *name the versions it supports in the
  error message*, because a legacy client may surface nothing else to the user.

## Version negotiation, observed

The handshake negotiation is: client proposes a version in `initialize`; server answers with
that version if it supports it, **or any other version it does support**; the client then
decides. Measured against Claude Code 2.1.220:

| Server's `initialize` answer | Client behaviour |
|---|---|
| Echo the requested `2025-11-25` | Proceeds. |
| `2025-06-18` (older, known to the client) | **Proceeds normally** — full session, tool call succeeds. |
| `1900-01-01` (unknown) | **Silent drop.** No further messages, no error written to the server, nothing on the client's stderr in `-p` mode. The tools are simply absent. |

That last row is the important one: an unsupported version does not fail loudly anywhere. It
presents as "the tome tools don't show up", which is indistinguishable from a config typo. This
is why the version string Tome answers with must be *the client's requested version whenever we
support it*, never a hardcoded newest.

## Stdout discipline, observed

With a `debug: starting up\n` line written to stdout before the `initialize` response, Claude
Code 2.1.220 **skipped the line and completed the session normally**. The plan's expectation
("the client disconnects with an opaque parse error") is not what this client does today —
its reader drops unparseable lines.

The invariant is unchanged — nothing but JSON-RPC on stdout — for three reasons: other clients
are not obliged to be lenient; a stray write that lands *mid-line* (from a non-line-buffered
writer, or a panic message) corrupts a real message in a way line-skipping cannot save; and
leniency observed in one client version is not a contract. What changes is only the *test's*
framing: assert stdout is pure JSON-RPC because that is the spec's requirement, not because
Claude Code enforces it.

## Oversized results, observed

A tool returning **500 000 characters** in one text block did not break anything at the
protocol level: the client accepted the frame, then diverted the content to a file on disk
instead of the model's context, telling the model it "exceeded the token limit". (Claude Code's
default cap on tool output is 25 000 tokens.)

So a huge result is not a *transport* risk — it is a *usefulness* defect: the agent asked for a
page and got a filename. This confirms P4-013's design as the right shape for the right reason:
truncate to a token budget with `truncated: true`, and give `tome_get_page` a `section`
argument so an agent can fetch a subtree instead of the document. That work is S3-4.

## Other confirmations

- **Transports are stdio and Streamable HTTP.** Both eras of the spec define exactly these two;
  there is no Unix-socket transport (the error the original plan made, corrected in P4-013).
  `tome mcp --http` therefore remains a legal opt-in surface. The deprecated HTTP+SSE transport
  should not be built.
- **`tools/call` carries client-namespaced `_meta`** (`claudecode/toolUseId`, `progressToken`).
  A server must tolerate unknown `_meta` keys and unknown params fields generally.
- **Requests arrived strictly serially** in every observed session. Concurrent handling
  (P4-014) is still required by spec, but nothing observed exercises it.
- **Clean exit on stdin close works and matters:** the hello server exits when stdin closes,
  and no orphan processes were left behind by any of the five sessions.
- **No SDK is needed.** The entire required surface is four methods over line-delimited
  JSON-RPC; the hello server is ~90 lines. Hand-rolling in `tome-cli` with `serde_json`
  (already in the tree) avoids a new dependency (`rmcp`) whose value would be transports and
  capabilities Tome does not use. This matches P4-014's sketch.

## Method

A ~90-line Node stdio server (`server.mjs`, preserved below) implementing the four methods,
with fault injection via environment variables: claimed protocol version, a stray stdout line,
an N-byte tool result. Every message in and out is appended to a trace file, so every claim
above quotes a real session rather than a reconstruction. The client is the real one:

```
claude -p '…call the hello tool…' --mcp-config cfg.json --strict-mcp-config \
  --allowedTools "mcp__hello__hello"
```

five times: happy path, server answers 2025-06-18, server answers 1900-01-01, stdout garbage,
500 KB result. Claude Code 2.1.220, Node 26.3.0, macOS 26.5.

## Raw trace — the happy path, verbatim

`->` client to server, `<-` server to client. This is the complete session: six messages, and
the whole protocol surface S3-2 must implement.

```
-> {"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{"roots":{"listChanged":true},"elicitation":{}},"clientInfo":{"name":"claude-code","title":"Claude Code","version":"2.1.220","description":"Anthropic's agentic coding tool","websiteUrl":"https://claude.com/claude-code"}},"jsonrpc":"2.0","id":0}
<- {"jsonrpc":"2.0","id":0,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"spike008-hello","version":"0.0.1"}}}
-> {"method":"notifications/initialized","jsonrpc":"2.0"}
-> {"method":"tools/list","jsonrpc":"2.0","id":1}
<- {"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"hello","description":"Say hello. Returns a greeting for the given name.","inputSchema":{"type":"object","properties":{"name":{"type":"string","description":"Who to greet"}},"required":["name"]}}]}}
-> {"method":"tools/call","params":{"name":"hello","arguments":{"name":"spike008"},"_meta":{"claudecode/toolUseId":"toolu_015JoGQzFVUniKQhYswj5Fzk","progressToken":2}},"jsonrpc":"2.0","id":2}
<- {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"Hello, spike008! (from spike008)"}]}}
```

And the version-rejection session in full — note the client goes silent rather than erroring:

```
-> {"method":"initialize","params":{"protocolVersion":"2025-11-25", …}}
<- {"jsonrpc":"2.0","id":0,"result":{"protocolVersion":"1900-01-01", …}}
(nothing further; tools absent from the session; no diagnostic anywhere)
```

## The test client, kept

The spike's third output is a development test client. The scripted-session form used here is
the one S3-2's stdout-purity test needs — no MCP client required at all:

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"hello","arguments":{"name":"tome"}}}' \
  | tome mcp
```

Every line of the output must parse as JSON-RPC with the expected `id`s — that *is* the
stdout-discipline test, and closing the pipe *is* the clean-exit test.

<details>
<summary>server.mjs — the hello-world server, as run</summary>

```js
#!/usr/bin/env node
// SPIKE-008 hello-world MCP stdio server.
//
// Deliberately hand-rolled — the spike's question is what the protocol
// minimally requires, and an SDK would answer a different question.
//
// Fault injection via env vars:
//   SPIKE_PROTOCOL_VERSION  — protocolVersion to claim in initialize result
//                             (default: echo the client's requested version)
//   SPIKE_STDOUT_GARBAGE=1  — print a non-JSON line to stdout before the
//                             initialize response (the stray-println! test)
//   SPIKE_HUGE_RESULT=N     — hello tool returns N bytes of text
import { appendFileSync } from "node:fs";
import { createInterface } from "node:readline";

const TRACE = process.env.SPIKE_TRACE || "/dev/null";
const trace = (dir, line) => appendFileSync(TRACE, `${dir} ${line}\n`);

const send = (msg) => {
  const line = JSON.stringify(msg);
  trace("<-", line);
  process.stdout.write(line + "\n");
};

const TOOLS = [
  {
    name: "hello",
    description: "Say hello. Returns a greeting for the given name.",
    inputSchema: {
      type: "object",
      properties: { name: { type: "string", description: "Who to greet" } },
      required: ["name"],
    },
  },
];

const rl = createInterface({ input: process.stdin });
rl.on("line", (line) => {
  trace("->", line);
  let msg;
  try {
    msg = JSON.parse(line);
  } catch {
    return; // a real server would answer -32700; irrelevant to the spike
  }
  const { id, method, params } = msg;

  if (method === "initialize") {
    if (process.env.SPIKE_STDOUT_GARBAGE === "1") {
      trace("<-", "GARBAGE LINE");
      process.stdout.write("debug: starting up\n"); // the stray println!
    }
    send({
      jsonrpc: "2.0",
      id,
      result: {
        protocolVersion:
          process.env.SPIKE_PROTOCOL_VERSION || params.protocolVersion,
        capabilities: { tools: {} },
        serverInfo: { name: "spike008-hello", version: "0.0.1" },
      },
    });
  } else if (method === "notifications/initialized") {
    // notification: no reply
  } else if (method === "tools/list") {
    send({ jsonrpc: "2.0", id, result: { tools: TOOLS } });
  } else if (method === "tools/call") {
    const huge = Number(process.env.SPIKE_HUGE_RESULT || 0);
    const text = huge
      ? "x".repeat(huge)
      : `Hello, ${params?.arguments?.name ?? "world"}! (from spike008)`;
    send({
      jsonrpc: "2.0",
      id,
      result: { content: [{ type: "text", text }] },
    });
  } else if (id !== undefined) {
    // Requests we don't implement get a proper error; notifications are ignored.
    send({
      jsonrpc: "2.0",
      id,
      error: { code: -32601, message: `Method not found: ${method}` },
    });
  }
});
rl.on("close", () => process.exit(0)); // stdin closed: exit, leave no orphan
```

</details>

## What S3-2 inherits from this

1. Implement the **legacy `2025-11-25` handshake**; echo the client's requested version when
   supported; when rejecting, name the supported versions in the error string.
2. The four-method surface, `-32601` for the rest, tolerate unknown `_meta`/params keys.
3. Line-delimited JSON-RPC on stdout, nothing else, flushed per message; logging to stderr
   (already the `tome-cli` default — the tracing subscriber writes to stderr).
4. Exit cleanly on stdin EOF.
5. No new dependency: hand-roll on `serde_json`.
6. The stdout-purity and clean-exit test is a scripted pipe, not a mock client; the
   *handshake* test against real Claude Code stays a manual gate command
   (documented, run before merging S3-2) because the gate script cannot assume a logged-in
   Claude Code.
