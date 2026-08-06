# Releasing Stormbuffer

Build and check each release archive on its target platform:

| Target              | Build host                  | Archive   |
| ------------------- | --------------------------- | --------- |
| Linux x86-64        | Supported x86-64 Linux host | `.tar.gz` |
| macOS x86-64        | Intel Mac                   | `.tar.gz` |
| macOS Apple silicon | Apple silicon Mac           | `.tar.gz` |
| Windows x86-64      | x86-64 Windows host         | `.zip`    |

Each archive contains `stormbuffer`, `stormbuf`, `sbuf`, `stormbuffer-mcp`, the
license, release and project documentation, man pages, and shell completions. A
SHA-256 file accompanies every archive.

## Prepare the release

1. Set the workspace version in `Cargo.toml`. The CLI and MCP adapter read this
   value at build time.
2. Update `Cargo.lock`, user-facing documentation, and release notes for any
   changed behavior. Call out changes to record format or rollback support.
3. Run the repository checks listed in `AGENTS.md`. Package
   `stormbuffer-core` with `cargo package --locked -p stormbuffer-core`. Before
   the first crates.io publication, Cargo can list the adapters' package
   contents but cannot fully package them until `stormbuffer-core` exists in the
   registry.
4. On each build host, replace `<TARGET>` and `<VERSION>` below, then build,
   package, and check the archive:

   ```sh
   rustup target add <TARGET>
   cargo build --locked --release --target <TARGET> -p stormbuffer -p stormbuffer-mcp
   python3 scripts/release/pkg.py --target <TARGET> --version <VERSION>
   python3 scripts/release/check.py dist/stormbuffer-<VERSION>-<TARGET>.<EXTENSION> --version <VERSION>
   ```

   On Windows, use `py -3` in place of `python3` if Python was installed with
   the standard launcher.

   Record the Linux distribution and version used to build the GNU archive in
   the release notes; that host establishes its minimum libc baseline.

The archive check verifies its SHA-256 file, required documentation, man pages,
completions, all three CLI names, and the MCP protocol surface. It initializes a
project store, creates and syncs a canonical record, removes the unpacked
programs, and confirms that the record is unchanged.

## Create the GitHub Release

Create and push an annotated tag named `v<VERSION>`, such as `v0.1.0`. The tag
version must exactly match the workspace version. In GitHub, create a draft
release from that tag, write the release notes, and manually upload all supported
archives and checksum files from `dist/`.

Before publishing the draft:

1. Download one archive and verify its SHA-256 checksum.
2. Confirm the draft contains four archives and four checksum files.
3. Add any migration, model-cache, platform baseline, or rollback guidance users
   need.
4. Publish the draft only after every uploaded archive has passed
   `scripts/release/check.py` on its target platform.

## Publish to crates.io

GitHub archives are the primary binary release. crates.io publication is
optional and uses this order because the adapters depend on the core crate:

```sh
cargo publish --dry-run -p stormbuffer-core
cargo publish -p stormbuffer-core

# Wait until stormbuffer-core is available from the crates.io index.
cargo publish --dry-run -p stormbuffer
cargo publish --dry-run -p stormbuffer-mcp
cargo publish -p stormbuffer
cargo publish -p stormbuffer-mcp
```

`stormbuffer-server` is unfinished and has `publish = false`. Do not publish it.
Never reuse a published version. If publication fails after the core crate is
live, fix the remaining package and publish it with the same version only when
its contents have not already been accepted by crates.io.

## Upgrade, rollback, and uninstall

Installing or replacing Stormbuffer changes only program and support files. It
does not move, rewrite, or delete canonical Markdown. Before an upgrade, record
the store location with `stormbuffer root` and create a portable backup:

```sh
stormbuffer export stormbuffer-backup.json
stormbuffer status
```

After replacing the programs, run `stormbuffer status` and `stormbuffer doctor`.
Run `stormbuffer sync` if the disposable projection needs rebuilding. To roll
back, replace the programs with the previous release and repeat those checks.
Only roll back across a record-format change when the newer release notes say
the older release can still read the records.

To uninstall, remove the four programs and any installed man pages or shell
completions. Do not remove the path reported by `stormbuffer root` or a project's
`.sbuf/` directory unless the user separately intends to delete that data. The
downloaded embedding model is a disposable cache and may be removed separately.
