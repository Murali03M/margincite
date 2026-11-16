/**
 * The map's own symbols, drawn in one weight. Every one is decorative:
 * the words beside it carry the meaning.
 */
type SymbolProps = { className?: string };

const stroke = {
	fill: "none",
	stroke: "currentColor",
	strokeWidth: 5,
	strokeLinecap: "round",
	strokeLinejoin: "round",
} as const;

export function StartSymbol({ className }: SymbolProps) {
	return (
		<svg viewBox="0 0 64 64" className={className} aria-hidden>
			<path d="M32 8 58 54H6Z" {...stroke} />
		</svg>
	);
}

export function ControlSymbol({ className, label }: SymbolProps & { label?: string }) {
	return (
		<svg viewBox="0 0 64 64" className={className} aria-hidden>
			<circle cx="32" cy="32" r="25" {...stroke} />
			{label && (
				<text
					x="32"
					y="43"
					textAnchor="middle"
					fontFamily="var(--font-display)"
					fontSize="32"
					fill="currentColor"
				>
					{label}
				</text>
			)}
		</svg>
	);
}

export function FinishSymbol({ className }: SymbolProps) {
	return (
		<svg viewBox="0 0 64 64" className={className} aria-hidden>
			<circle cx="32" cy="32" r="26" {...stroke} />
			<circle cx="32" cy="32" r="15" {...stroke} />
		</svg>
	);
}

/** A control not yet reached: the dashed circle of a planned leg. */
export function PlannedSymbol({ className }: SymbolProps) {
	return (
		<svg viewBox="0 0 64 64" className={className} aria-hidden>
			<circle cx="32" cy="32" r="25" {...stroke} strokeDasharray="9 9" />
		</svg>
	);
}

export function NorthArrow({ className, weight = 4 }: SymbolProps & { weight?: number }) {
	return (
		<svg viewBox="0 0 48 80" className={className} aria-hidden>
			<path d="M24 76V10M24 6 12 30M24 6l12 24" {...stroke} strokeWidth={weight} />
			<path d="M15 58V42l18 16V42" {...stroke} strokeWidth={weight} />
		</svg>
	);
}

export function ScaleBar({ className }: SymbolProps) {
	return (
		<svg viewBox="0 0 200 22" className={className} aria-hidden>
			<path d="M2 4v14h196V4M51 10v8M100 4v14M149 10v8" {...stroke} strokeWidth={3} />
			<path d="M2 18h49v-5H2zM100 18h49v-5h-49z" fill="currentColor" />
		</svg>
	);
}

export function ArrowIcon({ className }: SymbolProps) {
	return (
		<svg viewBox="0 0 24 24" className={className} width="22" height="22" aria-hidden>
			<path d="M4 12h15M13 6l6 6-6 6" {...stroke} strokeWidth={2.5} />
		</svg>
	);
}

export function MenuIcon({ className }: SymbolProps) {
	return (
		<svg viewBox="0 0 24 24" className={className} width="26" height="26" aria-hidden>
			<path d="M4 7h16M4 12h16M4 17h16" {...stroke} strokeWidth={2.5} />
		</svg>
	);
}

export function CloseIcon({ className }: SymbolProps) {
	return (
		<svg viewBox="0 0 24 24" className={className} width="26" height="26" aria-hidden>
			<path d="M6 6l12 12M18 6 6 18" {...stroke} strokeWidth={2.5} />
		</svg>
	);
}

export type KeySymbol = "contour" | "open" | "thicket" | "marsh" | "course";

/** One swatch from the map key, drawn the way the map draws it. */
export function KeySwatch({ symbol, className }: SymbolProps & { symbol: KeySymbol }) {
	return (
		<svg viewBox="0 0 48 30" className={className} aria-hidden>
			{symbol === "contour" && (
				<g fill="none" stroke="var(--color-contour)" strokeLinecap="round">
					<path d="M2 24C10 8 22 4 30 9s12 9 16 5" strokeWidth="2.6" />
					<path d="M2 29C12 16 22 12 29 16s11 8 17 5" strokeWidth="1.4" />
					<path d="M6 13C13 4 22 1 31 3" strokeWidth="1.4" />
				</g>
			)}
			{symbol === "open" && (
				<g>
					<rect x="1" y="1" width="46" height="28" rx="3" fill="var(--color-open)" stroke="var(--color-ink)" strokeWidth="1.5" />
					<path d="M3 21C11 12 19 22 27 14s12-2 18-7" fill="none" stroke="var(--color-contour)" strokeWidth="1.4" strokeLinecap="round" />
					<g fill="var(--color-ink)">
						<circle cx="12" cy="9" r="1.5" />
						<circle cx="24" cy="23" r="1.5" />
						<circle cx="36" cy="18" r="1.5" />
					</g>
				</g>
			)}
			{symbol === "thicket" && (
				<g>
					<rect x="1" y="1" width="46" height="28" rx="3" fill="var(--color-fight)" stroke="var(--color-ink)" strokeWidth="1.5" />
					<g fill="#4f8a2b">
						{[8, 17, 26, 35].map((x) => [8, 16, 24].map((y) => <circle key={`${x}-${y}`} cx={x + (y % 16 ? 0 : 4)} cy={y} r="1.5" />))}
					</g>
				</g>
			)}
			{symbol === "marsh" && (
				<g>
					<rect x="1" y="1" width="46" height="28" rx="3" fill="#ddf0fb" stroke="var(--color-ink)" strokeWidth="1.5" />
					<path d="M5 8h38M5 15h38M5 22h38" stroke="var(--color-marsh)" strokeWidth="2.4" />
				</g>
			)}
			{symbol === "course" && (
				<g fill="none" stroke="var(--color-course)" strokeWidth="3">
					<circle cx="10" cy="15" r="8" />
					<path d="M18 15h13" strokeLinecap="round" />
					<circle cx="39" cy="15" r="8" />
				</g>
			)}
		</svg>
	);
}
