# Release checklist

- Run the Rust workspace checks.
- Run `cargo test -p stormbuffer --test examples` and
  `pnpm --dir docs check`.
- Run `pnpm --dir docs lint`, `pnpm --dir docs test`, and `pnpm --dir docs build`.
- Confirm man pages are in root `assets/man/` and completions are in root
  `assets/completions/`; both are generated and gitignored.
- Exercise `stormbuffer`, `stormbuf`, and `sbuf` in the packaged build.
- Read changed docs as a person: verify prose describes shipped behavior,
  examples, version labels, breadcrumbs, sidebar order, previous/next links,
  and search.
- Check the built site at desktop and narrow widths with JavaScript disabled.
