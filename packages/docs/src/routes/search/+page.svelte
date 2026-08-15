<script lang="ts">
	import { resolve } from '$app/paths';
	import Seo from '$lib/components/Seo.svelte';
</script>

<Seo
	title="Search · Stormbuffer"
	description="Search the Stormbuffer documentation."
	pathname="/search/"
	index={false} />

<main class="fallback-page" data-pagefind-ignore>
	<a class="skip-link" href="#fallback-content">Skip to content</a>
	<div class="fallback-inner" id="fallback-content">
		<p class="eyebrow">Stormbuffer documentation</p>
		<h1>Search</h1>
		<p>
			Search uses the local Pagefind index in the production build. Use the navigation to browse these static pages when
			JavaScript is disabled.
		</p>
		<a class="button-link" href={resolve('/docs/installation/')}>Start with installation</a>
	</div>
</main>

<style>
	.fallback-page {
		min-height: 100vh;
		padding: 2rem;
		background: var(--paper);
	}

	.fallback-inner {
		max-width: 44rem;
		margin: 8rem auto;
	}

	.fallback-inner h1 {
		margin: 0 0 1rem;
		font-size: clamp(3rem, 8vw, 5rem);
		line-height: 1;
	}
</style>
