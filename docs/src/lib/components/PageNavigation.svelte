<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Doc } from '$lib/content/types';

	let { previous, next }: { previous?: Doc; next?: Doc } = $props();
</script>

<nav class="page-navigation" aria-label="Page navigation" data-pagefind-ignore>
	{#if previous}
		<a class="page-nav-link previous" href={resolve(`/docs/${previous.slug}/`)}>
			<span>Previous</span>
			<strong>← {previous.title}</strong>
		</a>
	{:else}<span></span>{/if}
	{#if next}
		<a class="page-nav-link next" href={resolve(`/docs/${next.slug}/`)}>
			<span>Next</span>
			<strong>{next.title} →</strong>
		</a>
	{/if}
</nav>

<style>
	.page-navigation {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
		margin-top: 4rem;
		padding-top: 1rem;
		border-top: 1px solid var(--line);
	}

	.page-nav-link {
		display: grid;
		gap: 0.25rem;
		padding: 0.9rem;
		border: 1px solid var(--line);
		background: var(--surface-raised);
		text-decoration: none;
	}

	.page-nav-link:hover {
		border-color: var(--teal);
		box-shadow: 0.25rem 0.25rem 0 var(--yellow);
	}

	.page-nav-link span {
		color: var(--muted);
		font-family: 'IBM Plex Mono Variable', 'JetBrains Mono Variable', monospace;
		font-size: 0.7rem;
		text-transform: uppercase;
	}

	.page-nav-link strong {
		color: var(--teal-dark);
		font-family: 'IBM Plex Serif', Georgia, serif;
		font-size: 1.05rem;
	}

	.page-nav-link.next {
		text-align: right;
	}

	@media (max-width: 520px) {
		.page-navigation {
			grid-template-columns: 1fr;
		}

		.page-nav-link.next {
			text-align: left;
		}
	}
</style>
