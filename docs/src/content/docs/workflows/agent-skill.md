---
title: Agent skill
description: Give an agent the Stormbuffer workflow while keeping memory changes visible.
section: Reference
group: Agent setup
order: 7
---

<script>
  import CopySkill from '$lib/components/CopySkill.svelte';
</script>

The Stormbuffer skill teaches a coding agent to retrieve project memory, cite what it used,
and propose sourced memories for human approval. The copy below comes from the canonical
skill shipped in this repository.

<CopySkill />

Place the copied `SKILL.md` in the skill directory used by your agent.

The skill expects `sbuf` to be installed and a store to be initialized.

## Verification

From the project where the agent will run:

```sh
sbuf --project init
sbuf --project status
sbuf --project search "project conventions" --json
```

`status` should identify the intended project store. The search should return an empty JSON array
without prompting when the store has no matching memories.
