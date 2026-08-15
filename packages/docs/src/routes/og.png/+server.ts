import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { Resvg } from '@resvg/resvg-js';
import Opengraph from '$lib/components/Opengraph.svelte';
import satori from 'satori';
import { html } from 'satori-html';
import { render } from 'svelte/server';

export const prerender = true;

const width = 1200;
const height = 630;

function staticPath(file: string): string {
	return join(process.cwd(), 'static', file);
}

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
	return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function stripSvelteComments(value: string): string {
	return value.replace(/<!--[\s\S]*?-->/g, '');
}

export async function GET(): Promise<Response> {
	const [googleSans, ibmPlexSans, favicon] = await Promise.all([
		readFile(staticPath('og/fonts/google-sans-latin-normal.ttf')).then(toArrayBuffer),
		readFile(staticPath('og/fonts/ibm-plex-sans-latin-normal.ttf')).then(toArrayBuffer),
		readFile(join(process.cwd(), 'src', 'lib', 'assets', 'favicon.svg')).then(
			(value) => `data:image/svg+xml;base64,${value.toString('base64')}`
		)
	]);
	const { body } = render(Opengraph, { props: { width, height, faviconDataUrl: favicon } });
	const markup = html(stripSvelteComments(body));
	const svg = await satori(markup, {
		width,
		height,
		fonts: [
			{ name: 'Google Sans', data: googleSans, weight: 400, style: 'normal' },
			{ name: 'IBM Plex Sans', data: ibmPlexSans, weight: 400, style: 'normal' }
		]
	});
	const png = new Resvg(svg, { fitTo: { mode: 'width', value: width } }).render().asPng();

	return new Response(toArrayBuffer(png), {
		headers: { 'cache-control': 'public, immutable, max-age=31536000', 'content-type': 'image/png' }
	});
}
