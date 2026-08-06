<script lang="ts">
	import { resolve } from '$app/paths';
	import { docSections } from '$lib/content/types';
	import type { Doc } from '$lib/content/types';

	let { docs, currentSlug }: { docs: Doc[]; currentSlug: string } = $props();
</script>

<div class="sidebar-inner">
	<p class="sidebar-kicker">Documentation</p>
	{#each docSections as section (section)}
		<section class="sidebar-section" aria-labelledby={`sidebar-${section}`}>
			<h2 id={`sidebar-${section}`}>{section}</h2>
			<nav aria-label={`${section} pages`}>
				{#each docs.filter((doc) => doc.section === section) as doc (doc.slug)}
					<a
						class:active={doc.slug === currentSlug}
						href={resolve(`/docs/${doc.slug}/`)}
						aria-current={doc.slug === currentSlug ? 'page' : undefined}>
						{doc.title}
					</a>
				{/each}
			</nav>
		</section>
	{/each}
</div>

<style>
	.sidebar-inner {
		position: sticky;
		top: 6.75rem;
		max-height: calc(100vh - 8rem);
		overflow-y: auto;
		padding-right: 0.5rem;
	}

	.sidebar-kicker {
		margin: 0 0 0.85rem;
		color: var(--coral);
		font-family: 'IBM Plex Mono Variable', 'JetBrains Mono Variable', monospace;
		font-size: 0.7rem;
		font-weight: 650;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.sidebar-section {
		margin-top: 1.7rem;
	}

	.sidebar-section h2 {
		margin: 0 0 0.4rem;
		font-size: 0.78rem;
		font-weight: 650;
		letter-spacing: 0.01em;
	}

	.sidebar-section nav {
		display: grid;
		gap: 0.1rem;
	}

	.sidebar-section a {
		padding: 0.38rem 0.55rem;
		border-left: 2px solid transparent;
		color: var(--muted);
		font-size: 0.86rem;
		line-height: 1.35;
		text-decoration: none;
	}

	.sidebar-section a:hover,
	.sidebar-section a.active {
		border-left-color: var(--coral);
		background: var(--paper-deep);
		color: var(--teal-dark);
	}
</style>
