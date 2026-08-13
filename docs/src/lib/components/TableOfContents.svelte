<script lang="ts">
	import type { DocHeading } from '$lib/content/types';
	import { startScrollSpy } from '$lib/scroll-spy';

	let { headings }: { headings: DocHeading[] } = $props();
	let activeHeading = $state<string | null>(null);

	$effect(() => {
		const elements = headings
			.map((heading) => document.getElementById(heading.slug))
			.filter((heading): heading is HTMLElement => heading instanceof HTMLElement);

		return startScrollSpy(elements, (heading) => {
			activeHeading = heading;
		});
	});
</script>

{#if headings.length > 0}
	<nav class="toc" aria-label="On this page" data-pagefind-ignore>
		<p class="toc-title">On this page</p>
		<ul>
			{#each headings as heading (heading.slug)}
				<li class:toc-subitem={heading.level === 3} class:active={heading.slug === activeHeading}>
					<a href={`#${heading.slug}`} aria-current={heading.slug === activeHeading ? 'location' : undefined}
						>{heading.title}</a>
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
		color: var(--gold);
		font-family: 'Google Sans Code Variable', 'Google Sans Code', monospace;
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
		position: relative;
		line-height: 1.35;
	}

	.toc li.active::before {
		position: absolute;
		top: 0.15em;
		bottom: 0.15em;
		left: -1.05rem;
		width: 2px;
		background: var(--gold-bright);
		content: '';
	}

	.toc li.toc-subitem {
		padding-left: 0.75rem;
	}

	.toc a {
		color: var(--muted);
		font-size: 0.78rem;
		text-decoration: none;
	}

	.toc a:hover,
	.toc a[aria-current='location'] {
		color: var(--teal-dark);
	}

	.toc a[aria-current='location'] {
		font-weight: 650;
	}
</style>
