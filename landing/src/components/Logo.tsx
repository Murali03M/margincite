import { content } from "../content";

/** A map sheet with a ruled margin, and one control circled in that margin. */
export function Logo({ className }: { className?: string }) {
	return (
		<span className={"inline-flex items-center gap-2.5 text-course " + (className ?? "")}>
			<svg viewBox="0 0 40 40" className="h-9 w-9 shrink-0" aria-hidden>
				<rect x="2.5" y="2.5" width="35" height="35" rx="3" fill="#fff" stroke="currentColor" strokeWidth="3" />
				<path d="M15 3v34" stroke="currentColor" strokeWidth="2" />
				<circle cx="9" cy="14" r="3.6" fill="currentColor" />
				<path d="M20 12h12M20 20h9M20 28h12" stroke="var(--color-contour)" strokeWidth="2.2" strokeLinecap="round" />
			</svg>
			<span className="font-[family-name:var(--font-display)] text-[2rem] uppercase leading-none tracking-[0.03em]">
				{content.brand}
			</span>
		</span>
	);
}
