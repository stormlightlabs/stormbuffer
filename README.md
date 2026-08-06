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
`.stormbuffer/` directory instead:

```sh
stormbuffer|stormbuf|sbuf --project init
stormbuffer|stormbuf|sbuf --project root
stormbuffer|stormbuf|sbuf --project status
stormbuffer|stormbuf|sbuf status --json
```
