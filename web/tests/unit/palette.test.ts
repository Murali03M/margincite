import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const SRC = join(__dirname, "../../src");
const OLD = /indigo|violet|purple|fuchsia|gradient|shadow-|backdrop-blur|#6366f1|#4f46e5|#8b5cf6/i;

function sourceFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		return statSync(path).isDirectory() ? sourceFiles(path) : [path];
	});
}

test("no source file uses the old accent colours, gradients, shadows or blur", () => {
	const hits = sourceFiles(SRC).filter((file) => OLD.test(readFileSync(file, "utf8")));
	expect(hits).toEqual([]);
});

test("the stylesheet has no box shadow, glow or blur and sets the neutral palette", () => {
	const css = readFileSync(join(SRC, "index.css"), "utf8");
	expect(css).not.toMatch(/box-shadow\s*:\s*(?!none)/);
	expect(css).not.toMatch(/blur\(|gradient\(/);
	for (const colour of ["#0a0a0b", "#0f0f10", "#161618", "#1d1d20", "#26262a", "#34343a", "#ececee", "#a0a0a8"]) {
		expect(css).toContain(colour);
	}
});

function luminance(hex: string): number {
	const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) =>
		c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4,
	);
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (hi + 0.05) / (lo + 0.05);
}

test("every text colour reads at 4.5:1 on every surface it sits on", () => {
	const css = readFileSync(join(SRC, "index.css"), "utf8");
	const token = (name: string) => css.match(new RegExp(`--color-${name}:\\s*(#[0-9a-f]{6})`, "i"))?.[1] as string;
	for (const text of ["ink", "ink-soft", "ink-muted"]) {
		for (const surface of ["canvas", "bar", "raised", "hover"]) {
			expect(contrast(token(text), token(surface))).toBeGreaterThanOrEqual(4.5);
		}
	}
	for (const mark of ["good", "warn", "bad", "info"]) {
		expect(contrast(token(mark), token("canvas"))).toBeGreaterThanOrEqual(3);
	}
});
