---
title: Contributing
description: Keep public behavior, examples, and operator artifacts aligned.
section: Reference
group: Contributors
order: 7
version: '0.1'
toc:
  - title: Documentation changes
    slug: documentation-changes
    level: 2
  - title: Checks and artifacts
    slug: checks-and-artifacts
    level: 2
---

# Contributing

## Documentation changes

Documentation is part of every public change. Update it when a change adds,
removes, renames, or changes a command, option, argument, alias, exit status,
output field, configuration key, record field, protocol operation, or user
workflow. Update examples when command syntax or result shapes change.

Navigation, versions, frontmatter, man pages, and shell completions also need a
docs review.

## Checks and artifacts

After changing the shared Clap definition, build the workspace, run the
example smoke test, and check the docs:

```sh
cargo build --workspace
cargo test -p stormbuffer --test documented_examples
pnpm --dir docs check
```

The workspace build runs `crates/cli/build.rs`, which writes man pages and
completions to the repository-root `assets/man/` and `assets/completions/`
directories. These generated outputs are disposable and gitignored. Do not
create crate-local asset directories.
