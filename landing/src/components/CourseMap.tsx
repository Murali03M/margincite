import { content } from "../content";
import { asset } from "../asset";

/**
 * The hero illustration: an example site drawn as terrain, with the first
 * three fixes marked as an orienteering course. The terrain is a static SVG
 * built by scripts/generate-map.mjs; the course is overprinted on top here.
 */
type Point = { x: number; y: number };
type CourseMapProps = {
	viewBox: string;
	/** Draw each fix's title beside its control. Off where the map is small. */
	tags?: boolean;
	/** Lettering size in map units. */
	labelSize?: number;
	className?: string;
};

const RADIUS = 31;
const START: Point = { x: 722, y: 505 };
const CONTROLS: readonly Point[] = [
	{ x: 980, y: 250 },
	{ x: 1214, y: 404 },
	{ x: 1090, y: 690 },
];
const EXIT: Point = { x: 936, y: 912 };
const PLACES: readonly Point[] = [
	{ x: 858, y: 132 },
	{ x: 1380, y: 500 },
	{ x: 1484, y: 262 },
	{ x: 972, y: 594 },
];

const halo = (size: number) =>
	({ stroke: "#fff", strokeWidth: size, strokeLinejoin: "round", paintOrder: "stroke" }) as const;

const step = (index: number) => ({ ["--step" as string]: index });

/** A straight leg between two marks, stopping short of both, as map convention has it. */
function leg(from: Point, to: Point, gapFrom: number, gapTo: number) {
	const length = Math.hypot(to.x - from.x, to.y - from.y);
	const ux = (to.x - from.x) / length;
	const uy = (to.y - from.y) / length;
	return {
		x1: from.x + ux * gapFrom,
		y1: from.y + uy * gapFrom,
		x2: to.x - ux * gapTo,
		y2: to.y - uy * gapTo,
	};
}

/** Roughly how wide a line of Barlow Semi Condensed 600 sets, in ems. */
export function textWidth(text: string): number {
	return [...text].reduce((sum, char) => {
		if (/[iljtfr.,:' ]/.test(char)) return sum + 0.27;
		if (/[mwMW]/.test(char)) return sum + 0.68;
		if (/[A-Z]/.test(char)) return sum + 0.52;
		return sum + 0.45;
	}, 0);
}

function Course() {
	const stops = [START, ...CONTROLS];
	// The start triangle points at the first control.
	const bearing = (Math.atan2(CONTROLS[0].y - START.y, CONTROLS[0].x - START.x) * 180) / Math.PI + 90;
	return (
		<g data-course fill="none" stroke="var(--color-course)" strokeWidth="6" strokeLinecap="round">
			{CONTROLS.map((control, i) => (
				<line
					key={i}
					className="course-leg"
					pathLength={1}
					style={step(i)}
					{...leg(stops[i], control, i === 0 ? 38 : RADIUS, RADIUS)}
				/>
			))}
			<line
				className="course-leg"
				pathLength={1}
				style={step(CONTROLS.length)}
				{...leg(CONTROLS[CONTROLS.length - 1], EXIT, RADIUS, 0)}
			/>
			<path
				d={`M${START.x} ${START.y - 36}l31 54h-62Z`}
				strokeLinejoin="round"
				transform={`rotate(${bearing.toFixed(1)} ${START.x} ${START.y})`}
			/>
			{CONTROLS.map((control, i) => (
				<circle
					key={i}
					data-control={i + 1}
					className="course-mark"
					style={step(i)}
					cx={control.x}
					cy={control.y}
					r={RADIUS}
					fill="none"
				/>
			))}
		</g>
	);
}

function ControlLabel({ index, tags, size }: { index: number; tags: boolean; size: number }) {
	const control = CONTROLS[index];
	const note = content.hero.sketchNotes[index];
	const height = size * 2.05;
	const width = (textWidth(note) + 1.5) * size;
	const tagX = control.x + RADIUS + 16;
	const tagY = control.y - height / 2 - (index === 0 ? 14 : 0);
	// Control 2's number sits above it, because a leg arrives where the others carry theirs.
	const above = index === 1;
	return (
		<g>
			<text
				className="course-mark"
				style={step(index)}
				x={above ? control.x : control.x - RADIUS - 12}
				y={above ? control.y - RADIUS - 14 : control.y - RADIUS + 14}
				textAnchor={above ? "middle" : "end"}
				fontFamily="var(--font-display)"
				fontSize={size * 3.3}
				fill="var(--color-course)"
				{...halo(size * 0.42)}
			>
				{index + 1}
			</text>
			{tags && (
				<g className="course-tag" style={step(index)}>
					<rect x={tagX} y={tagY} width={width} height={height} rx="4" fill="#fff" stroke="var(--color-course)" strokeWidth="2.5" />
					<text
						x={tagX + width / 2}
						y={tagY + height / 2 + size * 0.35}
						textAnchor="middle"
						fontFamily="var(--font-body)"
						fontSize={size}
						fontWeight="600"
						fill="var(--color-ink)"
					>
						{note}
					</text>
				</g>
			)}
		</g>
	);
}

/** "Start here", on a white knock-out so no contour runs through the words. */
function StartLabel({ size }: { size: number }) {
	const label = content.hero.arrowNote.toUpperCase();
	const width = (textWidth(label) + label.length * 0.09 + 0.9) * size;
	const x = START.x - 6;
	const y = START.y + 68;
	return (
		<g>
			<rect data-start-label x={x - width / 2} y={y - size * 1.05} width={width} height={size * 1.5} rx="3" fill="#fff" />
			<text
				x={x}
				y={y}
				textAnchor="middle"
				fontFamily="var(--font-body)"
				fontSize={size}
				fontWeight="600"
				letterSpacing="0.09em"
				fill="var(--color-course-deep)"
			>
				{label}
			</text>
		</g>
	);
}

export function CourseMap({ viewBox, tags = false, labelSize = 21, className }: CourseMapProps) {
	const fixes = content.hero.sketchNotes.map((note, i) => `${i + 1}: ${note}`).join(". ");
	return (
		<svg
			viewBox={viewBox}
			preserveAspectRatio="xMidYMid slice"
			className={className}
			role="img"
			aria-label={`${content.hero.mapLabel} ${fixes}.`}
		>
			<image href={asset("map/terrain.svg")} width="1600" height="900" />

			<g
				fontFamily="var(--font-body)"
				fontSize={labelSize * 1.05}
				fontWeight="600"
				letterSpacing="0.06em"
				fill="var(--color-ink)"
				textAnchor="middle"
				{...halo(labelSize * 0.32)}
			>
				{content.hero.places.map((place, i) => (
					<text key={place} x={PLACES[i].x} y={PLACES[i].y}>
						{place}
					</text>
				))}
			</g>

			<Course />
			<StartLabel size={labelSize * 0.92} />

			{CONTROLS.map((_, i) => (
				<ControlLabel key={i} index={i} tags={tags} size={labelSize} />
			))}
		</svg>
	);
}
