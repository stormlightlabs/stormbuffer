<script lang="ts">
	import type { PageProps } from './$types';
	import DocsShell from '$lib/components/DocsShell.svelte';
	import { getDoc, getDocs } from '$lib/content';

	let { data }: PageProps = $props();
	const doc = $derived.by(() => {
		const found = getDoc(data.slug);
		if (!found) {
			throw new Error(`Documentation page not found: ${data.slug}`);
		}
		return found;
	});
	const docs = getDocs();
	const Content = $derived(doc.component);
</script>

<DocsShell {doc} {docs}>
	{#snippet content()}
		<Content />
	{/snippet}
</DocsShell>
