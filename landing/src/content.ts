/** Every word on the page. Components hold no copy of their own. Claims match the README. */

export const REPO_URL = "https://github.com/Murali03M/margincite";

const sectionLinks = [
	{ label: "Features", href: "#features" },
	{ label: "How it works", href: "#how" },
	{ label: "A closer look", href: "#product" },
	{ label: "FAQ", href: "#faq" },
] as const;

const github = { label: "Get it on GitHub", href: REPO_URL } as const;

export type Serves = "SEO" | "AIO" | "GEO";
export type Cost = "free" | "key" | "paid";

export const content = {
	brand: "Margincite",
	nav: { links: sectionLinks, cta: github },
	visits: {
		label: "Visits",
		loading: "Counting visits",
		spoken: (count: string) => `${count} visits to this page`,
	},
	hero: {
		titleLead: "Every fix comes with",
		titleAccent: "its evidence",
		body: "Margincite reads every page of your website and hands back a ranked list of fixes for Google search, Google's AI Overviews, and AI assistants like ChatGPT, Claude, Gemini, Grok and Perplexity. Each fix shows what was found, where, and how to repair it.",
		primary: github,
		secondary: { label: "See how it works", href: "#how" },
		status: "Free and open source. Runs on your own machine with your own keys.",
		strip: {
			label: "Key features",
			items: ["Whole-site crawl", "Ranked fixes with evidence", "SEO, AIO and GEO in one list", "Your keys, your machine"],
		},
		exhibit: {
			badge: "Example",
			note: "An illustration, not a screenshot.",
			file: "example.com/robots.txt",
			lines: [
				{ text: "User-agent: GPTBot" },
				{ text: "Disallow: /", marker: 1 },
				{ text: "User-agent: ClaudeBot" },
				{ text: "Disallow: /", marker: 2 },
				{ text: "User-agent: PerplexityBot" },
				{ text: "Disallow: /", marker: 3 },
				{ text: "" },
				{ text: "User-agent: *" },
				{ text: "Allow: /" },
			],
			tag: {
				title: "Exhibit 1 of 17",
				rows: [
					{ label: "Site", value: "example.com" },
					{ label: "Found", value: "3 AI crawlers blocked" },
					{ label: "Check", value: "AI-crawler access" },
					{ label: "Fix", value: "Remove 3 Disallow rules" },
				],
			},
			label: "Example finding, illustrated: example.com's robots.txt blocks three AI crawlers. Marker 1: GPTBot is disallowed. Marker 2: ClaudeBot is disallowed. Marker 3: PerplexityBot is disallowed. The fix is to remove the three Disallow rules.",
		},
	},
	features: {
		heading: "What it does today",
		intro: "Everything here works in the current release. Each line says what it needs. A feature without its connection is simply off, and the app says how to turn it on.",
		columns: { what: "What you get", serves: "Serves", needs: "Needs" },
		costs: { free: "Free", key: "Your AI key", paid: "Paid service" } satisfies Record<Cost, string>,
		items: [
			{
				title: "Crawl and site health",
				body: "A score, every page found, and checks for titles, headings, links, structured data, robots rules and AI-crawler access.",
				serves: ["SEO", "GEO"],
				needs: "Nothing",
				cost: "free",
			},
			{
				title: "Ranked suggestions",
				body: "A to-do list built from the failed checks, with the pages each one affects, why it matters and how to fix it.",
				serves: ["SEO", "AIO", "GEO"],
				needs: "Nothing",
				cost: "free",
			},
			{
				title: "AI visibility and mentions",
				body: "Which assistants mention or cite your site, and for which of your customers' questions.",
				serves: ["GEO"],
				needs: "A key for at least one AI provider",
				cost: "key",
			},
			{
				title: "Rank tracking and AI Overviews",
				body: "Positions for your keywords, and whether a Google AI Overview appears and cites you.",
				serves: ["SEO", "AIO"],
				needs: "A DataForSEO account",
				cost: "paid",
			},
			{
				title: "Organic traffic and positions",
				body: "Clicks, impressions and positions from your own search data.",
				serves: ["SEO"],
				needs: "Google Search Console",
				cost: "free",
			},
			{
				title: "Page speed",
				body: "Core Web Vitals and performance scores for your pages.",
				serves: ["SEO"],
				needs: "A PageSpeed Insights key",
				cost: "free",
			},
			{
				title: "Backlinks",
				body: "How many links point at the site and which pages they reach.",
				serves: ["SEO"],
				needs: "A Bing Webmaster Tools key",
				cost: "free",
			},
			{
				title: "Keywords",
				body: "Checks that each target keyword is used where it counts. Search volume and related ideas come from DataForSEO.",
				serves: ["SEO", "AIO"],
				needs: "Nothing, or DataForSEO for volume",
				cost: "free",
			},
		] satisfies readonly { title: string; body: string; serves: readonly Serves[]; needs: string; cost: Cost }[],
	},
	layers: {
		heading: "SEO, AIO and GEO, side by side",
		intro: "People now find a website three ways. Margincite checks all three and ranks the fixes in one list.",
		columns: [
			{ name: "SEO", full: "Search engine optimisation" },
			{ name: "AIO", full: "Google AI Overviews" },
			{ name: "GEO", full: "Generative engine optimisation" },
		],
		rows: [
			{
				label: "Where it happens",
				cells: [
					"The ranked results on Google and Bing.",
					"The AI Overview at the top of Google's results.",
					"Answers from ChatGPT, Claude, Gemini, Grok and Perplexity.",
				],
			},
			{
				label: "What counts",
				cells: [
					"Pages that crawl, load fast and say clearly what they are about.",
					"Being one of the sources the overview cites.",
					"AI crawlers can read the site, and assistants mention or cite it.",
				],
			},
			{
				label: "What Margincite checks",
				cells: [
					"Site health checks, page speed, positions from Search Console and backlinks from Bing.",
					"Whether an overview appears for your keywords, and whether it cites you.",
					"AI-crawler access in robots.txt, and which assistants mention or cite you for your questions.",
				],
			},
			{
				label: "What it needs",
				cells: [
					"Nothing for the checks. Free Google and Bing keys for the rest.",
					"A DataForSEO account.",
					"A key for at least one AI provider.",
				],
			},
		],
	},
	how: {
		heading: "How it works",
		steps: [
			{
				title: "Run it",
				body: "Clone the repository and start it with one command. It runs on your machine and keeps its data in PostgreSQL.",
				code: "bash scripts/dev.sh",
			},
			{
				title: "Connect your providers",
				body: "Add the keys you have under Settings: AI providers, Search Console, PageSpeed, Bing and DataForSEO. Keys are encrypted and never shown again.",
			},
			{
				title: "Add a site, read the fixes",
				body: "Margincite crawls politely, follows robots.txt, runs its checks and ranks what to fix first. Crawl again to confirm each fix.",
			},
		],
	},
	product: {
		heading: "One finding, up close",
		intro: "An example of a single suggestion, the way Margincite lays it out.",
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
				{
					label: "How it is confirmed",
					body: "The AI-crawler access check passes on the next crawl.",
				},
			],
		},
		sample: {
			caption: "example.com: 17 fixes, top 5",
			columns: { rank: "Rank", fix: "Fix", serves: "Serves", reach: "Affects" },
			rows: [
				{ title: "AI crawlers are blocked in robots.txt", serves: ["GEO"], reach: "Whole site" },
				{ title: "38 pages have no meta description", serves: ["SEO"], reach: "38 pages" },
				{ title: "No AI Overview cites you for 6 of 12 keywords", serves: ["AIO"], reach: "6 keywords" },
				{ title: "Organization structured data is missing", serves: ["SEO", "GEO"], reach: "Home page" },
				{ title: "Largest Contentful Paint is over 4 seconds", serves: ["SEO"], reach: "5 pages" },
			] satisfies readonly { title: string; serves: readonly Serves[]; reach: string }[],
		},
		honesty: "Margincite measures and reports. It does not promise rankings or citations.",
		coming: {
			heading: "Coming later",
			items: [
				"Using Margincite from Claude, ChatGPT and Grok as a connector",
				"Drafting fixes for you to approve before anything is published",
				"Alerts and scheduled reports",
			],
		},
	},
	faq: {
		heading: "Questions, answered",
		items: [
			{
				question: "What is Margincite?",
				answer: "A free, open-source app that reads every page of your website and returns a ranked list of fixes for Google search, Google's AI Overviews and AI assistants. Each fix shows what was found, where, why it matters and how to repair it.",
			},
			{
				question: "What do SEO, AIO and GEO mean?",
				answer: "SEO is ranking in the classic search results. AIO is Google's AI Overviews: whether one appears for your keywords and cites you. GEO, generative engine optimisation, is whether AI assistants such as ChatGPT, Claude, Gemini, Grok and Perplexity mention or cite your site.",
			},
			{
				question: "Does it cost anything?",
				answer: "Margincite is free. The services you connect bill you directly: AI providers charge per request, and DataForSEO charges per keyword with a minimum top-up. Search Console, Bing Webmaster Tools and PageSpeed Insights are free. Settings shows how many calls each service has made, and each site has daily limits on paid runs.",
			},
			{
				question: "Where does my data go?",
				answer: "It stays on your machine, in your own PostgreSQL database. Keys you enter are encrypted before they are stored and the app never shows them again. Requests go only to your site and to the services you connect.",
			},
			{
				question: "How is it different from Ahrefs or Semrush?",
				answer: "Those are hosted subscriptions with their own data. Margincite runs on your machine, reads your own site, uses your own Search Console data and keys, and puts classic search and AI answers in one ranked list with the evidence for each fix. It has no backlink index or keyword database of its own.",
			},
			{
				question: "Can I use it from Claude?",
				answer: "Not yet. A connector for Claude, ChatGPT and Grok is planned. Today you use Margincite through its web app.",
			},
			{
				question: "Can I host it for my team?",
				answer: "Not yet. Sign-in is a development token, so run it on your own machine for now and do not put it on the open internet.",
			},
		],
	},
	getIt: {
		heading: "Run it on your own site",
		body: "Margincite is free software under the AGPL-3.0 licence. You need Rust, PostgreSQL 14 or later, Node 20 or later, and pnpm.",
		commands: [
			`git clone ${REPO_URL}.git`,
			"cd margincite",
			"createdb margincite_dev",
			"cp backend/.env.example backend/.env",
			"pnpm --dir web install",
			"bash scripts/dev.sh",
		],
		commandsLabel: "Commands to install and start Margincite",
		after: "Then open http://localhost:5174 and sign in with dev-token.",
		primary: github,
		copy: { idle: "Copy commands", done: "Copied", failed: "Copy failed. Select the commands instead." },
		costs: "Margincite costs nothing. The services you connect bill you directly. Search Console, Bing Webmaster Tools and PageSpeed Insights are free.",
	},
	footer: {
		blurb: "SEO, AI Overview and AI-assistant checks for your website.",
		links: sectionLinks,
		licence: "Free software under AGPL-3.0.",
		source: { label: "Source on GitHub", href: REPO_URL },
		counter: { label: "Visit count by Abacus", href: "https://github.com/JasonLovesDoggo/abacus" },
	},
	seo: {
		title: "Margincite: SEO, AI Overview and AI-assistant checks for your website",
		description:
			"Free, open-source tool that crawls your site and returns a ranked list of fixes for Google search, Google AI Overviews and AI assistants like ChatGPT, Claude and Perplexity. Runs on your own machine.",
	},
} as const;
