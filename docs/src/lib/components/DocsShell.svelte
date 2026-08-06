<script lang="ts">
	import type { Snippet } from 'svelte';
	import Breadcrumbs from './Breadcrumbs.svelte';
	import PageNavigation from './PageNavigation.svelte';
	import Sidebar from './Sidebar.svelte';
	import SiteHeader from './SiteHeader.svelte';
	import TableOfContents from './TableOfContents.svelte';
	import { getAdjacentDocs } from '$lib/content';
	import type { Doc } from '$lib/content/types';

	let { doc, docs, content }: { doc: Doc; docs: Doc[]; content: Snippet } = $props();
	const adjacent = $derived(getAdjacentDocs(doc.slug));
</script>

<svelte:head>
	<title>{doc.title} · Stormbuffer docs</title>
	<meta name="description" content={doc.description} />
</svelte:head>

<SiteHeader {docs} currentSlug={doc.slug} />

<div class="docs-layout">
	<aside class="sidebar" aria-label="Documentation navigation">
		<Sidebar {docs} currentSlug={doc.slug} />
	</aside>

	<main id="main-content" class="docs-main">
		<Breadcrumbs {doc} />
		<article class="doc-article" data-pagefind-body>
			<header class="doc-heading">
				<div class="doc-meta">
					<span>{doc.section}</span>
				</div>
				<h1>{doc.title}</h1>
				<p class="doc-description">{doc.description}</p>
			</header>
			<div class="doc-content">
				{@render content()}
			</div>
		</article>
		<PageNavigation previous={adjacent.previous} next={adjacent.next} />
	</main>

	<aside class="toc-column" aria-label="Table of contents">
		<TableOfContents headings={doc.toc} />
	</aside>
</div>

<style>
	.docs-layout {
		display: grid;
		grid-template-columns: 13rem minmax(0, 1fr) 13rem;
		gap: clamp(2rem, 5vw, 5rem);
		max-width: 1440px;
		margin: 0 auto;
		padding: 2.75rem 2rem 5rem;
	}

	.sidebar,
	.toc-column {
		min-width: 0;
	}

	.docs-main {
		min-width: 0;
		max-width: 52rem;
	}

	.doc-heading {
		padding-bottom: 2rem;
		border-bottom: 1px solid var(--line);
	}

	.doc-meta {
		display: flex;
		gap: 0.55rem;
		margin-bottom: 0.8rem;
		color: var(--teal);
		font-family: 'IBM Plex Mono Variable', 'JetBrains Mono Variable', monospace;
		font-size: 0.76rem;
	}

	.doc-heading h1 {
		max-width: 14ch;
		margin: 0;
		color: var(--ink);
		font-size: clamp(2.4rem, 5vw, 4.5rem);
		line-height: 1.02;
	}

	.doc-description {
		max-width: 42rem;
		margin: 1.25rem 0 0;
		color: var(--muted);
		font-size: 1.12rem;
		line-height: 1.55;
	}

	.doc-content {
		padding-top: 2rem;
	}

	.doc-content :global(h2),
	.doc-content :global(h3) {
		scroll-margin-top: 6.5rem;
	}

	.doc-content :global(h2) {
		margin: 3rem 0 0.8rem;
		font-size: clamp(1.55rem, 2.5vw, 2.1rem);
		line-height: 1.15;
	}

	.doc-content :global(h3) {
		margin: 2rem 0 0.65rem;
		font-size: 1.35rem;
		line-height: 1.2;
	}

	.doc-content :global(p),
	.doc-content :global(ul),
	.doc-content :global(ol),
	.doc-content :global(table),
	.doc-content :global(blockquote) {
		max-width: 46rem;
	}

	.doc-content :global(p) {
		margin: 0 0 1.1rem;
	}

	.doc-content :global(ul),
	.doc-content :global(ol) {
		margin: 0 0 1.25rem;
		padding-left: 1.35rem;
	}

	.doc-content :global(li + li) {
		margin-top: 0.4rem;
	}

	.doc-content :global(li)::marker {
		color: var(--coral);
	}

	.doc-content :global(strong) {
		font-weight: 650;
	}

	.doc-content :global(code) {
		padding: 0.14rem 0.28rem;
		border: 1px solid var(--line);
		border-radius: 0.2rem;
		background: var(--paper-deep);
		font-family: 'JetBrains Mono Variable', 'JetBrains Mono', monospace;
		font-size: 0.86em;
	}

	.doc-content :global(pre) {
		max-width: 52rem;
		margin: 1.5rem 0;
		padding: 1.15rem 1.25rem;
		overflow-x: auto;
		border: 1px solid var(--code-line);
		border-radius: 0.35rem;
		background: var(--code-surface);
		color: var(--code-ink);
		box-shadow: 0.35rem 0.35rem 0 var(--yellow);
	}

	.doc-content :global(pre code) {
		padding: 0;
		border: 0;
		background: none;
		color: inherit;
		font-size: 0.84rem;
		line-height: 1.65;
	}

	.doc-content :global(blockquote) {
		margin: 1.5rem 0;
		padding: 0.8rem 1.1rem;
		border-left: 4px solid var(--yellow);
		background: var(--callout-surface);
	}

	.doc-content :global(blockquote p:last-child) {
		margin-bottom: 0;
	}

	.doc-content :global(table) {
		width: 100%;
		margin: 1.5rem 0;
		border-collapse: collapse;
		font-size: 0.93rem;
	}

	.doc-content :global(th),
	.doc-content :global(td) {
		padding: 0.65rem 0.7rem;
		border-bottom: 1px solid var(--line);
		text-align: left;
		vertical-align: top;
	}

	.doc-content :global(th) {
		color: var(--teal-dark);
		font-size: 0.78rem;
		font-weight: 650;
		letter-spacing: 0.02em;
		text-transform: uppercase;
	}

	.doc-content :global(hr) {
		margin: 2.5rem 0;
		border: 0;
		border-top: 1px solid var(--line);
	}

	@media (max-width: 1160px) {
		.docs-layout {
			grid-template-columns: 12rem minmax(0, 1fr);
			gap: 2.5rem;
		}

		.toc-column {
			display: none;
		}
	}

	@media (max-width: 900px) {
		.docs-layout {
			display: block;
			padding: 2rem 1.25rem 4rem;
		}

		.sidebar {
			display: none;
		}

		.docs-main {
			max-width: none;
		}
	}

	@media (max-width: 520px) {
		.doc-heading h1 {
			font-size: 2.8rem;
		}

		.doc-content :global(pre) {
			margin-right: -0.25rem;
			margin-left: -0.25rem;
			box-shadow: 0.2rem 0.2rem 0 var(--yellow);
		}
	}
</style>
