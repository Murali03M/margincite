/** Builders for the API's response shapes. Each takes overrides. */
import type {
	AiAnswer,
	AiAnswerDetail,
	AiRun,
	CheckRow,
	Crawl,
	Finding,
	AiOverviewRow,
	Integrations,
	Keyword,
	KeywordIdea,
	Metrics,
	PageDetail,
	PageFacts,
	PageSummary,
	ProviderId,
	ProviderOut,
	RankingKeyword,
	Rankings,
	Settings,
	Site,
	SiteSettings,
	SiteSummary,
	SpeedPage,
	SpeedReport,
	SuggestionDetail,
	SuggestionItem,
	SuggestionOut,
} from "../../src/api/types";

export function site(over: Partial<Site> = {}): Site {
	return {
		id: "s1",
		root_url: "https://example.com/",
		host: "example.com",
		path_prefix: "/",
		display_name: "example.com",
		page_cap: 200,
		...over,
	};
}

export function crawl(over: Partial<Crawl> = {}): Crawl {
	return {
		id: "c1",
		site_id: "s1",
		status: "completed",
		trigger: "first",
		stop_reason: "finished",
		robots_txt_found: false,
		sitemap_found: true,
		llms_txt_found: false,
		pages_crawled: 42,
		pages_failed: 0,
		error: null,
		score: 72,
		html_pages: 40,
		...over,
	};
}

export function suggestion(over: Partial<SuggestionOut> = {}): SuggestionOut {
	return {
		id: "g1",
		check_id: "title_missing",
		category: "technical",
		severity: 3,
		effort: 1,
		title: "Pages are missing a title",
		open: 4,
		done: 0,
		dismissed: 0,
		priority: 9,
		...over,
	};
}

export function summary(over: Partial<SiteSummary> = {}): SiteSummary {
	return {
		site: site(),
		latest_crawl: crawl(),
		score: 72,
		counts: { open: 7, done: 2, dismissed: 1 },
		by_category: [
			{ category: "technical", open: 4 },
			{ category: "ai_readiness", open: 2 },
			{ category: "content", open: 1 },
		],
		top: [suggestion()],
		info: { pages_crawled: 42, html_pages: 40, sitemap_found: true, llms_txt_found: false, reach_basis: "pages" },
		...over,
	};
}

export function item(over: Partial<SuggestionItem> = {}): SuggestionItem {
	return {
		target: "https://example.com/about",
		target_kind: "page",
		status: "open",
		evidence: { title_length: 0 },
		first_seen: "2026-11-01T10:00:00Z",
		last_seen: "2026-11-13T10:00:00Z",
		resolved_at: null,
		...over,
	};
}

export function suggestionDetail(over: Partial<SuggestionDetail> = {}): SuggestionDetail {
	return {
		suggestion: suggestion(),
		why: "A page title is the first line a reader and a crawler see.",
		fix: "Give every page a short title that says what the page is about.",
		items: [item(), item({ target: "https://example.com/contact" })],
		...over,
	};
}

export function pageSummary(over: Partial<PageSummary> = {}): PageSummary {
	return {
		id: "p1",
		crawl_id: "c1",
		url: "https://example.com/about",
		depth: 1,
		in_sitemap: true,
		status_code: 200,
		content_type: "text/html",
		fetch_error: null,
		needed_javascript: false,
		...over,
	};
}

export function pageFacts(over: Partial<PageFacts> = {}): PageFacts {
	return {
		title: "About us",
		meta_description: "Who we are.",
		h1s: ["About us"],
		headings: [
			{ level: 1, text: "About us" },
			{ level: 2, text: "Our story" },
		],
		canonical: "https://example.com/about",
		robots_meta: null,
		has_viewport: true,
		word_count: 320,
		structured_data: [{ "@type": "Organization" }],
		structured_data_errors: null,
		links: [],
		images: [],
		has_scripts: true,
		published: null,
		modified: null,
		author: null,
		...over,
	};
}

export function pageDetail(over: Partial<PageDetail> = {}): PageDetail {
	return {
		...pageSummary(),
		response_ms: 180,
		bytes: 12000,
		is_https: true,
		x_robots_tag: null,
		text: "We make maps.\nCome and see.",
		facts: pageFacts(),
		...over,
	};
}

export function finding(over: Partial<Finding> = {}): Finding {
	return {
		check_id: "title_missing",
		category: "technical",
		severity: 3,
		title: "This page has no title",
		evidence: { title_length: 0 },
		...over,
	};
}

export function metrics(over: Partial<Metrics> = {}): Metrics {
	return {
		site_health: { score: 72, previous: 68, history: [], reason: null },
		ai_visibility: {
			percent: 40,
			mentions: 6,
			citations: 4,
			answers: 15,
			last_run_at: "2026-11-13T10:00:00Z",
			engines: [{ engine: "claude", percent: 40, mentions: 6, answers: 15 }],
			reason: null,
		},
		visibility: { percent: 50, in_top_10: 2, keywords: 4, reason: null, source: "rankings" },
		organic: {
			clicks: 1200,
			impressions: 30000,
			keywords: 80,
			avg_position: 14.2,
			clicks_previous: 1000,
			keywords_previous: 85,
			daily: [
				{ date: "2026-11-10", clicks: 10, impressions: 300 },
				{ date: "2026-11-11", clicks: 30, impressions: 200 },
				{ date: "2026-11-12", clicks: 20, impressions: 400 },
			],
			source: "search_console",
			reason: null,
		},
		backlinks: { total: 530, linked_pages: 12, top_pages: [], source: "bing", synced_at: "2026-11-12T10:00:00Z", reason: null },
		rankings: { tracked: 4, in_top_3: 1, in_top_10: 3, average_position: 8.5, checked_at: "2026-11-14T08:00:00Z", reason: null },
		ai_overviews: { tracked: 4, present: 2, cited: 1, checked_at: "2026-11-14T08:00:00Z", reason: null },
		speed: { performance: 64, checked_at: "2026-11-14T09:00:00Z", reason: null },
		...over,
	};
}

/** Every block empty, each with the reason the server would give. */
export function emptyMetrics(): Metrics {
	return metrics({
		site_health: { score: null, previous: null, history: [], reason: "Crawl the site to get a health score." },
		ai_visibility: {
			percent: null,
			mentions: null,
			citations: null,
			answers: 0,
			last_run_at: null,
			engines: [],
			reason: "Run an AI visibility check to see this.",
		},
		visibility: { percent: null, in_top_10: null, keywords: 0, reason: "Add target keywords to see this.", source: "none" },
		organic: {
			clicks: null,
			impressions: null,
			keywords: null,
			avg_position: null,
			clicks_previous: null,
			keywords_previous: null,
			daily: [],
			source: "search_console",
			reason: "Connect Google Search Console to see organic traffic.",
		},
		backlinks: {
			total: null,
			linked_pages: null,
			top_pages: [],
			source: "bing",
			synced_at: null,
			reason: "Connect Bing Webmaster Tools to see backlinks.",
		},
		rankings: { tracked: 0, in_top_3: null, in_top_10: null, average_position: null, checked_at: null, reason: "Add search data in Settings to track rankings." },
		ai_overviews: { tracked: 0, present: null, cited: null, checked_at: null, reason: "Add search data in Settings to see AI Overviews." },
		speed: { performance: null, checked_at: null, reason: "Page speed has not been checked yet." },
	});
}

export function keyword(over: Partial<Keyword> = {}): Keyword {
	return {
		id: "k1",
		keyword: "road maps",
		target_url: null,
		landing_url: "https://example.com/maps",
		landing_source: "best_match",
		position: null,
		clicks: null,
		impressions: null,
		...over,
	};
}

export function integrations(over: Partial<Integrations> = {}): Integrations {
	return {
		secret_key_set: true,
		google: { configured: true },
		engines: [
			{ engine: "claude", configured: true, model: "claude-sonnet-5-5" },
			{ engine: "chatgpt", configured: false, model: "gpt-5" },
			{ engine: "grok", configured: false, model: "grok-4" },
			{ engine: "perplexity", configured: false, model: "sonar" },
			{ engine: "gemini", configured: true, model: "gemini-3-pro" },
		],
		...over,
	};
}

export function aiRun(over: Partial<AiRun> = {}): AiRun {
	return {
		id: "r1",
		status: "completed",
		trigger: "manual",
		engines: ["claude", "gemini"],
		answers_total: 4,
		answers_done: 4,
		error: null,
		started_at: "2026-11-13T10:00:00Z",
		finished_at: "2026-11-13T10:02:00Z",
		created_at: "2026-11-13T09:59:00Z",
		...over,
	};
}

export function aiAnswer(over: Partial<AiAnswer> = {}): AiAnswer {
	return {
		id: "a1",
		prompt_id: "q1",
		prompt: "What is the best map maker?",
		engine: "claude",
		model: "claude-sonnet-5-5",
		mentioned: true,
		cited: false,
		citation_rank: null,
		cited_domains: [],
		competitor_mentions: [],
		error: null,
		...over,
	};
}

export function aiAnswerDetail(over: Partial<AiAnswerDetail> = {}): AiAnswerDetail {
	return {
		...aiAnswer(),
		answer_text: "Example makes good maps.\nSee the site.",
		citations: [{ url: "https://example.com/maps", title: "Example maps", domain: "example.com" }],
		notes: [],
		...over,
	};
}

export function checkRow(over: Partial<CheckRow> = {}): CheckRow {
	return {
		check_id: "title_missing",
		title: "Every page has a title",
		category: "technical",
		status: "passed",
		open: 0,
		reason: null,
		...over,
	};
}

export function providerOut(provider: ProviderId = "claude", over: Partial<ProviderOut> = {}): ProviderOut {
	return {
		provider,
		key_set: false,
		source: "none",
		model: `${provider}-default`,
		default_model: `${provider}-default`,
		status: "unchecked",
		error: null,
		checked_at: null,
		...over,
	};
}

/** Settings with nothing set up. */
export function settings(over: Partial<Settings> = {}): Settings {
	return {
		providers: (["claude", "chatgpt", "grok", "gemini", "perplexity"] as const).map((id) => providerOut(id)),
		google: { client_id_set: false, client_secret_set: false, source: "none" },
		dataforseo: { login_set: false, password_set: false, source: "none", status: "unchecked", error: null, balance: null, checked_at: null },
		pagespeed: { key_set: false, source: "none" },
		crawler: { contact: null, contact_source: "none" },
		secret_key_ready: true,
		...over,
	};
}

/** Settings with every source set up and working. */
export function settingsReady(): Settings {
	return settings({
		providers: (["claude", "chatgpt", "grok", "gemini", "perplexity"] as const).map((id) =>
			providerOut(id, { key_set: true, source: "workspace", status: "ok", checked_at: "2026-11-14T10:00:00Z" }),
		),
		google: { client_id_set: true, client_secret_set: true, source: "workspace" },
		dataforseo: { login_set: true, password_set: true, source: "workspace", status: "ok", error: null, balance: 41.5, checked_at: "2026-11-14T10:00:00Z" },
		pagespeed: { key_set: true, source: "workspace" },
		crawler: { contact: "you@example.com", contact_source: "workspace" },
	});
}

export function siteSettings(over: Partial<SiteSettings> = {}): SiteSettings {
	return {
		display_name: "example.com",
		page_cap: 200,
		country: "US",
		language: "en",
		device: "desktop",
		track_rankings: false,
		ranking_frequency: "weekly",
		countries: [
			{ code: "US", name: "United States" },
			{ code: "GB", name: "United Kingdom" },
		],
		languages: [
			{ code: "en", name: "English" },
			{ code: "de", name: "German" },
		],
		...over,
	};
}

export function rankingKeyword(over: Partial<RankingKeyword> = {}): RankingKeyword {
	return {
		keyword_id: "k1",
		keyword: "road maps",
		position: 4,
		previous: 7,
		url: "https://example.com/maps",
		ai_overview: "absent",
		top: [
			{ position: 1, domain: "maps.example.org", url: "https://maps.example.org/", title: "The best road maps" },
			{ position: 4, domain: "example.com", url: "https://example.com/maps", title: "Road maps from Example" },
		],
		history: [
			{ at: "2026-11-01T08:00:00Z", position: 9 },
			{ at: "2026-11-08T08:00:00Z", position: 7 },
			{ at: "2026-11-14T08:00:00Z", position: 4 },
		],
		...over,
	};
}

export function rankings(over: Partial<Rankings> = {}): Rankings {
	return {
		location: { country: "US", language: "en", device: "desktop" },
		checked_at: "2026-11-14T08:00:00Z",
		keywords: [rankingKeyword()],
		...over,
	};
}

export function aiOverviewRow(over: Partial<AiOverviewRow> = {}): AiOverviewRow {
	return {
		keyword_id: "k1",
		keyword: "road maps",
		status: "cited",
		cited_domains: ["example.com", "maps.example.org"],
		your_urls: ["https://example.com/maps"],
		checked_at: "2026-11-14T08:00:00Z",
		...over,
	};
}

export function idea(over: Partial<KeywordIdea> = {}): KeywordIdea {
	return { keyword: "printed city maps", search_volume: 1900, competition: "LOW", cpc: 0.8, ...over };
}

export function speedPage(over: Partial<SpeedPage> = {}): SpeedPage {
	return {
		url: "https://example.com/",
		strategy: "mobile",
		performance: 64,
		lcp_ms: 3200,
		cls: 0.12,
		inp_ms: 240,
		tbt_ms: 410,
		field_data: true,
		error: null,
		...over,
	};
}

export function speedReport(over: Partial<SpeedReport> = {}): SpeedReport {
	return { checked_at: "2026-11-14T09:00:00Z", pages: [speedPage()], ...over };
}
