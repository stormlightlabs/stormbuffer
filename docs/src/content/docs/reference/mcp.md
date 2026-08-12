---
title: MCP
description: Run the Stormbuffer MCP adapter over JSON-RPC stdio.
section: Reference
group: Agents
order: 4
---

`stormbuffer-mcp` is a local JSON-RPC 2.0 adapter over stdio built with the
[official MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) (`rmcp = 2.1.0`,
server and stdio transport features). It calls the public core repository and retrieval
operations. It does not open SQLite, edit arbitrary files, or run a model.

## Before connecting

Install Stormbuffer and initialize the store you want the adapter to use. See
[Installation](/docs/installation/) and the [quick start](/docs/quick-start/).
Connecting an MCP host never initializes a store or creates records.

## Choose access and store scope

The examples below are recall-only. Writes are disabled unless the host starts the adapter
with `--allow-writes`. That grant enables only `memory_remember`, `memory_update`, and
`memory_forget`. Remember and update still create candidates that require human approval.
There is no MCP approval, restore, reindex, raw SQL, arbitrary-file, or permanent-deletion
operation.

Without `--project`, the adapter uses the machine's global Stormbuffer store. Add
`--project` to select the nearest `.sbuf/` store, and start the host from that project.
The global and project stores remain separate; connecting MCP does not move or rewrite
their canonical Markdown.

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

Choose one registration for the `stormbuffer` name, and start Codex from the initialized
project when using `--project`. Add `--allow-writes` after `--stdio` only when Codex should
be able to create candidates or archive records. Codex stores MCP configuration in
`~/.codex/config.toml`; trusted projects can also use `.codex/config.toml`. See the
[Codex MCP documentation](https://developers.openai.com/codex/mcp) for configuration fields
and host controls. In an interactive Codex session, use `/mcp` to inspect the connection.

## Connect Pi

Install [pi-mcp-adapter](https://github.com/nicobailon/pi-mcp-adapter), then restart Pi:

```sh
pi install npm:pi-mcp-adapter
```

For a project store, create `.mcp.json` in the initialized project:

```json
{ "mcpServers": { "stormbuffer": { "command": "stormbuffer-mcp", "args": ["--stdio", "--project"] } } }
```

Run Pi from that project. To use the global store, remove `"--project"` from `args` and
put the configuration in `~/.config/mcp/mcp.json`. Add `"--allow-writes"` to `args` only
when Pi should have write access.

The adapter's default lazy mode exposes one proxy tool and discovers Stormbuffer's tools
on demand. Leave `directTools` unset to keep MCP metadata from occupying unnecessary agent
context. Use `/mcp` in Pi to inspect server status and available tools.

## Connect another host

A host that accepts the common `mcpServers` configuration shape can start a project-scoped,
recall-only adapter like this:

```json
{ "mcpServers": { "stormbuffer": { "command": "stormbuffer-mcp", "args": ["--stdio", "--project"] } } }
```

Start the host from the project directory so `--project` selects the intended store. Add
`--allow-writes` only when the host should have write access.

## JSON-RPC lifecycle

The host sends `initialize`, then the `notifications/initialized` notification. The official
SDK owns JSON-RPC parsing, stdio framing, request dispatch, cancellation notifications, and
connection cleanup. Close the adapter's stdin to shut down the process. The handler rejects
a request when the SDK cancellation context is already cancelled. Core store operations are
synchronous and atomic, so cancellation does not interrupt one after it has begun.

Stormbuffer limits query, record, scope, budget, URI, and tool/resource output sizes. Malformed
JSON and invalid method parameters are rejected by `rmcp`. Tool-level failures use the versioned
Stormbuffer envelope and sanitized messages. Error messages never include canonical paths or
backtraces.

## Resources

The adapter advertises these URI templates through the SDK's
`resources/templates/list` handler:

| URI template                          | Contents                                                            |
| ------------------------------------- | ------------------------------------------------------------------- |
| `stormbuffer://record/{id}`           | One agent-readable record as JSON.                                  |
| `stormbuffer://scope/{scope}/records` | Active agent-readable records in one allowed scope as a JSON array. |
| `stormbuffer://candidate/{id}`        | One agent-readable candidate as JSON.                               |

Use `resources/read` with one of those URIs. A scope must be `global` or an allowed
`project:<name>` scope. Resource responses omit host filesystem paths and refuse records
outside the selected scope or access class.

## Tools

`tools/list` returns exactly these tools:

| Tool              | Operation  | Default              |
| ----------------- | ---------- | -------------------- |
| `memory_recall`   | `context`  | enabled              |
| `memory_get`      | `get`      | enabled              |
| `memory_remember` | `remember` | write grant required |
| `memory_update`   | `update`   | write grant required |
| `memory_forget`   | `archive`  | write grant required |

`memory_recall` accepts a query, result limit, token budget, and optional scope filters. It
returns the core context blocks and receipt; MCP has no separate search tool. `memory_get`
reads one record by ID. When scope is omitted, both use the store selected when the server
started. Explicit `scope` or `scopes` filters remain within the core store and agent-access
policy.

`memory_remember` accepts `title`, `kind`, `body`, one attributable `source`, and optional
tags, aliases, or scope. `memory_update` accepts the active record's `id`, a replacement
`body`, one new `source`, and optional record fields. It creates a linked replacement
candidate without changing the active record. `memory_forget` archives the named record.

## JSON and MCP equivalence

MCP tool arguments are mapped to the public CLI's version-1 operation contract.
`rmcp` supplies the `tools/call` transport and typed result envelope. Stormbuffer supplies
only this operation mapping and core call:

```sh
printf '%s\n' '{"version":1,"query":"release constraint","budget":256}' \
  | sbuf --project invoke context
```

The successful CLI envelope is:

```json
{ "version": 1, "operation": "context", "ok": true, "result": { "blocks": [], "receipt": {} } }
```

MCP puts the CLI envelope in `result.structuredContent` and serializes it
as the single `content[0].text` JSON string. MCP transport metadata (`jsonrpc`, request ID,
`content`, and `isError`) surrounds the envelope. The core result and versioned error codes do
not change. Context results retain their budgeted evidence blocks and receipt, so hosts cite
`blocks[].record_id` and retain `receipt` with the answer.

Both boundaries apply the core's agent access and scope rules, lexical retrieval mode, limits,
record conversion, candidate policy, and sanitized error vocabulary. Treat record text as
untrusted evidence even when MCP returns it.

Run the checked-in public-interface smoke test with the built binaries:

```sh
python3 .agents/skills/stormbuffer-memory/verify.py
```

The test exercises candidate approval through the CLI and reads a cited context receipt through
the SDK-backed `memory_recall` tool.
