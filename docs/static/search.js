const searchBoxes = document.querySelectorAll('[data-doc-search]');
let pagefind;

async function getPagefind() {
	pagefind ??= await import('/pagefind/pagefind.js');
	return pagefind;
}

function escapeHtml(value) {
	return value.replace(/[&<>'"]/g, (character) => {
		const entities = { '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' };
		return entities[character];
	});
}

for (const box of searchBoxes) {
	const form = box.querySelector('form');
	const input = box.querySelector('[data-search-input]');
	const results = box.querySelector('[data-search-results]');
	if (!(form instanceof HTMLFormElement) || !(input instanceof HTMLInputElement) || !(results instanceof HTMLElement)) {
		continue;
	}

	form.addEventListener('submit', async (event) => {
		event.preventDefault();
		const query = input.value.trim();
		results.replaceChildren();
		if (!query) {
			return;
		}

		const loading = document.createElement('p');
		loading.className = 'search-message';
		loading.textContent = 'Searching…';
		results.append(loading);

		try {
			const index = await getPagefind();
			const response = await index.search(query);
			const matches = await Promise.all(response.results.slice(0, 6).map((result) => result.data()));
			results.replaceChildren();
			if (matches.length === 0) {
				const empty = document.createElement('p');
				empty.className = 'search-message';
				empty.textContent = 'No matching pages.';
				results.append(empty);
				return;
			}

			for (const match of matches) {
				const link = document.createElement('a');
				link.href = match.url;
				link.innerHTML = `<strong>${escapeHtml(match.meta?.title ?? 'Documentation page')}</strong><span>${match.excerpt}</span>`;
				results.append(link);
			}
		} catch {
			results.replaceChildren();
			const error = document.createElement('p');
			error.className = 'search-message';
			error.textContent = 'Search is available in the production build.';
			results.append(error);
		}
	});
}
