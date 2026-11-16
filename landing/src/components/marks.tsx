/** Drawn marks for the evidence room. One stroke weight, round joins, colours from the tokens. */

type MarkProps = { className?: string };

/** A yellow evidence marker: a folded A-frame tent with its number printed on the front. */
export function Marker({ n, className }: MarkProps & { n: number | string }) {
	return (
		<svg viewBox="0 0 48 44" className={className} aria-hidden>
			<path d="M37 5 46 38 43 40 36.2 9Z" fill="var(--color-marker-deep)" />
			<path d="M11 5h26l7 35H4Z" fill="var(--color-marker)" stroke="var(--color-ink)" strokeWidth="1.5" strokeLinejoin="round" />
			<path d="M11 5h26l-.8 4H11.8Z" fill="var(--color-marker-deep)" />
			<text
				x="24"
				y="32.5"
				textAnchor="middle"
				fill="var(--color-ink)"
				style={{ fontFamily: "var(--font-display)", fontWeight: 850, fontSize: 19, fontStretch: "100%" }}
			>
				{n}
			</text>
		</svg>
	);
}

/** A marker not yet set down: the same tent drawn as a dashed outline. Marks things coming later. */
export function PlannedMarker({ className }: MarkProps) {
	return (
		<svg viewBox="0 0 48 44" className={className} aria-hidden>
			<path
				d="M11 5h26l7 35H4Z"
				fill="none"
				stroke="currentColor"
				strokeWidth="2"
				strokeDasharray="4 3.2"
				strokeLinejoin="round"
			/>
		</svg>
	);
}

export function ArrowIcon({ className }: MarkProps) {
	return (
		<svg viewBox="0 0 20 20" className={className ?? "h-4 w-4"} aria-hidden>
			<path d="M3 10h13M11 5l5 5-5 5" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
		</svg>
	);
}

/** The GitHub mark, used only on links that go to the repository. */
export function GitHubMark({ className }: MarkProps) {
	return (
		<svg viewBox="0 0 16 16" className={className ?? "h-[1.15rem] w-[1.15rem]"} aria-hidden>
			<path
				fill="currentColor"
				d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
			/>
		</svg>
	);
}

export function CopyIcon({ className }: MarkProps) {
	return (
		<svg viewBox="0 0 20 20" className={className ?? "h-4 w-4"} aria-hidden>
			<rect x="6.5" y="6.5" width="10" height="10" rx="1.5" fill="none" stroke="currentColor" strokeWidth="2" />
			<path d="M13.5 3.5h-8a2 2 0 0 0-2 2v8" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
		</svg>
	);
}

export function CheckIcon({ className }: MarkProps) {
	return (
		<svg viewBox="0 0 20 20" className={className ?? "h-4 w-4"} aria-hidden>
			<path d="m4 10.5 4 4 8-9" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
		</svg>
	);
}

export function MenuIcon() {
	return (
		<svg viewBox="0 0 24 24" className="h-6 w-6" aria-hidden>
			<path d="M4 7h16M4 12h16M4 17h16" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" />
		</svg>
	);
}

export function CloseIcon() {
	return (
		<svg viewBox="0 0 24 24" className="h-6 w-6" aria-hidden>
			<path d="M6 6l12 12M18 6 6 18" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" />
		</svg>
	);
}
