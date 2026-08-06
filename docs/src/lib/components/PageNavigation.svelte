<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Doc } from '$lib/content/types';

	let { previous, next }: { previous?: Doc; next?: Doc } = $props();
</script>

<nav class="page-navigation" aria-label="Page navigation" data-pagefind-ignore>
	{#if previous}
		<a class="page-nav-link previous" href={resolve(`/docs/${previous.slug}/`)}>
			<span>Previous</span>
			<strong><span class="i-ri-arrow-left-line" aria-hidden="true"></span>{previous.title}</strong>
		</a>
	{:else}<span></span>{/if}
	{#if next}
		<a class="page-nav-link next" href={resolve(`/docs/${next.slug}/`)}>
			<span>Next</span>
			<strong>{next.title}<span class="i-ri-arrow-right-line" aria-hidden="true"></span></strong>
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

	.page-nav-link > span {
		color: var(--muted);
		font-family: 'Google Sans Code Variable', 'Google Sans Code', monospace;
		font-size: 0.7rem;
		text-transform: uppercase;
	}

	.page-nav-link strong {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		color: var(--teal-dark);
		font-family: 'Google Sans Variable', 'Google Sans', sans-serif;
		font-size: 1.05rem;
	}

	.page-nav-link.next {
		text-align: right;
	}

	.page-nav-link.next strong {
		justify-content: flex-end;
	}

	@media (max-width: 520px) {
		.page-navigation {
			grid-template-columns: 1fr;
		}

		.page-nav-link.next {
			text-align: left;
		}

		.page-nav-link.next strong {
			justify-content: flex-start;
		}
	}
</style>
