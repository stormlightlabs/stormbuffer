# Releasing Stormbuffer

Stormbuffer publishes native archives through GitHub Releases. Build and check
each archive on the operating system it targets.

| Target              | Build host                  | Archive   |
| ------------------- | --------------------------- | --------- |
| Linux x86-64        | Supported x86-64 Linux host | `.tar.gz` |
| macOS x86-64        | Intel Mac                   | `.tar.gz` |
| macOS Apple silicon | Apple silicon Mac           | `.tar.gz` |
| Windows x86-64      | x86-64 Windows host         | `.zip`    |

## 1. Prepare

1. Set the workspace version in `Cargo.toml`. The CLI and MCP adapter read this
   value at build time.
2. Update `Cargo.lock`, user-facing documentation, and release notes.
3. Run the checks in `AGENTS.md` and
   `cargo package --locked -p stormbuffer-core`.

## 2. Build and verify

On each build host, replace `<TARGET>`, `<VERSION>`, and `<EXTENSION>`:

```sh
rustup target add <TARGET>
cargo build --locked --release --target <TARGET> -p stormbuffer -p stormbuffer-mcp
python3 scripts/release/pkg.py --target <TARGET> --version <VERSION>
python3 scripts/release/check.py dist/stormbuffer-<VERSION>-<TARGET>.<EXTENSION> --version <VERSION>
```

Use `tar.gz` on Linux and macOS, and `zip` on Windows. The package contains
`sbuf`, `stormbuffer-mcp`, project documentation, `sbuf` man pages and shell
completions. The packaging script also writes a SHA-256 checksum. Record the
Linux build host in the release notes because it establishes the GNU libc
baseline.

## 3. Publish

1. Create and push an annotated `v<VERSION>` tag that exactly matches the
   workspace version.
2. Create a draft GitHub Release from the tag.
3. Upload all four archives and their checksum files from `dist/`.
4. Download one archive and verify its checksum.
5. Publish only after every archive passes the release check on its target
   platform.

Put migration or rollback warnings in the release notes. The
[installation guide](docs/src/content/docs/installation.md) owns the normal
upgrade, rollback, and uninstall instructions.

## Optional: publish crates

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
Never reuse a published version.
