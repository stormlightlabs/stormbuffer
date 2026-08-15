export const site = {
	name: 'Stormbuffer',
	title: 'Stormbuffer - Inspectable memory for people and agents',
	description: 'Stormbuffer is a local-first memory system for people and agents.',
	url: 'https://stormbuffer.stormlightlabs.org',
	imagePath: '/og.png',
	imageAlt: 'Stormbuffer documentation.',
	githubUrl: 'https://github.com/stormlightlabs/stormbuffer'
} as const;

export function absoluteUrl(pathname: string): string {
	return new URL(pathname, site.url).toString();
}
