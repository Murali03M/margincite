#!/usr/bin/env node
/**
 * Draws the hero's orienteering map as static SVG files.
 *
 * The terrain is an illustration of an imaginary site, not data. Everything is
 * derived from one seed, so the output is identical on every run and the files
 * in public/map/ can be committed.
 *
 *   node scripts/generate-map.mjs
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const WIDTH = 1600;
export const HEIGHT = 900;
const CELL = 10;
const COLS = WIDTH / CELL + 1;
const ROWS = HEIGHT / CELL + 1;

const INK = {
	contour: "#B4652B",
	openLand: "#FFC533",
	roughOpen: "#FFE08A",
	thicket: "#BFDD8A",
	fight: "#7DB94A",
	water: "#2E9BDD",
	black: "#1D1524",
};

/** Small seeded generator, so the map never changes between runs. */
export function mulberry32(seed) {
	let a = seed >>> 0;
	return () => {
		a = (a + 0x6d2b79f5) >>> 0;
		let t = a;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

/** Smooth value noise on a lattice of the given size. */
export function makeNoise(seed, lattice = 64) {
	const random = mulberry32(seed);
	const values = Array.from({ length: lattice * lattice }, random);
	const at = (ix, iy) =>
		values[(((iy % lattice) + lattice) % lattice) * lattice + (((ix % lattice) + lattice) % lattice)];
	const fade = (t) => t * t * (3 - 2 * t);
	const sample = (x, y) => {
		const x0 = Math.floor(x);
		const y0 = Math.floor(y);
		const tx = fade(x - x0);
		const ty = fade(y - y0);
		const top = at(x0, y0) * (1 - tx) + at(x0 + 1, y0) * tx;
		const bottom = at(x0, y0 + 1) * (1 - tx) + at(x0 + 1, y0 + 1) * tx;
		return top * (1 - ty) + bottom * ty;
	};
	return (x, y, octaves = 4) => {
		let total = 0;
		let amplitude = 0.5;
		let frequency = 1;
		for (let i = 0; i < octaves; i += 1) {
			total += amplitude * sample(x * frequency, y * frequency);
			amplitude *= 0.5;
			frequency *= 2;
		}
		return total;
	};
}

const bump = (x, y, [cx, cy, radius, height]) =>
	height * Math.exp(-((x - cx) ** 2 + (y - cy) ** 2) / (2 * radius * radius));

/** Samples a function over the grid. `closed` pins the border low so every outline closes. */
export function sampleField(fn, closed = false) {
	const field = new Float64Array(COLS * ROWS);
	for (let row = 0; row < ROWS; row += 1) {
		for (let col = 0; col < COLS; col += 1) {
			const onBorder = row === 0 || col === 0 || row === ROWS - 1 || col === COLS - 1;
			field[row * COLS + col] = closed && onBorder ? -1 : fn(col * CELL, row * CELL);
		}
	}
	return field;
}

// Which edges of a grid cell each marching-squares case joins. Edges: 0 top, 1 right, 2 bottom, 3 left.
const CASES = [
	[], [[3, 2]], [[2, 1]], [[3, 1]], [[0, 1]], [[0, 3], [2, 1]], [[0, 2]], [[0, 3]],
	[[0, 3]], [[0, 2]], [[0, 1], [3, 2]], [[0, 1]], [[3, 1]], [[2, 1]], [[3, 2]], [],
];

/** Traces the lines where the field crosses `level`. Returns lists of [x, y] points. */
export function traceLevel(field, level) {
	const value = (col, row) => field[row * COLS + col];
	const cross = (a, b) => (level - a) / (b - a || 1e-9);
	const segments = [];
	for (let row = 0; row < ROWS - 1; row += 1) {
		for (let col = 0; col < COLS - 1; col += 1) {
			const tl = value(col, row);
			const tr = value(col + 1, row);
			const br = value(col + 1, row + 1);
			const bl = value(col, row + 1);
			const index = (tl > level ? 8 : 0) | (tr > level ? 4 : 0) | (br > level ? 2 : 0) | (bl > level ? 1 : 0);
			const edgePoint = (edge) => {
				if (edge === 0) return [(col + cross(tl, tr)) * CELL, row * CELL];
				if (edge === 1) return [(col + 1) * CELL, (row + cross(tr, br)) * CELL];
				if (edge === 2) return [(col + cross(bl, br)) * CELL, (row + 1) * CELL];
				return [col * CELL, (row + cross(tl, bl)) * CELL];
			};
			for (const [from, to] of CASES[index]) segments.push([edgePoint(from), edgePoint(to)]);
		}
	}
	return joinSegments(segments);
}

function joinSegments(segments) {
	const key = ([x, y]) => `${Math.round(x * 20)},${Math.round(y * 20)}`;
	const byEnd = new Map();
	const add = (k, entry) => byEnd.set(k, [...(byEnd.get(k) ?? []), entry]);
	segments.forEach((segment, index) => {
		add(key(segment[0]), { index, end: 0 });
		add(key(segment[1]), { index, end: 1 });
	});
	const used = new Set();
	const grow = (line, forwards) => {
		for (;;) {
			const tip = forwards ? line[line.length - 1] : line[0];
			const next = (byEnd.get(key(tip)) ?? []).find((entry) => !used.has(entry.index));
			if (!next) return line;
			used.add(next.index);
			const point = segments[next.index][next.end === 0 ? 1 : 0];
			line = forwards ? [...line, point] : [point, ...line];
		}
	};
	const lines = [];
	segments.forEach((segment, index) => {
		if (used.has(index)) return;
		used.add(index);
		lines.push(grow(grow([segment[0], segment[1]], true), false));
	});
	return lines;
}

const distance = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const isClosed = (line) => line.length > 3 && distance(line[0], line[line.length - 1]) < 0.5;

/** Drops points that sit closer together than `gap`, keeping the ends. */
export function thin(line, gap = 9) {
	const kept = [line[0]];
	for (const point of line.slice(1, -1)) {
		if (distance(point, kept[kept.length - 1]) >= gap) kept.push(point);
	}
	return [...kept, line[line.length - 1]];
}

const n = (value) => Math.round(value * 10) / 10;

/** Turns a point list into a smooth path, curving through the midpoints. */
export function toPath(rawLine) {
	const closed = isClosed(rawLine);
	const line = thin(rawLine);
	if (line.length < 4) return "";
	const mid = (a, b) => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
	if (closed) {
		const points = line.slice(0, -1);
		const start = mid(points[points.length - 1], points[0]);
		const curves = points.map((point, i) => {
			const m = mid(point, points[(i + 1) % points.length]);
			return `Q${n(point[0])} ${n(point[1])} ${n(m[0])} ${n(m[1])}`;
		});
		return `M${n(start[0])} ${n(start[1])}${curves.join("")}Z`;
	}
	const curves = line.slice(1, -1).map((point, i) => {
		const m = mid(point, line[i + 2]);
		return `Q${n(point[0])} ${n(point[1])} ${n(m[0])} ${n(m[1])}`;
	});
	const last = line[line.length - 1];
	return `M${n(line[0][0])} ${n(line[0][1])}${curves.join("")}L${n(last[0])} ${n(last[1])}`;
}

const shapes = (field, level, minPoints = 8) =>
	traceLevel(field, level)
		.filter((line) => line.length >= minPoints)
		.map(toPath)
		.filter(Boolean)
		.join("");

/** A patch of ground: soft blobs broken up by two scales of noise, so it reads as surveyed, not drawn. */
function patch(noise, blobs, roughness, offset) {
	return sampleField((x, y) => {
		const body = blobs.reduce((sum, blob) => sum + bump(x, y, blob), 0);
		const broad = noise(x / 170 + offset, y / 170 + offset) - 0.5;
		const fine = noise(x / 55 + offset * 2, y / 55 + offset * 2, 3) - 0.5;
		return body * 0.82 + roughness * (broad * 1.1 + fine * 0.75);
	}, true);
}

/** Wet ground follows the contours, so it is cut from a band of the relief around each centre. */
function wetGround(noise, height, centres, offset) {
	return sampleField((x, y) => {
		const wet = centres.reduce((most, centre) => {
			const along = bump(x, y, centre);
			const across = Math.abs(height(x, y) - height(centre[0], centre[1]));
			return Math.max(most, along - across * 3.4);
		}, 0);
		return wet + 0.28 * (noise(x / 60 + offset, y / 60 + offset, 3) - 0.5);
	}, true);
}

function smoothTrail(points) {
	const mid = (a, b) => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
	const curves = points.slice(1, -1).map((point, i) => {
		const m = mid(point, points[i + 2]);
		return `Q${point[0]} ${point[1]} ${n(m[0])} ${n(m[1])}`;
	});
	const last = points[points.length - 1];
	return `M${points[0][0]} ${points[0][1]}${curves.join("")}L${last[0]} ${last[1]}`;
}

const HILLS = [
	[980, 250, 150, 0.95], [1290, 420, 190, 0.8], [1090, 700, 150, 0.7], [1500, 170, 150, 0.75],
	[800, 560, 170, 0.5], [1480, 760, 170, 0.65], [640, 130, 190, 0.5], [300, 720, 240, 0.55],
	[180, 220, 220, 0.5], [470, 420, 150, 0.35],
];

export function buildTerrain(seed = 20261113) {
	const noise = makeNoise(seed);
	const height = (x, y) =>
		HILLS.reduce((sum, hill) => sum + bump(x, y, hill), 0) + 0.55 * noise(x / 260, y / 260);
	const relief = sampleField(height);

	const levels = Array.from({ length: 19 }, (_, i) => 0.2 + i * 0.075);
	const contours = levels.map((level, i) => ({ index: i % 5 === 2, d: shapes(relief, level, 6) }));
	// Form lines: dashed half-steps that show the shape of the ground between contours.
	const formLines = [0.4625, 0.6125, 0.7625, 0.9125].map((level) => shapes(relief, level, 10));

	const openLand = patch(noise, [[1330, 380, 110, 1], [1210, 300, 55, 0.9], [900, 830, 85, 1], [1540, 600, 65, 0.9], [420, 200, 60, 0.9], [250, 560, 75, 0.95], [690, 330, 45, 0.85]], 0.62, 3);
	const roughOpen = patch(noise, [[1330, 390, 150, 1], [900, 830, 120, 1], [760, 330, 70, 0.9], [250, 560, 110, 0.95], [1100, 420, 45, 0.8]], 0.62, 9);
	const thicket = patch(noise, [[1150, 540, 100, 1], [1480, 250, 100, 1], [860, 150, 85, 1], [1400, 690, 95, 1], [700, 760, 75, 0.95], [1050, 80, 65, 0.9], [130, 110, 85, 0.95], [520, 820, 80, 0.95], [330, 380, 55, 0.85], [1290, 90, 45, 0.85], [960, 470, 40, 0.8]], 0.72, 17);
	const fight = patch(noise, [[1160, 545, 55, 1], [1490, 245, 58, 1], [1405, 700, 50, 1], [860, 150, 42, 0.95], [130, 110, 40, 0.9], [520, 820, 36, 0.9]], 0.5, 23);
	const water = wetGround(noise, height, [[1010, 610, 95, 1], [1235, 165, 70, 1], [1560, 430, 70, 0.95], [380, 690, 80, 0.95]], 31);

	const random = mulberry32(seed + 7);
	const scatter = (count) => Array.from({ length: count }, () => [n(20 + random() * 1560), n(20 + random() * 860), n(2 + random() * 1.8)]);

	const trails = [
		[[0, 610], [180, 640], [340, 560], [470, 600], [600, 700], [690, 760], [760, 610], [900, 520], [1040, 470], [1180, 430], [1330, 300], [1460, 130], [1560, 40]],
		[[700, 60], [820, 250], [940, 380], [1040, 470], [1120, 610], [1250, 760], [1330, 880]],
		[[1180, 430], [1320, 520], [1470, 540], [1600, 500]],
		[[240, 0], [300, 150], [250, 330], [340, 470], [470, 600]],
	].map(smoothTrail);

	return {
		contours,
		formLines,
		openLand: shapes(openLand, 0.5),
		roughOpen: shapes(roughOpen, 0.5),
		thicket: shapes(thicket, 0.5),
		fight: shapes(fight, 0.5),
		water: shapes(water, 0.5),
		boulders: scatter(70),
		knolls: scatter(22),
		trails,
	};
}

/** Magnetic north lines: thin, parallel, a few degrees off the sheet's vertical. */
function northLines() {
	const lines = [];
	for (let x = -40; x < WIDTH + 120; x += 150) lines.push(`M${x} ${HEIGHT}L${x - 64} 0`);
	return `<path d="${lines.join("")}" fill="none" stroke="${INK.water}" stroke-width="1.1" stroke-opacity=".6"/>`;
}

const contourPaths = (terrain, colour) =>
	terrain.contours
		.map(({ index, d }) => `<path d="${d}" stroke-width="${index ? 2.4 : 1.15}"/>`)
		.join("")
		.replace(/^/, `<g fill="none" stroke="${colour}" stroke-linecap="round" stroke-linejoin="round">`) + "</g>";

export function terrainSvg(terrain) {
	return [
		`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${WIDTH} ${HEIGHT}" preserveAspectRatio="xMidYMid slice">`,
		"<defs>",
		`<pattern id="marsh" width="10" height="7" patternUnits="userSpaceOnUse"><path d="M0 3.5H10" stroke="${INK.water}" stroke-width="2.2"/></pattern>`,
		`<pattern id="screen" width="9" height="9" patternUnits="userSpaceOnUse"><rect width="9" height="9" fill="${INK.fight}"/><circle cx="2.5" cy="2.5" r="1.1" fill="#5C9A33"/><circle cx="7" cy="7" r="1.1" fill="#5C9A33"/></pattern>`,
		"</defs>",
		`<rect width="${WIDTH}" height="${HEIGHT}" fill="#fff"/>`,
		`<path d="${terrain.roughOpen}" fill="${INK.roughOpen}" fill-rule="evenodd"/>`,
		`<path d="${terrain.openLand}" fill="${INK.openLand}" fill-rule="evenodd"/>`,
		`<path d="${terrain.thicket}" fill="${INK.thicket}" fill-rule="evenodd"/>`,
		`<path d="${terrain.fight}" fill="url(#screen)" fill-rule="evenodd"/>`,
		`<path d="${terrain.water}" fill="url(#marsh)" fill-rule="evenodd"/>`,
		northLines(),
		`<g fill="none" stroke="${INK.contour}" stroke-width="1" stroke-dasharray="7 4" stroke-linecap="round">${terrain.formLines.map((d) => `<path d="${d}"/>`).join("")}</g>`,
		contourPaths(terrain, INK.contour),
		`<g fill="${INK.contour}">${terrain.knolls.map(([x, y, r]) => `<circle cx="${x}" cy="${y}" r="${n(r + 1)}"/>`).join("")}</g>`,
		`<g fill="none" stroke="${INK.black}" stroke-width="1.7" stroke-opacity=".8" stroke-dasharray="9 6" stroke-linecap="round">${terrain.trails.map((d) => `<path d="${d}"/>`).join("")}</g>`,
		`<g fill="${INK.black}">${terrain.boulders.map(([x, y, r]) => `<circle cx="${x}" cy="${y}" r="${r}"/>`).join("")}</g>`,
		"</svg>",
	].join("");
}

/** Contour lines only, in one ink, used as a quiet ground on other sections. */
export function contoursSvg(terrain, colour) {
	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${WIDTH} ${HEIGHT}">${contourPaths(terrain, colour)}</svg>`;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
	const out = join(dirname(fileURLToPath(import.meta.url)), "..", "public", "map");
	mkdirSync(out, { recursive: true });
	const terrain = buildTerrain();
	const files = {
		"terrain.svg": terrainSvg(terrain),
		"contours-brown.svg": contoursSvg(terrain, INK.contour),
		"contours-white.svg": contoursSvg(terrain, "#ffffff"),
	};
	for (const [name, svg] of Object.entries(files)) {
		writeFileSync(join(out, name), svg);
		console.log(`${name}: ${(svg.length / 1024).toFixed(1)} KB`);
	}
}
