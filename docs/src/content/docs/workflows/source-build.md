---
title: Build from source
description: Build and install Stormbuffer from source.
section: Get started
group: Get started
order: 3
---

Build from source when you want the current development version or need to
modify Stormbuffer locally. You'll need Git and a stable Rust toolchain.

## Install the current checkout

Clone the repository and install both programs:

```sh
git clone https://github.com/stormlightlabs/stormbuffer.git
cd stormbuffer
cargo install --path crates/cli --locked
cargo install --path crates/mcp --locked
```

Cargo normally installs them in `~/.cargo/bin`. Make sure that directory is on
`PATH`, then verify the installation:

```sh
command -v sbuf
command -v stormbuffer-mcp
sbuf --version
stormbuffer-mcp --version
```

After pulling new source, rerun the two `cargo install` commands. Cargo replaces
the installed programs; it does not move or rewrite your stores.

Continue with the [quick start](/docs/quick-start/) to initialize a store. To
connect an agent, see the [MCP reference](/docs/reference/mcp/) or
[agent skill](/docs/workflows/agent-skill/).
