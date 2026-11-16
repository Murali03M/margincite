import { REPO_URL, content } from "../../src/content";

const text = JSON.stringify(content);

it("makes no invented number claims", () => {
	expect(text).not.toMatch(/\d[\d,]*\+/);
	expect(text).not.toMatch(/trusted by|customers love|#1/i);
});

it("links only to sections that exist, or to the repository", () => {
	const ids = ["#top", "#features", "#layers", "#how", "#product", "#faq", "#get-it"];
	const hrefs = [...content.nav.links, content.nav.cta, content.hero.primary, content.hero.secondary].map((l) => l.href);
	for (const href of hrefs) expect([...ids, REPO_URL]).toContain(href);
});

it("points every GitHub action at the public repository", () => {
	expect(REPO_URL).toBe("https://github.com/Murali03M/margincite");
	expect(content.getIt.commands[0]).toBe(`git clone ${REPO_URL}.git`);
});

it("labels the illustrated finding as an example", () => {
	expect(content.hero.exhibit.badge).toBe("Example");
	expect(content.hero.exhibit.label).toMatch(/^Example finding, illustrated/);
});

it("puts the Claude connector under coming later, not under today", () => {
	expect(JSON.stringify(content.features)).not.toMatch(/connector/i);
	expect(content.product.coming.items.join(" ")).toMatch(/connector/);
});
