---
title: Installation
description: Build Stormbuffer from a source checkout and verify the command-line interface.
section: Get started
group: Get started
order: 1
---

Build Stormbuffer from a source checkout, then use the CLI to initialize and inspect a store.

## Build from a checkout

Install Rust, then install the CLI from the workspace root:

```sh
cargo install --path crates/cli --locked
```

## Confirm the installation

Run the CLI version and help commands:

```sh
stormbuffer --version
stormbuffer --help
```

The installed command names are `stormbuffer`, `stormbuf`, and `sbuf`.
They share the same commands and options. Use the name on your system when following
the [CLI reference](/docs/cli/reference/).

## Choose a store

Stormbuffer uses a global store by default.

Add `--project` to use the nearest `.sbuf/` directory instead:

```sh
stormbuffer --project init
stormbuffer --project status
```

Project stores are private by default. Add `.sbuf/` to the project’s ignore rules before
creating project memory:

```sh
printf '%s\n' '.sbuf/' >> .gitignore
```
