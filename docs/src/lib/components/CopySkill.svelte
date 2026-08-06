<script lang="ts">
	import { resolve } from '$app/paths';
	import { skillSource } from '$lib/content/skill';

	let label = $state('Copy skill');

	async function copy(): Promise<void> {
		try {
			await navigator.clipboard.writeText(skillSource);
			label = 'Copied';
		} catch {
			label = 'Copy failed';
		}
		window.setTimeout(() => (label = 'Copy skill'), 1600);
	}
</script>

<div class="skill-actions">
	<button type="button" onclick={copy} aria-live="polite">
		<span class="i-ri-file-copy-line" aria-hidden="true"></span>
		<span>{label}</span>
	</button>
	<a href={resolve('/stormbuffer-memory-skill.md')}>
		<span class="i-ri-external-link-line" aria-hidden="true"></span>
		<span>View raw skill</span>
	</a>
	<a href={resolve('/stormbuffer-memory-skill.md')} download="SKILL.md">
		<span class="i-ri-download-line" aria-hidden="true"></span>
		<span>Download SKILL.md</span>
	</a>
</div>

<style>
	.skill-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.65rem;
		margin: 1rem 0 1.5rem;
	}

	button,
	a {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		min-height: 2.5rem;
		padding: 0.45rem 0.75rem;
		border: 1px solid var(--line);
		border-radius: 0.25rem;
		background: var(--surface-raised);
		color: var(--teal-dark);
		font:
			650 0.82rem/1 'IBM Plex Sans Variable',
			sans-serif;
		text-decoration: none;
		cursor: pointer;
	}

	button:hover,
	a:hover {
		border-color: var(--teal);
		background: var(--paper-deep);
	}
</style>
