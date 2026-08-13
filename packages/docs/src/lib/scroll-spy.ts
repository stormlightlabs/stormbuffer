const HEADING_OFFSET_TOLERANCE = 4;

export type HeadingPosition = { id: string; top: number; scrollMarginTop: number };

export type ViewportPosition = { scrollY: number; height: number; documentHeight: number };

export function findActiveHeading(headings: HeadingPosition[], viewport: ViewportPosition): string | null {
	if (headings.length === 0) return null;
	if (viewport.scrollY < 1) return null;

	if (viewport.scrollY + viewport.height >= viewport.documentHeight - 1) {
		return headings.at(-1)?.id ?? null;
	}

	let activeHeading: string | null = null;
	for (const heading of headings) {
		if (heading.top > viewport.scrollY + heading.scrollMarginTop + HEADING_OFFSET_TOLERANCE) {
			break;
		}
		activeHeading = heading.id;
	}

	return activeHeading;
}

export function startScrollSpy(headings: HTMLElement[], onChange: (activeHeading: string | null) => void): () => void {
	let animationFrame: number | undefined;
	let previousHeading: string | null | undefined;

	const update = () => {
		animationFrame = undefined;
		const scrollY = window.scrollY;
		const activeHeading = findActiveHeading(
			headings.map((heading) => ({
				id: heading.id,
				top: heading.getBoundingClientRect().top + scrollY,
				scrollMarginTop: Number.parseFloat(getComputedStyle(heading).scrollMarginTop) || 0
			})),
			{ scrollY, height: window.innerHeight, documentHeight: document.documentElement.scrollHeight }
		);

		if (activeHeading !== previousHeading) {
			previousHeading = activeHeading;
			onChange(activeHeading);
		}
	};

	const scheduleUpdate = () => {
		if (animationFrame === undefined) animationFrame = requestAnimationFrame(update);
	};

	update();
	window.addEventListener('scroll', scheduleUpdate, { passive: true });
	window.addEventListener('resize', scheduleUpdate);

	return () => {
		window.removeEventListener('scroll', scheduleUpdate);
		window.removeEventListener('resize', scheduleUpdate);
		if (animationFrame !== undefined) cancelAnimationFrame(animationFrame);
	};
}
