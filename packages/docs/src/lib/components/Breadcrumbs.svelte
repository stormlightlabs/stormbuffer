<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Doc } from '$lib/content/types';

	let { doc }: { doc: Doc } = $props();
</script>

<nav class="breadcrumbs" aria-label="Breadcrumbs" data-pagefind-ignore>
	<a href={resolve('/')}>Home</a>
	<span aria-hidden="true">/</span>
	<a href={resolve(`/docs/${doc.slug}/`)}>{doc.group}</a>
	<span aria-hidden="true">/</span>
	<span aria-current="page">{doc.title}</span>
</nav>

<style>
	.breadcrumbs {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.45rem;
		margin-bottom: 2.5rem;
		color: var(--muted);
		font-size: 0.8rem;
	}

	.breadcrumbs a {
		color: var(--muted);
	}

	.breadcrumbs span[aria-current='page'] {
		color: var(--ink);
	}

	@media (max-width: 900px) {
		.breadcrumbs {
			margin-bottom: 2rem;
		}
	}
</style>
