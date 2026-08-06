<script lang="ts">
	let { id }: { id: string } = $props();
</script>

<div class="search" data-doc-search>
	<form role="search" action="/search/" method="get">
		<label class="sr-only" for={id}>Search documentation</label>
		<input {id} name="q" type="search" data-search-input placeholder="Search docs" autocomplete="off" />
		<button type="submit" aria-label="Search documentation">⌕</button>
	</form>
	<div class="search-results" data-search-results aria-live="polite"></div>
</div>

<style>
	.search {
		position: relative;
		width: 13.5rem;
	}

	.search form {
		display: flex;
		align-items: center;
		border: 1px solid var(--line);
		border-radius: 0.35rem;
		background: var(--surface-raised);
	}

	.search input {
		width: 100%;
		min-width: 0;
		padding: 0.42rem 0.55rem;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--ink);
		font-size: 0.86rem;
	}

	.search form:focus-within {
		border-color: var(--teal);
		box-shadow: 0 0 0 2px var(--focus-soft);
	}

	.search button {
		width: 2.2rem;
		align-self: stretch;
		border: 0;
		border-left: 1px solid var(--line);
		background: transparent;
		color: var(--teal-dark);
		cursor: pointer;
	}

	.search-results {
		position: absolute;
		top: calc(100% + 0.5rem);
		right: 0;
		z-index: 15;
		display: grid;
		width: min(26rem, 80vw);
		max-height: 25rem;
		overflow-y: auto;
		border: 1px solid var(--line);
		background: var(--surface-raised);
		box-shadow: var(--shadow);
	}

	.search-results:empty {
		display: none;
	}

	.search-results :global(a),
	.search-results :global(.search-message) {
		display: grid;
		gap: 0.18rem;
		margin: 0;
		padding: 0.8rem 0.9rem;
		font-size: 0.82rem;
		text-decoration: none;
	}

	.search-results :global(a + a) {
		border-top: 1px solid var(--line);
	}

	.search-results :global(a:hover) {
		background: var(--paper-deep);
	}

	.search-results :global(strong) {
		color: var(--teal-dark);
	}

	.search-results :global(span) {
		display: block;
		color: var(--muted);
		line-height: 1.4;
	}

	.search-results :global(mark) {
		background: var(--mark-surface);
		color: var(--mark-ink);
	}

	.search-results :global(.search-message) {
		color: var(--muted);
	}
</style>
