<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Doc } from '$lib/content/types';
	import Search from './Search.svelte';

	let { docs, currentSlug = '' }: { docs: Doc[]; currentSlug?: string } = $props();

	const primaryLinks = [
		{ label: 'Get started', href: '/docs/installation/', slug: 'installation' },
		{ label: 'CLI', href: '/docs/cli/reference/', slug: 'cli/reference' },
		{ label: 'Concepts', href: '/docs/concepts/architecture/', slug: 'concepts/architecture' }
	] as const;

	function isCurrent(slug: string): boolean {
		return currentSlug === slug || currentSlug.startsWith(`${slug}/`);
	}

	const skipHref = $derived(currentSlug ? resolve(`/docs/${currentSlug}/#main-content`) : resolve('/#main-content'));
</script>

<a class="skip-link" href={skipHref}>Skip to content</a>
<header class="site-header" data-pagefind-ignore>
	<div class="header-inner">
		<a class="brand" href={resolve('/')} aria-label="Stormbuffer documentation home">
			<span class="brand-mark" aria-hidden="true">S</span>
			<span>stormbuffer</span>
		</a>

		<nav class="primary-nav" aria-label="Primary navigation">
			{#each primaryLinks as link (link.slug)}
				<a class:active={isCurrent(link.slug)} href={resolve(link.href)}>{link.label}</a>
			{/each}
		</nav>

		<div class="header-actions">
			<div class="desktop-search"><Search id="header-search" /></div>
			<details class="mobile-menu">
				<summary><span aria-hidden="true">☰</span> Menu</summary>
				<div class="mobile-menu-panel">
					<Search id="mobile-search" />
					<nav aria-label="Mobile navigation">
						{#each primaryLinks as link (link.slug)}
							<a class:active={isCurrent(link.slug)} href={resolve(link.href)}>{link.label}</a>
						{/each}
					</nav>
					<div class="mobile-doc-links">
						{#each docs as doc (doc.slug)}
							<a class:active={doc.slug === currentSlug} href={resolve(`/docs/${doc.slug}/`)}>{doc.title}</a>
						{/each}
					</div>
				</div>
			</details>
		</div>
	</div>
</header>

<style>
	.site-header {
		position: sticky;
		top: 0;
		z-index: 10;
		border-bottom: 1px solid var(--line);
		background: rgb(247 245 239 / 94%);
		backdrop-filter: blur(12px);
	}

	.header-inner {
		display: flex;
		align-items: center;
		gap: 2rem;
		max-width: 1440px;
		min-height: 4.5rem;
		margin: 0 auto;
		padding: 0.75rem 2rem;
	}

	.brand {
		display: inline-flex;
		align-items: center;
		gap: 0.65rem;
		flex: 0 0 auto;
		color: var(--ink);
		font-size: 1.04rem;
		font-weight: 650;
		letter-spacing: -0.02em;
		text-decoration: none;
	}

	.brand-mark {
		display: grid;
		place-items: center;
		width: 2rem;
		height: 2rem;
		border-radius: 0.35rem;
		background: var(--teal);
		color: var(--white);
		font-family: 'IBM Plex Serif', serif;
		font-size: 1.4rem;
		font-weight: 600;
		line-height: 1;
	}

	.primary-nav {
		display: flex;
		align-items: center;
		gap: 1.35rem;
		margin-right: auto;
	}

	.primary-nav a,
	.mobile-menu-panel nav a,
	.mobile-doc-links a {
		color: var(--muted);
		font-size: 0.92rem;
		font-weight: 550;
		text-decoration: none;
	}

	.primary-nav a:hover,
	.primary-nav a.active,
	.mobile-menu-panel nav a:hover,
	.mobile-menu-panel nav a.active,
	.mobile-doc-links a:hover,
	.mobile-doc-links a.active {
		color: var(--teal-dark);
	}

	.primary-nav a.active {
		text-decoration: underline;
		text-decoration-color: var(--coral);
		text-decoration-thickness: 2px;
	}

	.header-actions {
		display: flex;
		align-items: center;
		gap: 0.85rem;
	}

	.mobile-menu {
		display: none;
		position: relative;
	}

	.mobile-menu summary {
		padding: 0.45rem 0.65rem;
		border: 1px solid var(--line);
		border-radius: 0.35rem;
		color: var(--teal-dark);
		font-size: 0.88rem;
		font-weight: 600;
		cursor: pointer;
		list-style: none;
	}

	.mobile-menu summary::-webkit-details-marker {
		display: none;
	}

	.mobile-menu-panel {
		position: absolute;
		top: calc(100% + 0.75rem);
		right: 0;
		display: grid;
		gap: 1rem;
		width: min(20rem, calc(100vw - 2rem));
		padding: 1rem;
		border: 1px solid var(--line);
		background: var(--white);
		box-shadow: var(--shadow);
	}

	.mobile-menu-panel nav,
	.mobile-doc-links {
		display: grid;
		gap: 0.6rem;
	}

	.mobile-doc-links {
		padding-top: 0.85rem;
		border-top: 1px solid var(--line);
	}

	@media (max-width: 900px) {
		.header-inner {
			gap: 1rem;
			padding: 0.7rem 1rem;
		}

		.primary-nav,
		.desktop-search {
			display: none;
		}

		.header-actions {
			margin-left: auto;
		}

		.mobile-menu {
			display: block;
		}
	}
</style>
