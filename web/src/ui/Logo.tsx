/** The product name with a small neutral mark. */
export function Logo() {
	return (
		<span className="inline-flex items-center gap-2">
			<svg viewBox="0 0 16 16" className="h-4 w-4 shrink-0 text-ink-soft" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden>
				<rect x="2.75" y="2.75" width="10.5" height="10.5" rx="2" />
				<path d="M5.5 6h5M5.5 8.5h5M5.5 11h3" strokeLinecap="round" />
			</svg>
			<span className="text-sm font-medium text-ink">Margincite</span>
		</span>
	);
}
