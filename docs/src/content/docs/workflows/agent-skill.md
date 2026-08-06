---
title: Agent skill
description: Give an agent the Stormbuffer workflow without hiding memory changes from you.
section: Reference
group: Agent setup
order: 7
---

<script>
  import CopySkill from '$lib/components/CopySkill.svelte';
</script>

The Stormbuffer skill teaches a coding agent to retrieve project memory, cite what it used,
and propose small changes for human approval. The copy below always comes from the canonical
skill shipped in this repository.

<CopySkill />

Place the copied `SKILL.md` in the skill directory used by your agent.

The skill expects the `stormbuffer` command to be installed and a store to be initialized.

## Verification

From the project where the agent will run:

```sh
stormbuffer --project init
stormbuffer --project status
stormbuffer --project search "project conventions" --json
```

The first command should identify the intended project store.

The second should return empty JSON without prompting, even when the store has no matching memories.
