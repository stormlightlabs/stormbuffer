---
title: Installation
description: Install and manage Stormbuffer.
section: Get started
group: Get started
order: 1
---

Stormbuffer ships as a packaged release for Linux x86-64, macOS on Intel or Apple
silicon, and Windows x86-64. Each archive includes the `sbuf` CLI, the
`stormbuffer-mcp` adapter, man pages, shell completions, and project documentation.

## Install

Download the archive for your platform and its `.sha256` file from the GitHub
release. On Linux or macOS, verify and unpack it:

```sh
archive=stormbuffer-0.1.0-x86_64-unknown-linux-gnu.tar.gz
shasum -a 256 -c "$archive.sha256"
tar -xzf "$archive"
install -m 755 "${archive%.tar.gz}"/bin/* "$HOME/.local/bin/"
```

On Windows, set `$archive` to the downloaded ZIP name and compare
`Get-FileHash -Algorithm SHA256 $archive` with the value in the downloaded
checksum file. Extract the ZIP and add its `bin` directory to `PATH`. The
archive's `share` directory contains optional man pages and shell completions.

## Build from a checkout

Follow [Build from source](/docs/workflows/source-build/) to install the CLI and
MCP adapter from a checkout. Man pages and shell completions are available in
the GitHub release archives.

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
store.

## Upgrade

Before an upgrade, export the selected store as described in
[Backup and recovery](/docs/workflows/backup-recovery/). Replace the installed
programs, then run `sbuf status` and `sbuf doctor`. Add `--project` when checking
a project store.

## Roll back

Restore the previous programs and repeat the status and doctor checks. Read the
newer release notes before rolling back across a record-format change.

## Uninstall

Remove `sbuf`, `stormbuffer-mcp`, and any man pages or completions you installed.
This leaves stores and their records in place. Use `sbuf root` before uninstalling
if you need to locate the global store; project stores live in `.sbuf/`.
