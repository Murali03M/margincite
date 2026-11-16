import { content } from "../../src/content";

const text = JSON.stringify(content);

it("has none of the reference's fictional content", () => {
	expect(text).not.toMatch(/scribbly/i);
});

it("makes no invented number claims", () => {
	expect(text).not.toMatch(/\d[\d,]*\+/);
	expect(text).not.toMatch(/trusted by|customers love|#1/i);
});

it("links only to sections that exist", () => {
	const ids = ["#top", "#features", "#how", "#product", "#early-access"];
	const hrefs = [
		...content.nav.links,
		content.nav.cta,
		content.hero.primary,
		content.hero.secondary,
	].map((l) => l.href);
	for (const href of hrefs) expect(ids).toContain(href);
});
