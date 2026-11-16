import { content } from "../../src/content";
import {
	AI_CRAWLERS,
	buildHead,
	buildJsonLd,
	buildRobots,
	buildSitemap,
	normalizeSiteUrl,
} from "../../src/seo";

const site = "https://margincite.com";

it("trims trailing slashes from the site address", () => {
	expect(normalizeSiteUrl("https://margincite.com/")).toBe(site);
	expect(normalizeSiteUrl("https://margincite.com///")).toBe(site);
});

it.each(["margincite.com", "ftp://margincite.com", ""])("refuses %s", (raw) => {
	expect(() => normalizeSiteUrl(raw)).toThrow("SITE_URL must start with http:// or https://");
});

it("builds the head tags", () => {
	const head = buildHead(site);
	expect(head).toContain(`<title>${content.seo.title}</title>`);
	expect(head).toContain(`<meta name="description" content="${content.seo.description}">`);
	expect(head).toContain(`<link rel="canonical" href="https://margincite.com/">`);
	expect(head).toContain(`<meta property="og:url" content="https://margincite.com/">`);
	expect(head).toContain(`<meta property="og:title" content="${content.seo.title}">`);
	expect(head).toContain(`<meta name="twitter:card" content="summary">`);
	expect(head).not.toContain("margincite.com//");
});

it("escapes characters that would break out of an attribute", () => {
	expect(buildHead('https://x.test/"><script>')).not.toContain('"><script>');
});

it("describes the organisation, site and app in structured data", () => {
	const types = buildJsonLd(site).map((entry) => (entry as { "@type": string })["@type"]);
	expect(types).toEqual(["Organization", "WebSite", "SoftwareApplication", "FAQPage"]);
});

it("invents no ratings, reviews or prices", () => {
	expect(JSON.stringify(buildJsonLd(site))).not.toMatch(/aggregateRating|review|offers|price/i);
});

it("embeds structured data that parses and cannot close its script tag", () => {
	const json = buildHead(site).match(/<script type="application\/ld\+json">(.*?)<\/script>/s)![1];
	expect(JSON.parse(json)).toHaveLength(4);
	expect(json).not.toContain("<");
});

it("lets every search and AI crawler in", () => {
	const robots = buildRobots(site);
	expect(robots).toContain("User-agent: *\nAllow: /");
	for (const bot of AI_CRAWLERS) expect(robots).toContain(`User-agent: ${bot}\nAllow: /`);
	expect(robots).not.toContain("Disallow");
	expect(robots).toContain("Sitemap: https://margincite.com/sitemap.xml");
});

it("names exactly the nine crawlers the product checks for", () => {
	expect(AI_CRAWLERS).toEqual(["GPTBot", "OAI-SearchBot", "ChatGPT-User", "ClaudeBot", "Claude-SearchBot", "Claude-User", "PerplexityBot", "Google-Extended", "Bingbot"]);
});

it("lists the one page in the sitemap", () => {
	const xml = buildSitemap(site, "2026-11-13");
	expect(xml).toContain("<loc>https://margincite.com/</loc>");
	expect(xml).toContain("<lastmod>2026-11-13</lastmod>");
});

it("says the app is free, open source and where its code lives", () => {
	const app = buildJsonLd(site)[2] as Record<string, unknown>;
	expect(app.isAccessibleForFree).toBe(true);
	expect(app.license).toBe("https://www.gnu.org/licenses/agpl-3.0.html");
	expect(app.sameAs).toEqual(["https://github.com/Murali03M/margincite"]);
});

it("answers the page's FAQ in structured data, word for word", () => {
	const faq = buildJsonLd(site)[3] as { mainEntity: { name: string; acceptedAnswer: { text: string } }[] };
	expect(faq.mainEntity.map((q) => q.name)).toEqual(content.faq.items.map((item) => item.question));
	expect(faq.mainEntity.map((q) => q.acceptedAnswer.text)).toEqual(content.faq.items.map((item) => item.answer));
});
