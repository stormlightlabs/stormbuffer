# AGENTS.md

Stormbuffer is a local-first memory system written mainly in Rust, with a small
SvelteKit documentation site and a local web editor.

## Working rules

- Prefer simple, maintainable code and existing platform/library behavior.
  Correctness and recoverability matter more than clever abstractions.
- Inspect callers and shared core behavior before patching a symptom in one
  interface.
- Preserve unrelated work in the tree. The user often uses git so rely on reading
  files and keeping track of what you've changed instead of git to confirm changes
- Add dependencies only when the standard library and installed dependencies do
  not cover the need. Keep pre-1.0 dependencies pinned when their format or ABI
  affects stored data.
- No legacy or compatibility anything.

## Herdr Pi orchestration

When the user explicitly requests Herdr/Pi delegation, use the project-local
`$orchestrate` skill. Use no more than two Pi instances.

## Project memory

Use `$stormbuffer-memory` once when work depends on prior project decisions,
conventions, commands, architecture, or unfinished work. Keep retrieval project-scoped
and fail soft when Stormbuffer is unavailable or has no useful evidence.

## Architecture boundaries

- Markdown with TOML frontmatter is canonical. SQLite, FTS, vectors, generated
  output, and caches must be disposable and rebuildable.
- `stormbuffer-core` owns models, validation, policy, repository operations,
  indexing, retrieval, mutation, and context compilation.
- The CLI, JSON protocol, MCP server, and web server are adapters. They call the
  core and do not edit records/databases directly or duplicate policy.
- Keep records readable and repairable without Stormbuffer. Preserve
  user-authored Markdown when parsing and rendering.
- Use locking, temporary files, `fsync`, and atomic replacement for canonical
  writes. A projection failure must never invalidate a committed record.

## Rust

- Use stable, idiomatic Rust and the workspace's edition. Avoid `unsafe` unless
  a dependency boundary requires it and the invariant is documented and tested.
- Prefer concrete types and small modules. Add a trait at a real substitution or
  unstable dependency boundary, such as embeddings or vector storage.
- Use typed IDs/enums at domain boundaries. Attach actionable context to errors,
  but do not leak secrets, record bodies, host paths, or backtraces in normal
  user output.
  - Prefer enums to multiple constants where appropriate
- Avoid function-scoped imports, hidden global state, and panic-based error
  handling in library or expected CLI paths.
- Tests should use temporary roots and fixture stores, never the developer's
  actual data, cache, home directory, editor, or model installation.
- Verify behavior at the highest stable boundary: core integration tests for
  storage/retrieval, process tests for CLI/JSON, and protocol tests for MCP/HTTP.

## CLI and protocols

- `sbuf` is the only public CLI executable.
- Follow clig.dev conventions. Keep results on stdout, diagnostics on stderr,
  prompts TTY-only, errors actionable, and exit codes stable.
- JSON invocation is versioned, bounded, non-interactive, and JSON-only on
  stdin/stdout. Logs stay on stderr. MCP exposes the same core semantics. Keep version
  at 1 until the initial release.
- Build man pages and completions from the runtime Clap definition. Do not hand
  maintain a second command tree.
- An unfinished command may be an explicit side-effect-free stub. Never make a
  stub look successful or document it as implemented.

## Svelte and documentation

- `docs/` is a static SvelteKit/mdsvex site. Prefer server/static rendering and
  ordinary links; reading and navigation must work without client JavaScript.
- Keep components small and accessible with their styles scoped.
- Use semantic HTML before ARIA and add client state only for real interaction.

## Verification

Start with the narrowest relevant check and stop when the result is established.
Once configured, milestone/release checks are:

```sh
cargo fmt # don't bother checking
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm --dir docs check
pnpm --dir docs lint
pnpm --dir docs test
pnpm --dir docs build
```

Do not rerun unchanged checks. Do not claim commands pass when their workspace
configuration or dependencies have not been implemented yet.

## Keep this file current

Treat this as project configuration, not a static manifesto. When user feedback,
review, or implementation reveals a reusable project rule, update `AGENTS.md` in
the same change. Write the smallest concrete rule that would prevent the issue
again. Do not record one-off task details, personal data, transient workarounds,
or rules already enforced by tooling. Remove or revise instructions when the
architecture or tooling makes them false.
