import { formatNumber } from "../ui/format";

const WIDTH = 600;
const HEIGHT = 160;
const PAD = 10;

export type ChartGeometry = { line: string; area: string; max: number };

/** Turns values into an SVG line and a filled area under it, scaled into a width by height box. */
export function chartGeometry(values: number[], width: number, height: number, pad: number): ChartGeometry {
	const max = Math.max(0, ...values);
	const innerW = width - 2 * pad;
	const innerH = height - 2 * pad;
	const x = (i: number) => (values.length === 1 ? width / 2 : pad + (i / (values.length - 1)) * innerW);
	const y = (v: number) => (max === 0 ? height - pad : height - pad - (v / max) * innerH);
	const points = values.map((v, i) => `${x(i)},${y(v)}`);
	const line = points.map((point, i) => `${i === 0 ? "M" : "L"}${point}`).join(" ");
	const area = values.length < 2 ? "" : `${line} L${x(values.length - 1)},${height - pad} L${x(0)},${height - pad} Z`;
	return { line, area, max };
}

type Props = { title: string; daily: { date: string; value: number }[]; color: string };

/** A plain SVG line chart of one daily number. No chart library. */
export function TrafficChart({ title, daily, color }: Props) {
	if (daily.length === 0) {
		return (
			<div className="grid gap-2 rounded-md border border-line bg-raised p-4">
				<h3 className="text-sm font-medium text-ink">{title}</h3>
				<p className="text-xs text-ink-muted">No daily numbers yet.</p>
			</div>
		);
	}
	const values = daily.map((d) => d.value);
	const { line, area, max } = chartGeometry(values, WIDTH, HEIGHT, PAD);
	const total = values.reduce((sum, v) => sum + v, 0);
	const first = daily[0].date;
	const last = daily[daily.length - 1].date;
	const label = `${title} per day, ${first} to ${last}: ${formatNumber(total)} in total, highest day ${formatNumber(max)}.`;
	return (
		<div className="grid gap-2 rounded-md border border-line bg-raised p-4">
			<div className="flex items-baseline justify-between gap-2">
				<h3 className="text-sm font-medium text-ink">{title}</h3>
				<span className="text-xs text-ink-muted">Highest day {formatNumber(max)}</span>
			</div>
			<svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} role="img" aria-label={label} className="h-auto w-full" preserveAspectRatio="none">
				<line x1={PAD} y1={HEIGHT - PAD} x2={WIDTH - PAD} y2={HEIGHT - PAD} stroke="#34343a" strokeWidth="1" />
				{area && <path d={area} fill={color} fillOpacity="0.08" stroke="none" />}
				<path d={line} fill="none" stroke={color} strokeWidth="2" strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />
			</svg>
			<div className="flex justify-between text-[11px] text-ink-muted">
				<span>{first}</span>
				<span>{last}</span>
			</div>
		</div>
	);
}
