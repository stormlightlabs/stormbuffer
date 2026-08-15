<script lang="ts">
	import { absoluteUrl, site } from '$lib/site';

	interface Props {
		title: string;
		description: string;
		pathname: string;
		index?: boolean;
	}

	let { title, description, pathname, index = true }: Props = $props();

	const canonicalUrl = $derived(absoluteUrl(pathname));
	const imageUrl = absoluteUrl(site.imagePath);
	const robots = $derived(index ? 'index, follow, max-image-preview:large' : 'noindex, follow');
</script>

<svelte:head>
	<title>{title}</title>
	<meta name="description" content={description} />
	<meta name="robots" content={robots} />
	<link rel="canonical" href={canonicalUrl} />

	<meta itemprop="name" content={title} />
	<meta itemprop="description" content={description} />
	<meta itemprop="image" content={imageUrl} />

	<meta property="og:type" content="website" />
	<meta property="og:locale" content="en_US" />
	<meta property="og:site_name" content={site.name} />
	<meta property="og:url" content={canonicalUrl} />
	<meta property="og:title" content={title} />
	<meta property="og:description" content={description} />
	<meta property="og:image" content={imageUrl} />
	<meta property="og:image:secure_url" content={imageUrl} />
	<meta property="og:image:type" content="image/png" />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta property="og:image:alt" content={site.imageAlt} />

	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:url" content={canonicalUrl} />
	<meta name="twitter:title" content={title} />
	<meta name="twitter:description" content={description} />
	<meta name="twitter:image" content={imageUrl} />
	<meta name="twitter:image:alt" content={site.imageAlt} />
</svelte:head>
