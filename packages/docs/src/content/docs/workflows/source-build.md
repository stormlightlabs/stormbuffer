---
title: Git
description: Build and install Stormbuffer from source.
section: Get started
group: Get started
order: 3
---

Stormbuffer is not released yet, so the current version is installed from
source. You need Git and a stable Rust toolchain. Node.js 20 or newer and pnpm
10 are also required when you want the Codex or Pi plugin.

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

## Prepare the plugin workspace

The documentation site and agent plugins live in a pnpm workspace. Install its
dependencies from the repository root:

```sh
corepack enable
pnpm install --frozen-lockfile
```

This also generates the skill bundled with each plugin from the canonical source
in `crates/cli/assets/stormbuffer-memory.md`. Edit that source only; installation,
workspace checks, and package builds keep the plugin bundles synchronized. The
project-local skill points to the same source.

This links the local plugin packages without publishing or downloading a
Stormbuffer package from a registry. Keep the checkout in a stable location:
both host installation methods refer back to files in it.

Continue with the [quick start](/docs/quick-start/) to initialize a store. To
connect an agent, install the [Codex plugin](/docs/workflows/codex-plugin/) or
[Pi plugin](/docs/workflows/pi-plugin/). The [MCP reference](/docs/reference/mcp/)
and [agent skill](/docs/workflows/agent-skill/) cover manual integrations.
