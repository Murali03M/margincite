import type { RankingKeyword } from "../api/types";
import { shortDate } from "../ui/format";

const WIDTH = 600;
const HEIGHT = 180;
const PAD_X = 28;
const PAD_Y = 14;

type Point = { at: string; position: number | null };

/** Splits the history into runs of checks where the site was found, so a gap is drawn as a gap. */
export function segments(history: Point[], x: (i: number) => number, y: (p: number) => number): string[][] {
	const runs: string[][] = [];
	let current: string[] = [];
	history.forEach((point, i) => {
		if (point.position === null) {
			if (current.length) runs.push(current);
			current = [];
		} else {
			current.push(`${x(i)},${y(point.position)}`);
		}
	});
	if (current.length) runs.push(current);
	return runs;
}

function describe(history: Point[]): string {
	return history
		.map((p) => `${p.position === null ? "not in the top 100" : `position ${p.position}`} on ${shortDate(p.at)}`)
		.join(", ");
}

/** A plain SVG line of a keyword's position over time. Position 1 is at the top; a gap means not in the top 100. */
export function PositionChart({ keyword, history }: { keyword: string; history: RankingKeyword["history"] }) {
	if (history.length === 0) return <p className="text-sm text-ink-muted">No history yet. It builds up with each check.</p>;

	const worst = Math.max(10, ...history.map((p) => p.position ?? 0));
	const innerW = WIDTH - 2 * PAD_X;
	const innerH = HEIGHT - 2 * PAD_Y;
	const x = (i: number) => (history.length === 1 ? WIDTH / 2 : PAD_X + (i / (history.length - 1)) * innerW);
	const y = (position: number) => PAD_Y + ((position - 1) / Math.max(1, worst - 1)) * innerH;
	const label = `Position history for ${keyword}, lower is better: ${describe(history)}.`;

	return (
		<div className="grid gap-1.5">
			<svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} role="img" aria-label={label} className="h-auto w-full max-w-2xl">
				<line x1={PAD_X} y1={PAD_Y} x2={WIDTH - PAD_X} y2={PAD_Y} stroke="#34343a" />
				<line x1={PAD_X} y1={HEIGHT - PAD_Y} x2={WIDTH - PAD_X} y2={HEIGHT - PAD_Y} stroke="#34343a" />
				<text x={4} y={PAD_Y + 4} fontSize="11" fill="#8a8a93">
					1
				</text>
				<text x={4} y={HEIGHT - PAD_Y + 4} fontSize="11" fill="#8a8a93">
					{worst}
				</text>
				{segments(history, x, y).map((run) => (
					<path key={run.join(" ")} data-line d={`M${run.join(" L")}`} fill="none" stroke="#ececee" strokeWidth="2" strokeLinejoin="round" strokeLinecap="round" />
				))}
				{history.map((point, i) =>
					point.position === null ? (
						<g key={point.at} data-gap stroke="#e5484d" strokeWidth="2" strokeLinecap="round">
							<line x1={x(i) - 5} y1={HEIGHT - PAD_Y - 5} x2={x(i) + 5} y2={HEIGHT - PAD_Y + 5} />
							<line x1={x(i) - 5} y1={HEIGHT - PAD_Y + 5} x2={x(i) + 5} y2={HEIGHT - PAD_Y - 5} />
						</g>
					) : (
						<circle key={point.at} cx={x(i)} cy={y(point.position)} r="4" fill="#ececee" />
					),
				)}
			</svg>
			<div className="flex max-w-2xl justify-between text-[11px] text-ink-muted">
				<span>{shortDate(history[0].at)}</span>
				<span>{shortDate(history[history.length - 1].at)}</span>
			</div>
			<p className="text-xs text-ink-muted">Position 1 is the top of Google. A red cross means the site was not in the top 100 that day.</p>
		</div>
	);
}
