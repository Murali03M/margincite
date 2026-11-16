import type { ReactNode } from "react";

export type Tone = "line" | "course" | "open" | "ink";

const DOT_CLASS: Record<Tone, string> = {
	line: "dot-info",
	course: "dot-good",
	open: "dot-warn",
	ink: "dot-bad",
};

/** A small 1px-bordered tag in neutral text. Words carry the meaning; the dot only helps the eye. */
export function StatusTag({ tone = "line", children }: { tone?: Tone; children: ReactNode }) {
	return (
		<span className="tag">
			<span className={`dot ${DOT_CLASS[tone]}`} aria-hidden />
			{children}
		</span>
	);
}
