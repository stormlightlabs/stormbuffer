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

## Start the adapter

Build the standalone adapter from the repository root:

```sh
cargo build -p stormbuffer-mcp
```

Run it from the project whose memory the host should use:

```sh
stormbuffer-mcp --stdio --project
```

`--project` selects the nearest `.sbuf/` store. Otherwise, the adapter selects the global
store. Initialize a store with the CLI before starting MCP. MCP never initializes a store
or creates canonical records as a side effect of connecting.

Writes are disabled by default. A host operator enables them when starting the process:

```text
stormbuffer-mcp --stdio --project --allow-writes
```

That grant enables only `propose`, `supersede`, and `archive`. Proposals still become
candidates and require human approval. There is no MCP approval, restore, edit, reindex,
raw SQL, arbitrary-file, or permanent-deletion operation.

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

| Tool                    | Operation   | Default              |
| ----------------------- | ----------- | -------------------- |
| `stormbuffer_search`    | `search`    | enabled              |
| `stormbuffer_context`   | `context`   | enabled              |
| `stormbuffer_get`       | `get`       | enabled              |
| `stormbuffer_propose`   | `propose`   | write grant required |
| `stormbuffer_supersede` | `supersede` | write grant required |
| `stormbuffer_archive`   | `archive`   | write grant required |

Read tools accept the version-1 invoke fields `query`, `id`, `limit`, `budget`, `scope`,
`scopes`, and `access`. `access` is agent-only. Proposal and supersession fields follow
the record contract: `title`, `kind`, `scope`, `access`, `body`, `tags`, `aliases`,
`supersedes`, and attributable `sources`.

## JSON and MCP equivalence

MCP tool arguments are mapped to the public CLI's version-1 operation contract.
`rmcp` supplies the `tools/call` transport and typed result envelope. Stormbuffer supplies
only this operation mapping and core call:

```sh
printf '%s\n' '{"version":1,"query":"release constraint","limit":5}' \
  | sbuf --project invoke search
```

The successful CLI envelope is:

```json
{ "version": 1, "operation": "search", "ok": true, "result": [{ "record_id": "..." }] }
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

The test exercises candidate approval through the CLI, cites a context receipt, and reads a
search result through the SDK-backed MCP process.
