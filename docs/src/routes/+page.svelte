<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import { getDocs } from '$lib/content';

	const docs = getDocs();
</script>

<svelte:head>
	<title>Stormbuffer documentation</title>
	<meta
		name="description"
		content="Documentation for Stormbuffer, a local-first memory store for people and agents."
	/>
</svelte:head>

<SiteHeader {docs} />

<main id="main-content" class="landing">
	<section class="landing-hero" aria-labelledby="landing-title">
		<p class="eyebrow">stormbuffer · documentation</p>
		<h1 id="landing-title">Inspectable Memory.</h1>
		<p class="landing-lede">
			Stormbuffer stores durable facts, decisions, procedures, and project checkpoints as readable,
			indexed Markdown—then finds the right context when you need it.
		</p>
		<div class="landing-actions">
			<a class="button-link" href={resolve('/docs/installation/')}>Get started</a>
			<a class="text-link" href={resolve('/docs/concepts/architecture/')}>
				Learn More <span aria-hidden="true">→</span>
			</a>
		</div>

		<figure class="terminal" aria-labelledby="terminal-caption">
			<figcaption id="terminal-caption">
				<span class="terminal-dots" aria-hidden="true"><i></i><i></i><i></i></span>
				<span>~/projects/stormbuffer</span>
			</figcaption>
			<pre><code
					><span class="prompt">$</span> sbuf --project init
Initialized private project store at .sbuf

<span class="prompt">$</span> sbuf search "why is the index disposable?"
<span class="result">decision · The core owns canonical writes</span>
Markdown records are authoritative. SQLite can be rebuilt.
<span class="meta">project · 0.94 · architecture</span></code
				></pre>
		</figure>
	</section>

	<section class="landing-docs" aria-labelledby="docs-title">
		<div class="section-heading">
			<p class="eyebrow">Documentation</p>
			<h2 id="docs-title">Install & set up the store.</h2>
		</div>

		<nav class="landing-index" aria-label="Documentation sections">
			{#each docs as doc, index (doc.slug)}
				<a href={resolve(`/docs/${doc.slug}/`)}>
					<span class="doc-number">{String(index + 1).padStart(2, '0')}</span>
					<span class="doc-label"><small>{doc.section}</small><strong>{doc.title}</strong></span>
					<span class="doc-arrow" aria-hidden="true">→</span>
				</a>
			{/each}
		</nav>
	</section>
</main>

<style>
	.landing {
		max-width: 52rem;
		margin: 0 auto;
		padding: clamp(4rem, 9vw, 7.5rem) 2rem 6rem;
	}

	.landing-hero {
		padding-bottom: clamp(5rem, 10vw, 8rem);
	}

	.landing-hero h1 {
		margin: 0;
		font-size: clamp(3.25rem, 8vw, 5.7rem);
		line-height: 0.96;
		letter-spacing: -0.025em;
	}

	.landing-lede {
		max-width: 40rem;
		margin: 2rem 0 1.75rem;
		color: var(--muted);
		font-size: clamp(1.1rem, 2vw, 1.28rem);
	}

	.landing-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
	}

	.text-link {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		min-height: 2.75rem;
		padding: 0.55rem 0.5rem;
		color: var(--teal-dark);
		font-weight: 650;
		text-decoration: none;
	}

	.text-link:hover {
		color: var(--coral);
	}

	.terminal {
		margin: clamp(3rem, 7vw, 5rem) 0 0;
		overflow: hidden;
		border: 1px solid var(--code-line);
		border-radius: 0.65rem;
		background: var(--code-surface);
		box-shadow: 0 24px 60px rgb(23 51 45 / 16%);
		color: var(--code-ink);
	}

	.terminal figcaption {
		display: grid;
		grid-template-columns: 4rem 1fr 4rem;
		align-items: center;
		min-height: 2.75rem;
		margin: 0;
		padding: 0 0.9rem;
		border-bottom: 1px solid var(--code-line);
		color: color-mix(in srgb, var(--code-ink) 62%, transparent);
		font-family: 'JetBrains Mono Variable', monospace;
		font-size: 0.72rem;
		text-align: center;
	}

	.terminal-dots {
		display: flex;
		gap: 0.35rem;
	}

	.terminal-dots i {
		width: 0.55rem;
		height: 0.55rem;
		border-radius: 50%;
		background: var(--coral);
	}

	.terminal-dots i:nth-child(2) {
		background: var(--yellow);
	}

	.terminal-dots i:nth-child(3) {
		background: var(--teal);
	}

	.terminal pre {
		margin: 0;
		padding: clamp(1.25rem, 4vw, 2rem);
		overflow-x: auto;
		font:
			0.86rem/1.75 'JetBrains Mono Variable',
			monospace;
	}

	.prompt,
	.result {
		color: var(--yellow);
	}

	.result {
		font-weight: 650;
	}

	.meta {
		color: color-mix(in srgb, var(--code-ink) 58%, transparent);
	}

	.landing-docs {
		border-top: 1px solid var(--line);
		padding-top: 3rem;
	}

	.section-heading h2 {
		max-width: 25ch;
		margin: 0 0 2.5rem;
		font-size: clamp(2rem, 5vw, 3rem);
		line-height: 1.08;
	}

	.landing-index {
		border-top: 1px solid var(--line);
	}

	.landing-index a {
		display: grid;
		grid-template-columns: 2.5rem 1fr auto;
		gap: 1rem;
		align-items: center;
		padding: 1.15rem 0;
		border-bottom: 1px solid var(--line);
		color: var(--ink);
		text-decoration: none;
	}

	.landing-index a:hover {
		color: var(--teal-dark);
	}

	.doc-number,
	.doc-arrow {
		color: var(--coral);
		font-family: 'JetBrains Mono Variable', monospace;
		font-size: 0.72rem;
	}

	.doc-label {
		display: grid;
	}

	.doc-label small {
		color: var(--muted);
		font-size: 0.72rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.doc-label strong {
		font-family: 'IBM Plex Serif', Georgia, serif;
		font-size: 1.15rem;
	}

	@media (max-width: 600px) {
		.landing {
			padding: 3.25rem 1.25rem 4rem;
		}

		.landing-hero h1 {
			font-size: clamp(2.8rem, 14vw, 4rem);
		}

		.terminal figcaption {
			grid-template-columns: 3.5rem 1fr 3.5rem;
		}

		.landing-index a {
			grid-template-columns: 2rem 1fr auto;
			gap: 0.65rem;
		}
	}
</style>
