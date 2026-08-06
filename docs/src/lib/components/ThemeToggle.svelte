<script lang="ts">
	type Theme = 'light' | 'dark';

	const storageKey = 'stormbuffer-theme';
	const lightThemeColor = '#f7f5ef';
	const darkThemeColor = '#10211d';

	let isDark = $state(false);

	function readStoredTheme(): Theme | null {
		try {
			const value = window.localStorage.getItem(storageKey);
			return value === 'light' || value === 'dark' ? value : null;
		} catch {
			return null;
		}
	}

	function updateThemeColor(theme: Theme): void {
		document
			.querySelector('meta[name="theme-color"]')
			?.setAttribute('content', theme === 'dark' ? darkThemeColor : lightThemeColor);
	}

	function applyTheme(theme: Theme): void {
		document.documentElement.dataset.theme = theme;
		updateThemeColor(theme);
		try {
			window.localStorage.setItem(storageKey, theme);
		} catch {
			// The visual preference still applies when storage is unavailable.
		}
		isDark = theme === 'dark';
	}

	function toggleTheme(): void {
		applyTheme(isDark ? 'light' : 'dark');
	}

	$effect(() => {
		const systemTheme = window.matchMedia('(prefers-color-scheme: dark)');
		const storedTheme = readStoredTheme();
		const theme = storedTheme ?? (systemTheme.matches ? 'dark' : 'light');

		isDark = theme === 'dark';
		updateThemeColor(theme);

		function followSystemTheme(): void {
			if (!readStoredTheme()) {
				isDark = systemTheme.matches;
				updateThemeColor(systemTheme.matches ? 'dark' : 'light');
			}
		}

		systemTheme.addEventListener('change', followSystemTheme);
		return () => systemTheme.removeEventListener('change', followSystemTheme);
	});
</script>

<button
	class="theme-toggle"
	type="button"
	aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
	aria-pressed={isDark}
	title={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
	onclick={toggleTheme}>
	<span class="theme-toggle__label">{isDark ? 'Light' : 'Dark'}</span>
	<span class="theme-toggle__icon" aria-hidden="true">{isDark ? '☀' : '☾'}</span>
</button>

<style>
	.theme-toggle {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 0.45rem;
		min-width: 5.1rem;
		min-height: 2.75rem;
		padding: 0.45rem 0.7rem;
		border: 1px solid var(--line);
		border-radius: 999px;
		background: var(--surface-raised);
		color: var(--ink);
		font-size: 0.8rem;
		font-weight: 650;
		line-height: 1;
		cursor: pointer;
		transition:
			background-color 150ms ease,
			border-color 150ms ease,
			color 150ms ease,
			transform 150ms ease;
	}

	.theme-toggle:hover {
		border-color: var(--teal);
		background: var(--paper-deep);
		color: var(--teal-dark);
	}

	.theme-toggle:active {
		transform: translateY(1px);
	}

	.theme-toggle__icon {
		font-size: 1rem;
		line-height: 1;
	}

	@media (prefers-reduced-motion: reduce) {
		.theme-toggle {
			transition: none;
		}
	}
</style>
