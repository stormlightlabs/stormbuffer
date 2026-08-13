---
title: Backup and Recovery
description: Export memories, rebuild indexes, and choose what's shared.
section: Reference
group: Workflows
order: 8
---

Stormbuffer treats canonical Markdown as the backup boundary. An export contains the full
record Markdown, including source references and lifecycle metadata. SQLite, full-text and vector
projections, model downloads, locks, logs, and temporary files are disposable.

## Exporting and Moving

Initialize a project store before exporting it:

```sh
sbuf --project init --shared
```

Export the selected store to a JSON archive:

```sh
sbuf --project export stormbuffer-memory.json
```

Verify the archive before moving or relying on it:

```sh
sbuf --project verify-export stormbuffer-memory.json
```

The archive does not contain the host's absolute paths. Copy it using your normal backup or
transfer process. To import it into another store, choose a policy when the stores do not have the
same scope:

```sh
sbuf --project import stormbuffer-memory.json --on-scope remap
```

A scope remap changes `project:<project-id>` to the selected project's stable
scope. Imports preserve record IDs and Markdown when the selected policy does
not require a change.

Preview the resolved scope, destination, and collision action before importing:

```sh
sbuf --project import stormbuffer-memory.json --on-scope remap --dry-run
```

The preview validates the archive but writes no records or projections.

## Collisions

Stormbuffer rejects collisions unless you select a policy. Use these options after reviewing the
archive:

- `--on-scope fail|skip|remap` handles records from another global or project scope.
- `--on-id fail|skip|overwrite|remap` handles a different record with the same ID.
- `--on-existing fail|skip|overwrite` handles an equivalent record already in the store.

There is no automatic merge. `overwrite` replaces the selected canonical record. `skip` leaves it
alone. `remap` assigns a new ID and updates supersession links inside the imported set. Back up the
canonical files before using overwrite.

## Recovery

Restore `.sbuf/store.toml`, `.sbuf/.gitignore`, and `.sbuf/records/` (or import
an export archive), then rebuild projections. `store.toml` contains the stable
project identity and belongs in every project-store backup:

```sh
sbuf --project sync
sbuf --project reindex
```

## Garbage Collection

Inspect disposable data before removing it:

```sh
sbuf --project gc --dry-run
sbuf --project gc
```

`gc` only considers known indexes, model-cache files, locks, logs, and temporary files. It never
removes `store.toml`, `.gitignore`, or Markdown records. A dry run does not change anything.

## Starting Over

Use `destroy-store` only when you intend to remove the entire selected store. It
prints the stable store identity, the store root's total size, and the affected
canonical and disposable byte counts before asking for confirmation. Everything
under the printed store root is removed, including files Stormbuffer does not
recognize:

```text
sbuf --project destroy-store
```

For scripts, pass both the stable ID shown by the preview and `--yes`. Add
`--export` to create a verified recovery archive before deletion:

```text
sbuf --project destroy-store --store-id <project-id> --yes --export stormbuffer-before-destroy.json
```

A wrong or missing ID stops noninteractive destruction. For the global store,
the stable ID is `global`. Destroying it removes the global projection but keeps
the shared model cache used by project stores.

## Sharing and Merging

Global stores live outside the repository. For project stores, add `.sbuf/` to the repository's
ignore rules before initialization when the memory should stay out of version control. Initialize
with `sbuf --project init --shared` when the repository should carry a curated memory set.
Shared records are visible to anyone who can read the repository, so do not store secrets, raw
transcripts, credentials, or personal notes in them. Review source references before committing.

When branches change shared records, merge the Markdown files as canonical text, preserve both
claims when they conflict, and supersede one claim only after resolving the conflict.
Re-run `sync` after the merge. Do not commit generated indexes or model files.

If the repository already shares a store, leave its tracked canonical files intact. Use the global
store for personal memory, or a separate checkout when the memory must be project-scoped.
