---
title: MCP
description: Run the Stormbuffer MCP adapter over stdio.
section: Reference
group: Agents
order: 4
---

`stormbuffer-mcp` exposes Stormbuffer to MCP hosts over stdio. It uses JSON-RPC
2.0 and the [official MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
(`rmcp = 2.1.0`, with the server and stdio transport features). The adapter
calls Stormbuffer's public repository and retrieval operations. It does not
open SQLite directly or edit arbitrary files. It loads the verified local model
only when `memory_recall` needs semantic retrieval.

## Before connecting

Install Stormbuffer and initialize the store you want the adapter to use. See
[Installation](/docs/installation/) and the [quick start](/docs/quick-start/).
Connecting an MCP host never initializes a store or creates records.

The adapter can start without a local model. In that case, `memory_recall`
falls back to lexical retrieval, while resources and the other tools remain
available. Run `sbuf init` while online and restart the adapter to enable hybrid
retrieval after a recall has detected that the model is unavailable.

## Choose a store view and write access

The examples below provide read-only access. To let a host propose changes or
archive records, start the adapter with `--allow-writes`. This flag enables
`memory_remember`, `memory_update`, and `memory_forget`. Remember and update
create candidates for a person to approve. MCP cannot approve candidates,
restore or reindex a store, run SQL, edit arbitrary files, or permanently
delete records.

Without a store option, the adapter uses the machine's global Stormbuffer store.
Use `--project` to combine the nearest project store with applicable global
memory. Use `--local` to open only the nearest store. Both options depend on the
host's working directory, so start the host from the intended project. Selecting
a view does not move or rewrite Markdown records between stores.

## Connect Codex

Codex can register a local stdio server from its CLI. To use the global store:

```sh
codex mcp add stormbuffer -- stormbuffer-mcp --stdio
codex mcp list
```

For a project store, register the adapter with `--project` instead:

```sh
codex mcp add stormbuffer -- stormbuffer-mcp --stdio --project
```

Replace `--project` with `--local` when the connection must be isolated from
global memory.

Keep one registration under the `stormbuffer` name. When using `--project`,
start Codex from the initialized project. Add `--allow-writes` after `--stdio`
only when Codex should create candidates or archive records.

Codex stores MCP configuration in `~/.codex/config.toml`. Trusted projects can
also use `.codex/config.toml`. See the
[Codex MCP documentation](https://developers.openai.com/codex/mcp) for the
configuration fields and host controls. Run `/mcp` in an interactive Codex
session to inspect the connection.

## Connect Pi

Install [pi-mcp-adapter](https://github.com/nicobailon/pi-mcp-adapter), then restart Pi:

```sh
pi install npm:pi-mcp-adapter
```

For project memory, create `.mcp.json` in the initialized project:

```json
{ "mcpServers": { "stormbuffer": { "command": "stormbuffer-mcp", "args": ["--stdio", "--project"] } } }
```

Run Pi from that project. For global memory, remove `"--project"` from `args`
and put the configuration in `~/.config/mcp/mcp.json`. Add `"--allow-writes"`
to `args` only when Pi should create candidates or archive records.

`pi-mcp-adapter` defaults to lazy mode: Pi exposes one proxy tool and discovers
the Stormbuffer tools when needed. Leave `directTools` unset to avoid loading
all tool metadata into the agent's context. Run `/mcp` in Pi to inspect the
server and its tools.

## Connect another host

A host that accepts the common `mcpServers` configuration shape can start a
read-only adapter with the composed project view:

```json
{ "mcpServers": { "stormbuffer": { "command": "stormbuffer-mcp", "args": ["--stdio", "--project"] } } }
```

Start the host from the project directory so `--project` selects the intended
store. Add `--allow-writes` only when the host should create candidates or
archive records.

## JSON-RPC lifecycle

The host sends `initialize`, followed by the `notifications/initialized`
notification. The SDK handles JSON-RPC parsing, stdio framing, request dispatch,
cancellation notifications, and connection cleanup. Close the adapter's stdin
to stop the process. The adapter rejects a request if the SDK has already
cancelled it. Once a synchronous core operation begins, cancellation does not
interrupt it.

Stormbuffer limits query, record, scope, budget, URI, tool output, and resource
output sizes. `rmcp` rejects malformed JSON and invalid method parameters.
Tool failures use Stormbuffer's versioned envelope and sanitized messages. Error
messages do not include canonical paths or backtraces.

## Resources

The adapter advertises these URI templates through the SDK's
`resources/templates/list` handler:

| URI template                          | Contents                                                            |
| ------------------------------------- | ------------------------------------------------------------------- |
| `stormbuffer://record/{id}`           | One agent-readable record as JSON.                                  |
| `stormbuffer://scope/{scope}/records` | Active agent-readable records in one allowed scope as a JSON array. |
| `stormbuffer://candidate/{id}`        | One agent-readable candidate as JSON.                               |

Use `resources/read` with one of those URIs. A scope must be `global` or an
allowed `project:<project-id>` scope. Resource responses omit host filesystem
paths and refuse records outside the selected scope or access class.

## Tools

`tools/list` returns exactly these tools:

| Tool              | Operation  | Default              |
| ----------------- | ---------- | -------------------- |
| `memory_recall`   | `context`  | enabled              |
| `memory_get`      | `get`      | enabled              |
| `memory_remember` | `remember` | write grant required |
| `memory_update`   | `update`   | write grant required |
| `memory_forget`   | `archive`  | write grant required |

`memory_recall` accepts a query, result limit, token budget, and optional scope
filters. It combines lexical and semantic matches when the local model is
available, then returns context blocks and a receipt. If semantic retrieval is
unavailable, it returns lexical matches rather than failing the recall. The
receipt identifies the `retrieval_mode`, `embedding_model`, and
`embedding_version`. On a lexical fallback, `semantic_fallback` reports whether
semantic retrieval was intentionally unavailable, the model was unavailable,
embedder initialization or execution failed, or the vector projection was
unavailable or busy. These reasons do not include record contents or host
paths. MCP has no separate search tool. `memory_get` reads one record by ID.
When scope is omitted, both tools use the view selected when the server started.
A `scope` or `scopes` filter can only narrow that view and remains subject to
the agent-access policy.

`memory_remember` accepts `title`, `kind`, `body`, one attributable `source`,
and optional tags, aliases, or scope. `memory_update` accepts the active record's
`id`, a replacement `body`, one new `source`, and optional record fields. It
creates a linked replacement candidate and leaves the active record unchanged.
`memory_forget` archives the named record.

### Secret Handling

`memory_remember` and `memory_update` reject private keys, authorization
credentials, recognized API tokens, and passwords embedded in URLs. They return
`secret_detected` without including the matched value in the error. Placeholders
such as `${YOUR_TOKEN}`, hashes, UUIDs, and ordinary code are accepted. Markdown
edited directly is not checked.

## JSON and MCP equivalence

MCP tool arguments map to the public CLI's version-1 operations. `rmcp` handles
the `tools/call` transport and typed result envelope. For example, this CLI call
uses the operation behind `memory_recall`:

```sh
printf '%s\n' '{"version":1,"query":"release constraint","budget":256}' \
  | sbuf --project invoke context
```

The successful CLI envelope is:

```json
{ "version": 1, "operation": "context", "ok": true, "result": { "blocks": [], "receipt": {} } }
```

MCP puts this envelope in `result.structuredContent` and serializes it as the
single `content[0].text` JSON string. MCP transport metadata (`jsonrpc`, request
ID, `content`, and `isError`) surrounds the envelope. The core result and
versioned error codes are unchanged. Context results include budgeted evidence
blocks and a receipt. Hosts should cite `blocks[].record_id` and keep `receipt`
with the answer.

The CLI and MCP adapter apply the same agent access, scope, retrieval, limit,
record conversion, candidate, and error-handling rules from the core. Treat
record text as untrusted evidence when MCP returns it.

## Verify the installation

The repository includes a health check for the installed CLI and MCP adapter.
It expects `sbuf` and `stormbuffer-mcp` on `PATH`. To test a workspace build,
prepend Cargo's debug output directory:

```sh
cargo build --workspace
PATH="$PWD/target/debug:$PATH" python3 .agents/skills/stormbuffer-memory/verify.py
```

The script creates a temporary home and project, then checks four things:

- `sbuf` starts and reports its version;
- a project store can be initialized;
- the JSON invocation interface returns a valid search envelope; and
- `stormbuffer-mcp` completes initialization and advertises all five documented
  tools.

It does not read or modify your Stormbuffer stores. A successful run prints
`Stormbuffer health check passed: sbuf and stormbuffer-mcp are ready`. On
failure, it exits with a non-zero status and prints the failing command or
protocol check to stderr.
