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

Install Rust, clone the repository, and install both programs from the workspace root:

```sh
git clone https://github.com/stormlightlabs/stormbuffer.git
cd stormbuffer
cargo install --path crates/cli --locked
cargo install --path crates/mcp --locked
```

These commands install the programs. Man pages and shell completions are available in the
GitHub release archives. See [MCP](/docs/reference/mcp/) to connect the source-installed
adapter to Codex, Pi, or another MCP host.

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

## Choose a store

Stormbuffer uses a global store by default. Add `--project` to use the nearest
`.sbuf/` directory instead:

```sh
sbuf --project init
sbuf --project status
```

If project memory should stay out of version control, add `.sbuf/` to the project's ignore
rules before creating the store:

```sh
printf '%s\n' '.sbuf/' >> .gitignore
```

## Upgrade

Installing a release replaces programs and support files, not stores. Before an upgrade,
locate the selected store and create a JSON backup. For example, for a project store:

```sh
sbuf --project root
sbuf --project export stormbuffer-backup.json
```

Replace the installed programs, then run `sbuf --project status` and
`sbuf --project doctor`. Run `sbuf --project sync` if the disposable index needs to be
rebuilt. Omit `--project` from these commands when upgrading the global store.

## Roll back

To roll back, restore the previous programs and repeat those checks. Read the newer release
notes before rolling back across a record-format change.

## Uninstall

Remove `sbuf`, `stormbuffer-mcp`, and any man pages or completions you installed. Removing
the programs does not remove canonical records. `sbuf root` reports the global data location;
project data is stored in `.sbuf/`.

Delete either location only when you separately intend to delete those records.
The cached embedding model is disposable and can be removed independently.
