// @vitest-environment node
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { content } from "../../src/content";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "dist");
const html = readFileSync(join(dist, "index.html"), "utf8");
const root = html.slice(html.indexOf('<div id="root">'));
const text = root.replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&");
const assets = readdirSync(join(dist, "assets"));
const css = assets
	.filter((file) => file.endsWith(".css"))
	.map((file) => readFileSync(join(dist, "assets", file), "utf8"))
	.join("");

it("has every section's text in the HTML itself", () => {
	for (const line of [
		content.hero.titleAccent,
		content.hero.body,
		content.hero.exhibit.tag.rows[1].value,
		content.features.heading,
		content.features.items[3].body,
		content.layers.rows[2].cells[2],
		content.how.steps[2].title,
		content.product.note.title,
		content.product.sample.rows[2].title,
		content.product.honesty,
		content.faq.items[3].answer,
		content.getIt.heading,
		content.getIt.commands[0],
	]) expect(text).toContain(line);
});

it("prints no visit count into the HTML", () => {
	expect(root).not.toMatch(/data-digit/);
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
	expect(css).toMatch(/@font-face\s*\{[^}]*Archivo Variable/);
	expect(css).toMatch(/@font-face\s*\{[^}]*Public Sans/);
	expect(css).toMatch(/@font-face\s*\{[^}]*JetBrains Mono/);
	expect(html + css).not.toContain("fonts.googleapis.com");
	expect(assets.some((file) => file.endsWith(".woff2"))).toBe(true);
});

it("keeps the direction contract in the built page", () => {
	expect(html).toContain("seed key cb30d7e8");
});

it("writes robots.txt and sitemap.xml", () => {
	expect(readFileSync(join(dist, "robots.txt"), "utf8")).toContain("Sitemap: ");
	expect(readFileSync(join(dist, "sitemap.xml"), "utf8")).toContain("<loc>");
	expect(existsSync(join(dist, "..", "dist-ssr"))).toBe(false);
});
