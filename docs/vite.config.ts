import { icons as bootstrapIcons } from '@iconify-json/bi';
import { icons as remixIcons } from '@iconify-json/ri';
import presetIcons from '@unocss/preset-icons';
import rehypeSlug from 'rehype-slug';
import { escapeSvelte, mdsvex } from 'mdsvex';
import { createHighlighter } from 'shiki';
import UnoCSS from 'unocss/vite';
import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import adapter from '@sveltejs/adapter-static';
import { extractTableOfContents } from './src/lib/content/table-of-contents.ts';
import { sveltekit } from '@sveltejs/kit/vite';

const highlighter = await createHighlighter({
	themes: ['github-light', 'github-dark'],
	langs: ['shellscript', 'text', 'toml', 'json']
});

function documentationLanguage(language: string): 'shellscript' | 'text' | 'toml' | 'json' {
	switch (language) {
		case 'sh':
			return 'shellscript';
		case 'toml':
		case 'json':
		case 'text':
			return language;
		default:
			return 'text';
	}
}

export default defineConfig({
	plugins: [
		UnoCSS({ presets: [presetIcons({ collections: { bi: () => bootstrapIcons, ri: () => remixIcons } })] }),
		sveltekit({
			// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
			compilerOptions: {
				runes: ({ filename }) => (filename.split(/[/\\]/).includes('node_modules') ? undefined : true)
			},
			adapter: adapter(),
			preprocess: [
				mdsvex({
					extensions: ['.svx', '.md'],
					highlight: {
						highlighter: (code, language) =>
							escapeSvelte(
								highlighter
									.codeToHtml(code, {
										lang: documentationLanguage(language ?? 'text'),
										themes: { light: 'github-light', dark: 'github-dark' },
										defaultColor: false
									})
									.replace(' tabindex="0"', '')
							)
					},
					rehypePlugins: [rehypeSlug, extractTableOfContents]
				}),
				{
					name: 'mdsvex-script-module-fix',
					markup: ({ content, filename }) => {
						if (!filename?.match(/\.(?:md|svx)$/)) return;

						return { code: content.replace('<script context="module">', '<script module>') };
					}
				}
			],
			extensions: ['.svelte', '.svx', '.md']
		})
	],
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'client',
					browser: { enabled: true, provider: playwright(), instances: [{ browser: 'chromium', headless: true }] },
					include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
					exclude: ['src/lib/server/**']
				}
			},

			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
