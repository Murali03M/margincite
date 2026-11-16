// @vitest-environment node
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { content } from "../../src/content";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "dist");
const html = readFileSync(join(dist, "index.html"), "utf8");
const root = html.slice(html.indexOf('<div id="root">'));
const assets = readdirSync(join(dist, "assets"));
const css = assets
	.filter((file) => file.endsWith(".css"))
	.map((file) => readFileSync(join(dist, "assets", file), "utf8"))
	.join("");

it("has every section's text in the HTML itself", () => {
	for (const text of [
		content.hero.titleAccent,
		content.hero.body,
		content.hero.sketchNotes[0],
		content.features.heading,
		content.features.items[3].body,
		content.how.steps[2].title,
		content.product.note.title,
		content.product.honesty,
		content.cta.heading,
	]) expect(root).toContain(text);
});

it("hides nothing and leaves no template markers", () => {
	expect(html).not.toMatch(/opacity:\s*0[;"]/);
	expect(html).not.toContain("<!--app-head-->");
	expect(html).not.toContain("<!--app-html-->");
});

it("has exactly one title and one canonical link", () => {
	expect(html.match(/<title>/g)).toHaveLength(1);
	expect(html.match(/rel="canonical"/g)).toHaveLength(1);
});

it("serves its own fonts", () => {
	expect(css).toMatch(/@font-face\s*\{[^}]*League Gothic/);
	expect(css).toMatch(/@font-face\s*\{[^}]*Barlow Semi Condensed/);
	expect(html + css).not.toContain("fonts.googleapis.com");
	expect(assets.some((file) => file.endsWith(".woff2"))).toBe(true);
});

it("ships the map artwork as static files with no scripts in them", () => {
	for (const name of ["terrain.svg", "contours-brown.svg", "contours-white.svg"]) {
		const svg = readFileSync(join(dist, "map", name), "utf8");
		expect(svg.startsWith("<svg")).toBe(true);
		expect(svg).not.toMatch(/<script|onload=|href="http/i);
	}
});

it("keeps the direction contract in the built page", () => {
	expect(html).toContain("Seed key b960e27d");
});

it("writes robots.txt and sitemap.xml", () => {
	expect(readFileSync(join(dist, "robots.txt"), "utf8")).toContain("Sitemap: ");
	expect(readFileSync(join(dist, "sitemap.xml"), "utf8")).toContain("<loc>");
	expect(existsSync(join(dist, "..", "dist-ssr"))).toBe(false);
});
