---
title: Backup & Recovery
description: Export canonical memory, rebuild disposable data, and choose what a project shares.
section: Reference
group: Workflows
order: 8
---

Stormbuffer treats canonical Markdown as the backup boundary. An export contains the complete
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

The archive does not contain the host's absolute paths. Copy it using your normal backup or
transfer process. To import it into another store, choose a policy when the stores do not have the
same scope:

```sh
sbuf --project import stormbuffer-memory.json --on-scope remap
```

A scope remap changes `project:<name>` to the selected project's scope. It is explicit because it
changes record metadata. Imports preserve IDs and Markdown when no policy requires a change.

## Collisions

Stormbuffer stops instead of guessing when an import meets existing data. Use these options only
after reviewing the archive:

- `--on-scope fail|skip|remap` handles records from another global or project scope.
- `--on-id fail|skip|overwrite|remap` handles a different record with the same ID.
- `--on-existing fail|skip|overwrite` handles an equivalent record already in the store.

There is no automatic merge. `overwrite` replaces the selected canonical record; `skip` leaves it
alone. `remap` assigns a new ID and updates supersession links inside the imported set. Keep a copy
of the canonical files before using overwrite.

## Recovery

Restore `.sbuf/store.toml`, `.sbuf/.gitignore`, and `.sbuf/records/` (or import an export archive),
then rebuild projections:

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

## Privacy and Merging

Global stores are private user memory. Project stores are private by default; initialize with
`sbuf --project init --shared` only when the repository should carry a curated memory set.
Shared records are visible to anyone who can read the repository, so do not store secrets, raw
transcripts, credentials, or personal notes in them. Review source references before committing.

When branches change shared records, merge the Markdown files as canonical text, preserve both
claims when they conflict, and use an explicit supersession rather than silently choosing one.
Re-run `sync` after the merge. Do not commit generated indexes or model files.

Choose private project memory before initialization by omitting `--shared` and ignoring `.sbuf/`
under that repository's normal ignore policy. If the repository already shares a store, leave its
tracked canonical files intact. Use the global store for personal memory, or a separate private
checkout when the memory must remain project-scoped.
