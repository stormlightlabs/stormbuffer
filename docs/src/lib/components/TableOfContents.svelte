<script lang="ts">
	import type { DocHeading } from '$lib/content/types';

	let { headings }: { headings: DocHeading[] } = $props();
</script>

{#if headings.length > 0}
	<nav class="toc" aria-label="On this page" data-pagefind-ignore>
		<p class="toc-title">On this page</p>
		<ul>
			{#each headings as heading (heading.slug)}
				<li class:toc-subitem={heading.level === 3}>
					<a href={`#${heading.slug}`}>{heading.title}</a>
				</li>
			{/each}
		</ul>
	</nav>
{/if}

<style>
	.toc {
		position: sticky;
		top: 6.75rem;
		padding-left: 1rem;
		border-left: 1px solid var(--line);
	}

	.toc-title {
		margin: 0 0 0.85rem;
		color: var(--coral);
		font-family: 'IBM Plex Mono Variable', 'JetBrains Mono Variable', monospace;
		font-size: 0.7rem;
		font-weight: 650;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.toc ul {
		display: grid;
		gap: 0.5rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.toc li {
		line-height: 1.35;
	}

	.toc li.toc-subitem {
		padding-left: 0.75rem;
	}

	.toc a {
		color: var(--muted);
		font-size: 0.78rem;
		text-decoration: none;
	}

	.toc a:hover {
		color: var(--teal-dark);
	}
</style>
