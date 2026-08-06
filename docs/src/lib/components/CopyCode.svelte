<script lang="ts">
	import { onMount } from 'svelte';

	onMount(() => {
		const cleanups = Array.from(document.querySelectorAll<HTMLElement>('.doc-content pre')).map((pre) => {
			const code = pre.querySelector('code');
			if (!code) return () => {};
			const codeElement = code;

			const button = document.createElement('button');
			button.type = 'button';
			button.className = 'copy-code';
			button.setAttribute('aria-label', 'Copy code');
			button.setAttribute('aria-live', 'polite');

			const icon = document.createElement('span');
			icon.className = 'i-ri-file-copy-line';
			icon.setAttribute('aria-hidden', 'true');
			const label = document.createElement('span');
			label.textContent = 'Copy';
			button.append(icon, label);

			async function copy(): Promise<void> {
				try {
					await navigator.clipboard.writeText(codeElement.textContent ?? '');
					label.textContent = 'Copied';
					button.setAttribute('aria-label', 'Code copied');
				} catch {
					label.textContent = 'Copy failed';
					button.setAttribute('aria-label', 'Code copy failed');
				}
				window.setTimeout(() => {
					label.textContent = 'Copy';
					button.setAttribute('aria-label', 'Copy code');
				}, 1600);
			}

			button.addEventListener('click', copy);
			pre.append(button);
			return () => {
				button.removeEventListener('click', copy);
				button.remove();
			};
		});

		return () => cleanups.forEach((cleanup) => cleanup());
	});
</script>
