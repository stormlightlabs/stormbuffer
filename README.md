# Stormbuffer

Stormbuffer is a local-first memory store for devs & agents to hold sourced
facts, decisions, procedures, and project checkpoints, written in Rust.

## Development CLI

Build and run the primary CLI with Cargo:

```sh
cargo run -p stormbuffer -- --help
cargo run -p stormbuffer -- --version
```

Stores are global by default. Add `--project` to use the nearest
`.sbuf/` directory instead:

```sh
sbuf --project init
sbuf --project root
sbuf --project status
sbuf status --json
```

The documentation covers the workflows that are easy to get wrong:

- [Quick start](docs/src/content/docs/quick-start.md) for private and shared project stores.
- [Backup and recovery](docs/src/content/docs/workflows/backup-recovery.md) for portable
  export/import, collision policies, garbage collection, privacy, and merges.
- [MCP reference](docs/src/content/docs/reference/mcp.md) for the official Rust SDK adapter,
  exact resources and tools, write grants, and the CLI JSON equivalence contract.

The checked-in agent skill uses public interfaces only. Build the CLI and MCP binary, then run
its end-to-end verification:

```sh
cargo build -p stormbuffer -p stormbuffer-mcp
python3 .agents/skills/stormbuffer-memory/verify.py
```
