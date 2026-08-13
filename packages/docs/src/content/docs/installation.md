---
title: Installation
description: Install and manage Stormbuffer.
section: Get started
group: Get started
order: 1
---

Stormbuffer has not published a release yet. Install the current version from a
source checkout. The repository contains the `sbuf` CLI, the `stormbuffer-mcp`
adapter, lifecycle plugins for Codex and Pi, and this documentation site.

## Install from source

Clone the repository and install the command-line programs with Cargo:

```sh
git clone https://github.com/stormlightlabs/stormbuffer.git
cd stormbuffer
cargo install --path crates/cli --locked
cargo install --path crates/mcp --locked
```

The [source build guide](/docs/workflows/source-build/) covers prerequisites,
updates, and the optional JavaScript workspace used by the host plugins.

## Confirm the installation

```sh
sbuf --version
sbuf --help
stormbuffer-mcp --version
```

## Online and offline model behavior

Installation does not download a model. Stormbuffer acquires its pinned local
embedding model when a global store is initialized or retrieval first needs it.

Stormbuffer verifies the model checksum before use and stores the model in the platform cache.

| Environment                      | Behavior                                                                                                                                           |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Online                           | Store operations work, and the verified model is downloaded when semantic retrieval needs it.                                                      |
| Offline with the model cached    | Store operations and hybrid retrieval work without network access.                                                                                 |
| Offline without the model cached | Project initialization and canonical record operations work. Retrieval reports that the verified model is unavailable until the machine is online. |

If a global `init` cannot acquire the model, it still initializes the store and
does not change its canonical Markdown. Re-run `sbuf init` after network access
is restored.

Continue with the [quick start](/docs/quick-start/) to choose and initialize a
store. To add prompt-time recall, install the
[Codex plugin](/docs/workflows/codex-plugin/) or
[Pi plugin](/docs/workflows/pi-plugin/) from the same checkout.

## Upgrade

Before updating, export the selected store as described in
[Backup and recovery](/docs/workflows/backup-recovery/). Pull the newer source,
repeat the Cargo install commands, then run `sbuf status` and `sbuf doctor`. Add
`--project` when checking a project store.

## Rollback

Check out the previous source revision, reinstall both programs, and repeat the
status and doctor checks. Read the newer changelog entry before rolling back
across a record-format change.

## Uninstall

Remove `sbuf`, `stormbuffer-mcp`, and any man pages or completions you installed.
This leaves stores and their records in place. Use `sbuf root` before uninstalling
if you need to locate the global store; project stores live in `.sbuf/`.
