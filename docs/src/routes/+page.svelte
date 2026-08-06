<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import { getDocs } from '$lib/content';

	const docs = getDocs();
</script>

<svelte:head>
	<title>Stormbuffer documentation</title>
	<meta name="description" content="Documentation for Stormbuffer, a local-first memory store for people and agents." />
</svelte:head>

<SiteHeader {docs} />

<main id="main-content" class="landing">
	<section class="landing-hero" aria-labelledby="landing-title">
		<div>
			<p class="eyebrow">stormbuffer · documentation</p>
			<h1 id="landing-title">Inspectable Memory.</h1>
		</div>
		<div>
			<p class="landing-lede">
				Stormbuffer keeps durable facts, decisions, procedures, and project checkpoints in readable Markdown, with a
				powerful index for retrieval.
			</p>
			<div class="landing-actions">
				<a class="button-link" href={resolve('/docs/installation/')}>Get Started</a>
				<a class="button-link secondary" href={resolve('/docs/concepts/architecture/')}> Learn More </a>
			</div>
		</div>
	</section>

	<nav class="landing-index" aria-label="Documentation sections">
		{#each docs as doc (doc.slug)}
			<a href={resolve(`/docs/${doc.slug}/`)}>
				<span>{doc.section}</span>
				<strong>{doc.title}</strong>
			</a>
		{/each}
	</nav>
</main>

<style>
	.landing {
		max-width: 1440px;
		margin: 0 auto;
		padding: 5rem 2rem 6rem;
	}

	.landing-hero {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(18rem, 0.9fr);
		gap: clamp(2rem, 8vw, 8rem);
		align-items: end;
		padding: 2rem 0 5rem;
	}

	.landing-hero h1 {
		max-width: 10ch;
		margin: 0;
		font-size: clamp(3.3rem, 8vw, 7.5rem);
		line-height: 0.94;
	}

	.landing-lede {
		max-width: 32rem;
		margin: 0 0 1.75rem;
		color: var(--muted);
		font-size: clamp(1.05rem, 2vw, 1.3rem);
	}

	.landing-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
	}

	.button-link.secondary {
		background: transparent;
		color: var(--teal-dark);
	}

	.landing-index {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1rem;
		padding-top: 2rem;
		border-top: 1px solid var(--line);
	}

	.landing-index a {
		display: grid;
		align-content: start;
		min-height: 10rem;
		padding: 1.1rem;
		border-left: 3px solid var(--teal);
		background: var(--white);
		color: var(--ink);
		text-decoration: none;
	}

	.landing-index a:hover {
		border-left-color: var(--coral);
		box-shadow: var(--shadow);
	}

	.landing-index span {
		margin-bottom: 1.75rem;
		color: var(--coral);
		font-family: 'IBM Plex Mono Variable', 'JetBrains Mono Variable', monospace;
		font-size: 0.7rem;
		text-transform: uppercase;
	}

	.landing-index strong {
		font-family: 'IBM Plex Serif', Georgia, serif;
		font-size: 1.25rem;
	}

	@media (max-width: 900px) {
		.landing {
			padding: 3rem 1.25rem 4rem;
		}

		.landing-hero {
			grid-template-columns: 1fr;
			gap: 2rem;
			padding: 1.5rem 0 3rem;
		}

		.landing-index {
			grid-template-columns: 1fr;
		}

		.landing-index a {
			min-height: auto;
		}
	}
</style>
