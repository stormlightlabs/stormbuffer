---
title: Installation
description: Install & manage Stormbuffer.
section: Get started
group: Get started
order: 1
---

Stormbuffer ships as a packaged release for Linux x86-64, macOS on Intel or Apple
silicon, and Windows x86-64. Each archive includes the three equivalent CLI
names, the MCP adapter, man pages, shell completions, and project documentation.

## Install

Download the archive for your platform and its `.sha256` file from the GitHub
Release. On Linux or macOS, verify and unpack it:

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

Install Rust, then install the CLI from the workspace root:

```sh
cargo install --path crates/cli --locked
```

The separate MCP program can be built from the checkout with:

```sh
cargo install --path crates/mcp --locked
```

These commands install the programs. Man pages and shell completions are
included in the GitHub release archives.

## Confirm the installation

```sh
sbuf --version
sbuf --help
stormbuffer-mcp --version
```

## Online and offline model behavior

Installation does not download a model. Stormbuffer acquires its pinned local
embedding model when a global store is initialized or retrieval first needs it.

The download is checksum-verified before use and stays in the platform cache.

| Environment                      | Behavior                                                                                      |
| -------------------------------- | --------------------------------------------------------------------------------------------- |
| Online                           | Store operations work, and the verified model is downloaded when semantic retrieval needs it. |
| Offline with the model cached    | Store operations and hybrid retrieval work without network access.                            |
| Offline without the model cached | Project initialization and canonical record operations work.                                  |
|                                  | Retrieval reports that the verified model is unavailable until the machine is online.         |

If a global `init` cannot acquire the model, the store is still initialized and
its canonical Markdown remains valid. Re-run `sbuf init` after network
access is restored.

## Choose a store

Stormbuffer uses a global store by default. Add `--project` to use the nearest
`.sbuf/` directory instead:

```sh
sbuf --project init
sbuf --project status
```

Project stores are private by default. Add `.sbuf/` to the project's ignore
rules before creating project memory:

```sh
printf '%s\n' '.sbuf/' >> .gitignore
```

## Upgrade

Release installation replaces programs and support files, not stores. Before an
upgrade, locate the selected store and create a portable backup:

```sh
sbuf --project root
sbuf --project export stormbuffer-backup.json
```

Replace the installed programs, then run `sbuf --project status` and
`sbuf --project doctor`. Run `sbuf --project sync` if the
disposable index needs to be rebuilt.

## Rollback

To rollback, restore the previous programs and repeat those checks.
Read the newer release notes before rolling back across a record-format change.

## Uninstall

Remove the four programs and any man pages or completions you installed. This
does not remove canonical records. Global data will remain at the path reported by
`sbuf root` & project data will stay in `.sbuf/`.

Delete either location only when you separately intend to delete those records.
The cached embedding model is disposable and can be removed independently.
