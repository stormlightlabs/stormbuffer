---
title: SKILL.md
description: Give an agent the Stormbuffer workflow while keeping memory changes visible.
section: Reference
group: Agent setup
order: 7
---

<script>
  import CopySkill from '$lib/components/CopySkill.svelte';
</script>

The Stormbuffer skill teaches a coding agent when to retrieve memory, how to cite
what it used, and when one sourced memory candidate is worth human review.

## Install the skill

Choose the skill directory used by your agent, then run:

```sh
sbuf skill install --directory .agents/skills
```

This creates `.agents/skills/stormbuffer-global-memory/SKILL.md`. The skill selects
the global store explicitly, so it is suitable for cross-project preferences,
decisions, conventions, and procedures. It uses the same decision policy as the
project variant.

Where you install the skill and which store it reads are separate choices. You
can put this global-memory skill in a repository's local skill directory; only
that repository will load it, but it will still read the global store.

To install the project variant instead, select project scope:

```sh
sbuf --project skill install --directory .agents/skills
```

That creates `.agents/skills/stormbuffer-memory/SKILL.md` with commands that select
the nearest project store explicitly. The directory you choose determines which
agent or repository loads either skill; `sbuf` does not edit agent configuration.
Pass a different user-level or vendor-specific skill directory when that is what
your agent discovers; Stormbuffer does not guess among multiple valid locations.

Installation is offline: the skill is carried inside `sbuf`. Reinstalling the
same version is safe. If the destination contains different content, Stormbuffer
preserves it and exits with an error. Use `--force` only when you intend to
replace that file; replacement is atomic.

## Update or reinstall the skill

The installed skill is a copy bundled with the `sbuf` executable. Update `sbuf`
first, then repeat the original installation command with the same scope and
directory. Add `--force` to replace the older copy:

```sh
cargo install --path crates/cli --locked
sbuf skill install --directory .agents/skills --force
```

For a project-scoped skill, keep `--project`:

```sh
sbuf --project skill install --directory .agents/skills --force
```

Change `.agents/skills` to the directory used for the original installation.
The command replaces only the generated `SKILL.md`; it does not change memory
stores or records. Save any edits you made to the installed file before using
`--force`.

Codex detects local skill changes automatically. Restart Codex if the updated
skill does not appear. Reload or restart other agents according to their skill
discovery behavior.

## Copy the project skill from this site

The copy below comes from the canonical project skill shipped in this
repository.

<CopySkill />

Place the copied `SKILL.md` in the skill directory used by your agent.

The skill expects `sbuf` to be installed and a store to be initialized.

The downloadable skill uses `--project` for the nearest project's composed
project-and-global view.
Install it only for repositories where the agent should use project memory.

## Verification

From the project where the agent will run:

```sh
sbuf --project init
sbuf --project status
sbuf --project search "project conventions" --json
```

`status` should identify the intended project store. The search should return an
empty JSON array without prompting when the store has no matching memories.
