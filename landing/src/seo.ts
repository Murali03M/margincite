import { REPO_URL, content } from "./content";

/** The crawlers Margincite checks robots.txt rules for. This page lets all of them in. */
export const AI_CRAWLERS: readonly string[] = [
	"GPTBot",
	"OAI-SearchBot",
	"ChatGPT-User",
	"ClaudeBot",
	"Claude-SearchBot",
	"Claude-User",
	"PerplexityBot",
	"Google-Extended",
	"Bingbot",
];

export function normalizeSiteUrl(raw: string): string {
	const trimmed = raw.trim().replace(/\/+$/, "");
	if (!/^https?:\/\/[^/]/.test(trimmed)) {
		throw new Error("SITE_URL must start with http:// or https://");
	}
	return trimmed;
}

const escapeAttribute = (value: string) =>
	value
		.replace(/&/g, "&amp;")
		.replace(/"/g, "&quot;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;");

export function buildJsonLd(siteUrl: string): object[] {
	const url = `${siteUrl}/`;
	return [
		{ "@context": "https://schema.org", "@type": "Organization", name: content.brand, url },
		{
			"@context": "https://schema.org",
			"@type": "WebSite",
			name: content.brand,
			url,
			description: content.seo.description,
		},
		{
			"@context": "https://schema.org",
			"@type": "SoftwareApplication",
			name: content.brand,
			description: content.seo.description,
			applicationCategory: "BusinessApplication",
			url,
			isAccessibleForFree: true,
			license: "https://www.gnu.org/licenses/agpl-3.0.html",
			sameAs: [REPO_URL],
		},
		{
			"@context": "https://schema.org",
			"@type": "FAQPage",
			mainEntity: content.faq.items.map((item) => ({
				"@type": "Question",
				name: item.question,
				acceptedAnswer: { "@type": "Answer", text: item.answer },
			})),
		},
	];
}

export function buildHead(siteUrl: string): string {
	const url = escapeAttribute(`${siteUrl}/`);
	const title = escapeAttribute(content.seo.title);
	const description = escapeAttribute(content.seo.description);
	// "<" is written as an escape so nothing inside the data can close the script tag.
	const jsonLd = JSON.stringify(buildJsonLd(siteUrl)).replace(/</g, "\\u003c");
	return [
		`<title>${title}</title>`,
		`<meta name="description" content="${description}">`,
		`<link rel="canonical" href="${url}">`,
		`<meta property="og:type" content="website">`,
		`<meta property="og:site_name" content="${escapeAttribute(content.brand)}">`,
		`<meta property="og:title" content="${title}">`,
		`<meta property="og:description" content="${description}">`,
		`<meta property="og:url" content="${url}">`,
		`<meta name="twitter:card" content="summary">`,
		`<script type="application/ld+json">${jsonLd}</script>`,
	].join("\n\t\t");
}

export function buildRobots(siteUrl: string): string {
	const blocks = ["*", ...AI_CRAWLERS].map((agent) => `User-agent: ${agent}\nAllow: /`);
	return `${blocks.join("\n\n")}\n\nSitemap: ${siteUrl}/sitemap.xml\n`;
}

export function buildSitemap(siteUrl: string, lastmod: string): string {
	return [
		`<?xml version="1.0" encoding="UTF-8"?>`,
		`<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">`,
		`\t<url>`,
		`\t\t<loc>${siteUrl}/</loc>`,
		`\t\t<lastmod>${lastmod}</lastmod>`,
		`\t</url>`,
		`</urlset>`,
		"",
	].join("\n");
}
