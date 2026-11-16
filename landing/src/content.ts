import type { KeySymbol } from "./components/symbols";

const sectionLinks = [
	{ label: "Features", href: "#features" },
	{ label: "How it works", href: "#how" },
	{ label: "A closer look", href: "#product" },
] as const;

const earlyAccess = {
	label: "Request early access",
	href: "#early-access",
} as const;

export const content = {
	brand: "Margincite",
	nav: { links: sectionLinks, cta: earlyAccess },
	hero: {
		titleLead: "Notes in the",
		titleAccent: "margin",
		titleTail: "for your website",
		body: "Margincite reads every page of your site and hands back a ranked list of fixes: what to change so Google, ChatGPT, Claude and Perplexity can find, read and cite you.",
		primary: earlyAccess,
		secondary: { label: "See how it works", href: "#how" },
		arrowNote: "start here",
		sketchCaption: "example.com: 17 notes",
		sketchNotes: [
			"AI crawlers blocked in robots.txt",
			"38 pages have no description",
			"12 pages rank 5 to 20 on Google",
		],
		badge: "Example report",
		status: "Early access. Works inside Claude.",
		mapLabel: "Illustrated map of an example site. A purple course links its first three fixes in order.",
		mapNote: "An illustration, not a screenshot.",
		places: ["/guides", "/blog", "/app", "/docs"],
		key: [
			{ symbol: "contour", label: "Depth from home" },
			{ symbol: "open", label: "Search traffic" },
			{ symbol: "thicket", label: "Needs JavaScript" },
			{ symbol: "marsh", label: "Slow pages" },
			{ symbol: "course", label: "Fix order" },
		] satisfies readonly { symbol: KeySymbol; label: string }[],
	},
	features: {
		heading: "What it does today",
		intro: "Four things, all in the early-access build.",
		items: [
			{
				title: "Reads the whole site",
				body: "Follows your sitemap and links page by page, including pages that only show their content after JavaScript runs.",
			},
			{
				title: "Ranks what to fix",
				body: "Every note shows the pages affected, the evidence and the fix. The biggest wins sit at the top.",
			},
			{
				title: "Uses your real search data",
				body: "Connect Google Search Console and the ranking follows your actual queries, clicks and impressions.",
			},
			{
				title: "Works inside Claude",
				body: "Add Margincite to Claude as a connector, then ask it to audit your site or walk you through the list.",
			},
		],
	},
	how: {
		heading: "How it works",
		steps: [
			{
				title: "Add your site",
				body: "Paste the address. Margincite crawls it politely and follows your robots.txt.",
			},
			{
				title: "Read the notes",
				body: "Get a ranked list covering technical SEO, AI readiness and content.",
			},
			{
				title: "Fix and re-check",
				body: "Mark notes done. Margincite re-crawls every week and confirms each fix.",
			},
		],
	},
	product: {
		heading: "One note, up close",
		intro: "An example of what a single suggestion looks like.",
		tag: "Example",
		note: {
			title: "AI crawlers are blocked in robots.txt",
			parts: [
				{
					label: "What we found",
					body: "robots.txt disallows GPTBot, ClaudeBot and PerplexityBot on every page.",
				},
				{
					label: "Why it matters",
					body: "These crawlers cannot read the site, so AI answers have nothing of yours to cite.",
				},
				{
					label: "How to fix it",
					body: "Remove the three Disallow rules, or allow the sections you want read.",
				},
			],
		},
		honesty:
			"Margincite measures and reports. It does not promise rankings or citations.",
		coming: {
			heading: "Coming later",
			items: [
				"Drafting the fix for you, published only after you approve it",
				"Tracking which AI answer engines cite your site",
			],
		},
	},
	cta: {
		heading: "Want the notes for your site?",
		body: "Margincite is in early access. Leave your email and we will write when there is a seat for you.",
		emailLabel: "Email address",
		placeholder: "you@yourcompany.com",
		button: "Request early access",
		sending: "Sending…",
		success: "Thanks. You are on the list.",
		errors: {
			invalid_email: "That email address does not look right.",
			not_configured: "Sign-up is not connected yet. Please try again later.",
			network: "We could not reach the sign-up service. Please try again.",
			rejected: "The sign-up service did not accept that. Please try again.",
		},
		footnote: "We only use your email to contact you about early access.",
	},
	footer: {
		blurb: "SEO and AI search suggestions for your website.",
		links: sectionLinks,
	},
	seo: {
		title: "Margincite: SEO and AI search suggestions for your website",
		description:
			"Margincite crawls your whole site and returns a ranked list of fixes for Google and for AI answers like ChatGPT, Claude and Perplexity. Now in early access.",
	},
} as const;

export type ErrorReason = keyof typeof content.cta.errors;
