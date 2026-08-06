---
title: Installation
description: Build Stormbuffer from a source checkout and verify the command-line interface.
section: Get started
group: Get started
order: 1
---

Build Stormbuffer from a source checkout, then use the CLI to initialize and inspect a store.

## Build from a checkout

Install Rust, then build the workspace from its root:

```sh
cargo build --workspace
```

## Confirm the installation

Run the CLI version and help commands from the repository root:

```sh
cargo run -p stormbuffer -- --version
cargo run -p stormbuffer -- --help
```

The installed command names are `stormbuffer`, `stormbuf`, and `sbuf`. They share the same commands and options. Use the name on your system when following the [CLI reference](/docs/cli/reference/).

## Choose a store

Stormbuffer uses a global store by default. Add `--project` to use the nearest `.stormbuffer/` directory instead:

```sh
cargo run -p stormbuffer -- --project init
cargo run -p stormbuffer -- --project status
```

Project stores are private by default. Add `.stormbuffer/` to the project’s ignore rules before creating project memory:

```sh
printf '%s\n' '.stormbuffer/' >> .gitignore
```
