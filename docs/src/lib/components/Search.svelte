<script lang="ts">
	import { resolve } from '$app/paths';

	let { id }: { id: string } = $props();

	const dialogId = $derived(`${id}-dialog`);
	const titleId = $derived(`${id}-title`);
	const descriptionId = $derived(`${id}-description`);
	const inputId = $derived(`${id}-input`);
	const resultsId = $derived(`${id}-results`);
</script>

<div class="search" data-doc-search>
	<button
		class="search-trigger"
		type="button"
		data-search-trigger
		aria-controls={dialogId}
		aria-expanded="false"
		aria-haspopup="dialog"
		aria-label="Open documentation search (Ctrl+K or Cmd+K)">
		<span class="i-ri-search-line search-icon" aria-hidden="true"></span>
		<span class="search-trigger-label">Search docs</span>
		<span class="shortcut-hint" aria-hidden="true">
			<kbd>Ctrl</kbd>
			<span>/</span>
			<kbd>⌘</kbd>
			<kbd>K</kbd>
		</span>
	</button>

	<a class="search-fallback" href={resolve('/search/')}>
		<span class="i-ri-book-open-line" aria-hidden="true"></span>
		<span>Browse documentation</span>
	</a>

	<dialog
		class="command-dialog"
		data-search-dialog
		aria-labelledby={titleId}
		aria-describedby={descriptionId}
		id={dialogId}>
		<div class="dialog-inner">
			<header class="dialog-header">
				<div>
					<p class="dialog-kicker">Documentation search</p>
					<h2 id={titleId}>Search the docs</h2>
				</div>
				<button class="dialog-close" type="button" data-search-close aria-label="Close search">
					<span class="i-ri-close-line" aria-hidden="true"></span>
				</button>
			</header>

			<p class="dialog-description" id={descriptionId}>
				Find guides, commands, and concepts in the Stormbuffer documentation.
			</p>

			<form class="command-form" role="search" action="/search/" method="get" data-search-form>
				<label class="sr-only" for={inputId}>Search documentation</label>
				<div class="command-input-wrap">
					<span class="i-ri-search-line command-search-icon" aria-hidden="true"></span>
					<input
						id={inputId}
						name="q"
						type="search"
						data-search-input
						placeholder="Search pages, commands, and concepts…"
						autocomplete="off"
						role="combobox"
						aria-autocomplete="list"
						aria-expanded="false"
						aria-haspopup="listbox"
						aria-controls={resultsId} />
					<button type="submit" aria-label="Search documentation">
						<span class="i-ri-search-line" aria-hidden="true"></span>
					</button>
				</div>
			</form>

			<p class="search-status sr-only" data-search-status role="status" aria-live="polite"></p>
			<div class="search-results" id={resultsId} data-search-results>
				<p class="search-message">Start typing to search the documentation.</p>
			</div>

			<footer class="dialog-footer" aria-label="Search keyboard shortcuts">
				<span><kbd>↑</kbd><kbd>↓</kbd> to navigate</span>
				<span><kbd>Enter</kbd> to open</span>
				<span><kbd>Esc</kbd> to close</span>
			</footer>
		</div>
	</dialog>
</div>

<style>
	.search {
		position: relative;
		min-width: 12rem;
	}

	.search-trigger,
	.search-fallback {
		display: flex;
		align-items: center;
		width: 100%;
		min-height: 2.75rem;
		border: 1px solid var(--line);
		border-radius: 0.35rem;
		background: var(--surface-raised);
		color: var(--muted);
		font-size: 0.86rem;
	}

	.search-trigger {
		gap: 0.5rem;
		padding: 0.42rem 0.55rem;
		text-align: left;
		cursor: pointer;
	}

	.search-trigger:hover {
		border-color: var(--teal);
		color: var(--teal-dark);
	}

	.search-trigger:focus-visible {
		border-color: var(--teal);
	}

	.search-icon,
	.command-search-icon {
		flex: 0 0 auto;
		font-size: 1rem;
	}

	.search-trigger-label {
		min-width: 0;
		overflow: hidden;
		color: var(--ink);
		font-weight: 550;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.shortcut-hint {
		display: inline-flex;
		align-items: center;
		gap: 0.18rem;
		margin-left: auto;
		color: var(--muted);
		font-size: 0.67rem;
		line-height: 1;
		white-space: nowrap;
	}

	kbd {
		padding: 0.18rem 0.3rem;
		border: 1px solid var(--line);
		border-bottom-width: 2px;
		border-radius: 0.2rem;
		background: var(--paper-deep);
		font:
			650 0.68rem/1 'Google Sans Code Variable',
			'Google Sans Code',
			monospace;
	}

	.search-fallback {
		gap: 0.5rem;
		padding: 0.42rem 0.55rem;
		text-decoration: none;
	}

	.search-fallback:hover {
		border-color: var(--teal);
		color: var(--teal-dark);
	}

	.command-input-wrap button {
		display: inline-grid;
		place-items: center;
		width: 2.5rem;
		align-self: stretch;
		flex: 0 0 auto;
		border: 0;
		border-left: 1px solid var(--line);
		background: transparent;
		color: var(--teal-dark);
		cursor: pointer;
	}

	.command-input-wrap button:hover {
		background: var(--paper-deep);
	}

	:global(html.js) .search-fallback {
		display: none;
	}

	:global(html:not(.js)) .search-trigger {
		display: none;
	}

	.command-dialog {
		width: min(42rem, calc(100vw - 2rem));
		max-width: none;
		max-height: calc(100dvh - 2rem);
		margin: auto;
		padding: 0;
		overflow: hidden;
		border: 1px solid var(--line);
		border-radius: 0.65rem;
		background: var(--surface-raised);
		color: var(--ink);
		box-shadow: 0 24px 70px rgb(23 51 45 / 18%);
	}

	.command-dialog::backdrop {
		background: rgb(16 33 29 / 52%);
		backdrop-filter: blur(3px);
	}

	.command-dialog[open] {
		animation: search-dialog-in 140ms ease-out;
	}

	.dialog-inner {
		display: grid;
		gap: 1rem;
		max-height: calc(100dvh - 2rem);
		padding: 1.25rem;
	}

	.dialog-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}

	.dialog-kicker {
		margin: 0 0 0.3rem;
		color: var(--gold);
		font-family: 'Google Sans Code Variable', 'Google Sans Code', monospace;
		font-size: 0.68rem;
		font-weight: 650;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.dialog-header h2 {
		margin: 0;
		font-size: clamp(1.45rem, 4vw, 1.9rem);
		line-height: 1.1;
	}

	.dialog-close {
		display: inline-grid;
		place-items: center;
		width: 2.5rem;
		height: 2.5rem;
		flex: 0 0 auto;
		border: 1px solid var(--line);
		border-radius: 0.3rem;
		background: transparent;
		color: var(--muted);
		font-size: 1.5rem;
		line-height: 1;
		cursor: pointer;
	}

	.dialog-close:hover {
		border-color: var(--teal);
		background: var(--paper-deep);
		color: var(--teal-dark);
	}

	.dialog-description {
		max-width: 42rem;
		margin: -0.35rem 0 0;
		color: var(--muted);
		font-size: 0.88rem;
		line-height: 1.45;
	}

	.command-form {
		margin: 0;
	}

	.command-input-wrap {
		display: flex;
		align-items: center;
		min-height: 3.2rem;
		border: 1px solid var(--line);
		border-radius: 0.4rem;
		background: var(--paper);
	}

	.command-input-wrap:focus-within {
		border-color: var(--teal);
		box-shadow: 0 0 0 3px var(--focus-soft);
	}

	.command-search-icon {
		margin-left: 0.85rem;
		color: var(--teal);
	}

	.command-input-wrap input {
		width: 100%;
		min-width: 0;
		padding: 0.72rem 0.7rem;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--ink);
		font-size: 1rem;
	}

	.search-results {
		min-height: 4rem;
		max-height: min(48vh, 24rem);
		margin: 0 -0.25rem;
		overflow-y: auto;
		overscroll-behavior: contain;
	}

	.search-results :global(.search-result) {
		display: grid;
		gap: 0.18rem;
		margin: 0 0.25rem;
		padding: 0.75rem;
		border: 1px solid transparent;
		border-radius: 0.35rem;
		color: var(--ink);
		font-size: 0.86rem;
		text-decoration: none;
	}

	.search-results :global(.search-result:hover),
	.search-results :global(.search-result[aria-selected='true']) {
		border-color: var(--line);
		background: var(--paper-deep);
	}

	.search-results :global(.search-result strong) {
		color: var(--teal-dark);
		font-size: 0.95rem;
		font-weight: 650;
	}

	.search-results :global(.search-result span) {
		display: block;
		color: var(--muted);
		line-height: 1.45;
	}

	.search-results :global(.search-result mark) {
		background: var(--mark-surface);
		color: var(--mark-ink);
	}

	.search-results :global(.search-message) {
		margin: 0;
		padding: 0.75rem;
		color: var(--muted);
		font-size: 0.88rem;
	}

	.dialog-footer {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem 1rem;
		padding-top: 0.75rem;
		border-top: 1px solid var(--line);
		color: var(--muted);
		font-size: 0.72rem;
	}

	.dialog-footer kbd {
		margin-top: 0.25rem;
		margin-right: 0.25rem;
		font-size: 0.625rem;
	}

	@keyframes search-dialog-in {
		from {
			opacity: 0;
			transform: translateY(-0.5rem) scale(0.99);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}

	@media (max-width: 900px) {
		.search {
			width: 100%;
		}
	}

	@media (max-width: 520px) {
		.shortcut-hint {
			font-size: 0.62rem;
		}

		.command-dialog {
			width: calc(100vw - 0.75rem);
			max-height: calc(100dvh - 0.75rem);
			border-radius: 0.5rem;
		}

		.dialog-inner {
			max-height: calc(100dvh - 0.75rem);
			padding: 0.9rem;
		}

		.dialog-footer {
			gap: 0.55rem 0.8rem;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.command-dialog[open] {
			animation: none;
		}
	}
</style>
