<script lang="ts">
	import { resolve } from '$app/paths';

	let { markdown, slug }: { markdown: string; slug: string } = $props();
	let label = $state('Copy Markdown');

	async function copy(event: MouseEvent): Promise<void> {
		event.preventDefault();
		const rawUrl = (event.currentTarget as HTMLAnchorElement).href;
		try {
			await navigator.clipboard.writeText(markdown);
			label = 'Copied';
			window.setTimeout(() => (label = 'Copy Markdown'), 1600);
		} catch {
			window.location.assign(rawUrl);
		}
	}
</script>

<a class="copy-markdown" href={resolve('/docs/[...slug].md', { slug })} onclick={copy} aria-live="polite">
	<span class="i-ri-file-copy-line" aria-hidden="true"></span>
	<span>{label}</span>
</a>

<style>
	.copy-markdown {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		min-height: 2.5rem;
		padding: 0.45rem 0.75rem;
		border: 1px solid var(--line);
		border-radius: 0.25rem;
		background: var(--surface-raised);
		color: var(--teal-dark);
		font-size: 0.82rem;
		font-weight: 650;
		text-decoration: none;
	}

	.copy-markdown:hover {
		border-color: var(--teal);
		background: var(--paper-deep);
	}
</style>
