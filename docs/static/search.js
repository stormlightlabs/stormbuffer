const searchStates = [];
const initializedBoxes = new WeakSet();
let pagefind;
let activeSearch;

async function getPagefind() {
	pagefind ??= await import('/pagefind/pagefind.js');
	return pagefind;
}

function isVisible(element) {
	if (!(element instanceof HTMLElement)) return false;
	const style = window.getComputedStyle(element);
	return style.display !== 'none' && style.visibility !== 'hidden' && element.getClientRects().length > 0;
}

function preferredSearch() {
	pruneSearchStates();
	const visible = searchStates.find((state) => isVisible(state.trigger)) ?? searchStates.find((state) => isVisible(state.box));
	if (visible) return visible;

	const mobile = window.matchMedia('(max-width: 900px)').matches;
	return (
		searchStates.find((state) => mobile === Boolean(state.box.closest('details'))) ??
		searchStates[0]
	);
}

function escapeHtml(value) {
	return value.replace(/[&<>'"]/g, (character) => {
		const entities = { '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' };
		return entities[character];
	});
}

function getResultLinks(state) {
	return [...state.results.querySelectorAll('a[data-search-result]')];
}

function setStatus(state, message) {
	state.status.textContent = message;
}

function clearActiveResult(state) {
	state.activeIndex = -1;
	state.input.removeAttribute('aria-activedescendant');
	for (const link of getResultLinks(state)) {
		link.setAttribute('aria-selected', 'false');
	}
}

function setActiveResult(state, index) {
	const links = getResultLinks(state);
	if (links.length === 0) return;

	state.activeIndex = (index + links.length) % links.length;
	for (const [linkIndex, link] of links.entries()) {
		const isActive = linkIndex === state.activeIndex;
		link.setAttribute('aria-selected', String(isActive));
	}

	const activeLink = links[state.activeIndex];
	state.input.setAttribute('aria-activedescendant', activeLink.id);
	activeLink.scrollIntoView({ block: 'nearest' });
}

function moveActiveResult(state, direction) {
	const links = getResultLinks(state);
	if (links.length === 0) return;

	const nextIndex = state.activeIndex === -1 ? (direction > 0 ? 0 : links.length - 1) : state.activeIndex + direction;
	setActiveResult(state, nextIndex);
}

function renderMessage(state, message) {
	const element = document.createElement('p');
	element.className = 'search-message';
	element.textContent = message;
	state.results.replaceChildren(element);
	state.results.removeAttribute('role');
	state.results.removeAttribute('aria-label');
	state.input.setAttribute('aria-expanded', 'false');
	clearActiveResult(state);
}

function resetSearch(state) {
	window.clearTimeout(state.debounce);
	state.version += 1;
	state.input.value = '';
	renderMessage(state, 'Start typing to search the documentation.');
	setStatus(state, 'Search documentation.');
}

async function search(state, query, version) {
	setStatus(state, 'Searching…');
	renderMessage(state, 'Searching…');

	try {
		const index = await getPagefind();
		const response = await index.search(query);
		const settledMatches = await Promise.allSettled(response.results.slice(0, 8).map((result) => result.data()));
		const matches = settledMatches.flatMap((match) => (match.status === 'fulfilled' ? [match.value] : []));
		if (version !== state.version) return;
		if (response.results.length > 0 && matches.length === 0) throw new Error('Pagefind result fragments were unavailable');

		state.results.replaceChildren();
		clearActiveResult(state);
		if (matches.length === 0) {
			renderMessage(state, `No pages match “${query}”.`);
			setStatus(state, `No pages match “${query}”.`);
			return;
		}

		for (const [index, match] of matches.entries()) {
			const link = document.createElement('a');
			link.className = 'search-result';
			link.dataset.searchResult = '';
			link.id = `${state.results.id}-result-${index}`;
			link.href = match.url;
			link.setAttribute('role', 'option');
			link.setAttribute('aria-selected', 'false');
			link.tabIndex = -1;

			const title = document.createElement('strong');
			title.textContent = match.meta?.title ?? 'Documentation page';
			const excerpt = document.createElement('span');
			excerpt.innerHTML = escapeHtml(match.excerpt ?? '').replace(/&lt;(mark|\/mark)&gt;/g, '<$1>');
			link.append(title, excerpt);
			state.results.append(link);
		}
		state.results.setAttribute('role', 'listbox');
		state.results.setAttribute('aria-label', 'Search results');
		state.input.setAttribute('aria-expanded', 'true');

		const resultLabel = matches.length === 1 ? 'result' : 'results';
		setStatus(state, `${matches.length} ${resultLabel} found for “${query}”.`);
	} catch {
		if (version !== state.version) return;
		renderMessage(state, 'Search is unavailable right now. Try the full Search page.');
		setStatus(state, 'Search is unavailable right now.');
	}
}

function queueSearch(state) {
	window.clearTimeout(state.debounce);
	const query = state.input.value.trim();
	state.version += 1;
	const version = state.version;

	if (!query) {
		renderMessage(state, 'Start typing to search the documentation.');
		setStatus(state, 'Search documentation.');
		return;
	}

	renderMessage(state, 'Searching…');
	setStatus(state, 'Searching…');
	state.debounce = window.setTimeout(() => search(state, query, version), 120);
}

function focusReturnTarget(state) {
	if (state.openedDetails) {
		const details = state.box.closest('details');
		state.openedDetails = false;
		state.returnFocus = null;
		if (details instanceof HTMLDetailsElement) {
			details.open = false;
			const summary = details.querySelector(':scope > summary');
			if (summary instanceof HTMLElement) summary.focus({ preventScroll: true });
			return;
		}
	}

	const target = state.returnFocus;
	state.returnFocus = null;
	const fallback = searchStates.map((state) => state.trigger).find((trigger) => isVisible(trigger));

	if (target instanceof HTMLElement && target.isConnected && isVisible(target)) {
		target.focus({ preventScroll: true });
	} else if (fallback instanceof HTMLElement) {
		fallback.focus({ preventScroll: true });
	}
}

function closeSearch(state) {
	window.clearTimeout(state.debounce);
	state.version += 1;
	if (state.dialog.open) state.dialog.close();
}

function openSearch(state) {
	if (activeSearch && activeSearch !== state) closeSearch(activeSearch);
	if (state.dialog.open) {
		state.input.focus();
		return;
	}

	const details = state.box.closest('details');
	state.openedDetails = details instanceof HTMLDetailsElement && !details.open;
	if (state.openedDetails) details.open = true;

	state.returnFocus = document.activeElement instanceof HTMLElement && document.activeElement !== document.body
		? document.activeElement
		: state.trigger;
	resetSearch(state);
	state.trigger.setAttribute('aria-expanded', 'true');
	state.dialog.showModal();
	activeSearch = state;
	window.requestAnimationFrame(() => state.input.focus());
}

function pruneSearchStates() {
	for (let index = searchStates.length - 1; index >= 0; index -= 1) {
		const state = searchStates[index];
		if (state.box.isConnected) continue;
		window.clearTimeout(state.debounce);
		state.version += 1;
		if (activeSearch === state) activeSearch = undefined;
		searchStates.splice(index, 1);
	}
}

function initializeSearchBox(box) {
	if (initializedBoxes.has(box)) return;
	initializedBoxes.add(box);

	const trigger = box.querySelector('[data-search-trigger]');
	const dialog = box.querySelector('[data-search-dialog]');
	const form = box.querySelector('[data-search-form]');
	const input = box.querySelector('[data-search-input]');
	const results = box.querySelector('[data-search-results]');
	const status = box.querySelector('[data-search-status]');
	const close = box.querySelector('[data-search-close]');

	if (
		!(trigger instanceof HTMLButtonElement) ||
		!(dialog instanceof HTMLDialogElement) ||
		!(form instanceof HTMLFormElement) ||
		!(input instanceof HTMLInputElement) ||
		!(results instanceof HTMLElement) ||
		!(status instanceof HTMLElement) ||
		!(close instanceof HTMLButtonElement)
	) {
		return;
	}

	const state = {
		box,
		trigger,
		dialog,
		form,
		input,
		results,
		status,
		close,
		debounce: 0,
		version: 0,
		activeIndex: -1,
		returnFocus: null,
		openedDetails: false
	};
	searchStates.push(state);

	trigger.addEventListener('click', () => openSearch(state));
	close.addEventListener('click', () => closeSearch(state));
	form.addEventListener('submit', (event) => {
		event.preventDefault();
		window.clearTimeout(state.debounce);
		const query = state.input.value.trim();
		state.version += 1;
		if (query) search(state, query, state.version);
		else renderMessage(state, 'Start typing to search the documentation.');
	});
	input.addEventListener('input', () => queueSearch(state));
	input.addEventListener('keydown', (event) => {
		switch (event.key) {
			case 'ArrowDown':
				event.preventDefault();
				moveActiveResult(state, 1);
				break;
			case 'ArrowUp':
				event.preventDefault();
				moveActiveResult(state, -1);
				break;
			case 'Enter':
				if (state.activeIndex >= 0) {
					event.preventDefault();
					getResultLinks(state)[state.activeIndex]?.click();
				}
				break;
			case 'Escape':
				event.preventDefault();
				closeSearch(state);
				break;
		}
	});
	dialog.addEventListener('cancel', (event) => {
		event.preventDefault();
		closeSearch(state);
	});
	dialog.addEventListener('click', (event) => {
		if (event.target === dialog) closeSearch(state);
	});
	dialog.addEventListener('close', () => {
		state.trigger.setAttribute('aria-expanded', 'false');
		state.input.setAttribute('aria-expanded', 'false');
		if (activeSearch === state) activeSearch = undefined;
		focusReturnTarget(state);
	});
}

function initializeSearchBoxes(root = document) {
	if (root instanceof Element && root.matches('[data-doc-search]')) initializeSearchBox(root);
	for (const box of root.querySelectorAll('[data-doc-search]')) initializeSearchBox(box);
	pruneSearchStates();
}

initializeSearchBoxes();

new MutationObserver((mutations) => {
	for (const mutation of mutations) {
		for (const node of mutation.addedNodes) {
			if (node instanceof Element) initializeSearchBoxes(node);
		}
	}
	pruneSearchStates();
}).observe(document.body, { childList: true, subtree: true });

document.addEventListener('keydown', (event) => {
	if (event.defaultPrevented || event.isComposing || event.altKey) return;
	if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'k') return;

	event.preventDefault();
	const state = activeSearch ?? preferredSearch();
	if (!state) return;
	openSearch(state);
});
