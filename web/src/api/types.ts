/** Response shapes, copied from backend/API.md. Times are RFC 3339 strings. */

export type Meta = { total: number; page: number; limit: number };

export type Me = { user_id: string; workspace_id: string; email: string };

export type Site = {
	id: string;
	root_url: string;
	host: string;
	path_prefix: string;
	display_name: string;
	page_cap: number;
};
export type CreatedSite = Site & { first_crawl_id: string };

export type CrawlStatus = "queued" | "running" | "completed" | "failed";
export type StopReason = "finished" | "page_cap" | "robots_blocked" | "site_unreachable" | "time_limit";

export type Crawl = {
	id: string;
	site_id: string;
	status: CrawlStatus;
	trigger: "manual" | "scheduled" | "first";
	stop_reason: StopReason | null;
	robots_txt_found: boolean;
	sitemap_found: boolean;
	llms_txt_found: boolean;
	pages_crawled: number;
	pages_failed: number;
	error: string | null;
	score: number | null;
	html_pages: number;
};

export type PageSummary = {
	id: string;
	crawl_id: string;
	url: string;
	depth: number;
	in_sitemap: boolean;
	status_code: number | null;
	content_type: string | null;
	fetch_error: string | null;
	needed_javascript: boolean;
};

export type PageFacts = {
	title: string | null;
	meta_description: string | null;
	h1s: string[];
	headings: { level: number; text: string }[];
	canonical: string | null;
	robots_meta: string | null;
	has_viewport: boolean;
	word_count: number;
	structured_data: unknown[];
	structured_data_errors: unknown;
	links: { url: string; text: string; nofollow: boolean }[];
	images: { src: string; alt: string | null }[];
	has_scripts: boolean;
	published: string | null;
	modified: string | null;
	author: string | null;
};

export type PageDetail = PageSummary & {
	response_ms: number | null;
	bytes: number | null;
	is_https: boolean;
	x_robots_tag: string | null;
	text: string | null;
	facts: PageFacts | null;
};

export type Finding = {
	check_id: string;
	category: Category;
	severity: number;
	title: string;
	evidence: unknown;
};

export type Category = "technical" | "ai_readiness" | "content";
export type ItemStatus = "open" | "done" | "dismissed";

export type SuggestionOut = {
	id: string;
	check_id: string;
	category: Category;
	severity: number;
	effort: number;
	title: string;
	open: number;
	done: number;
	dismissed: number;
	priority: number;
};

export type SiteSummary = {
	site: Site;
	latest_crawl: Crawl | null;
	score: number | null;
	counts: { open: number; done: number; dismissed: number };
	by_category: { category: Category; open: number }[];
	top: SuggestionOut[];
	info: {
		pages_crawled: number;
		html_pages: number;
		sitemap_found: boolean;
		llms_txt_found: boolean;
		reach_basis: "pages";
	};
};

export type SuggestionItem = {
	target: string;
	target_kind: "page" | "site";
	status: ItemStatus;
	evidence: Record<string, unknown>;
	first_seen: string;
	last_seen: string;
	resolved_at: string | null;
};

export type SuggestionDetail = {
	suggestion: SuggestionOut;
	why: string;
	fix: string;
	items: SuggestionItem[];
};

/* ---- Metrics, integrations, keywords, checks, search, backlinks, AI visibility ---- */

export type EngineId = "claude" | "chatgpt" | "grok" | "perplexity" | "gemini";

export type Integrations = {
	secret_key_set: boolean;
	google: { configured: boolean };
	engines: { engine: EngineId; configured: boolean; model: string }[];
};

export type Metrics = {
	site_health: {
		score: number | null;
		previous: number | null;
		history: { at: string; score: number }[];
		reason: string | null;
	};
	ai_visibility: {
		percent: number | null;
		mentions: number | null;
		citations: number | null;
		answers: number;
		last_run_at: string | null;
		engines: { engine: EngineId; percent: number; mentions: number; answers: number }[];
		reason: string | null;
	};
	visibility: { percent: number | null; in_top_10: number | null; keywords: number; reason: string | null; source: string };
	organic: {
		clicks: number | null;
		impressions: number | null;
		keywords: number | null;
		avg_position: number | null;
		clicks_previous: number | null;
		keywords_previous: number | null;
		daily: { date: string; clicks: number; impressions: number }[];
		source: "search_console";
		reason: string | null;
	};
	backlinks: {
		total: number | null;
		linked_pages: number | null;
		top_pages: { url: string; count: number }[];
		source: "bing";
		synced_at: string | null;
		reason: string | null;
	};
	rankings: {
		tracked: number;
		in_top_3: number | null;
		in_top_10: number | null;
		average_position: number | null;
		checked_at: string | null;
		reason: string | null;
	};
	ai_overviews: {
		tracked: number;
		present: number | null;
		cited: number | null;
		checked_at: string | null;
		reason: string | null;
	};
	speed: { performance: number | null; checked_at: string | null; reason: string | null };
};

export type LandingSource = "set" | "search_console" | "best_match" | "home";

export type Keyword = {
	id: string;
	keyword: string;
	target_url: string | null;
	landing_url: string | null;
	landing_source: LandingSource | null;
	position: number | null;
	clicks: number | null;
	impressions: number | null;
	/** From DataForSEO. Null until volumes have been fetched. */
	search_volume?: number | null;
	competition?: string | null;
	cpc?: number | null;
};

export type CheckStatus = "passed" | "failed" | "not_run";

export type CheckRow = {
	check_id: string;
	title: string;
	category: Category;
	status: CheckStatus;
	open: number;
	reason: string | null;
};

export type ChecksList = { crawl_id: string | null; checks: CheckRow[] };

export type GoogleStatus = {
	status: "not_connected" | "choose_property" | "connected" | "needs_reconnecting";
	property: string | null;
	synced_at: string | null;
	error: string | null;
};
export type GoogleProperty = { property: string; permission: string };

export type BingStatus = {
	status: "not_connected" | "connected" | "needs_reconnecting";
	site_url: string | null;
	synced_at: string | null;
	error: string | null;
};

export type SearchQuery = {
	query: string;
	clicks: number;
	impressions: number;
	ctr: number;
	position: number;
	top_page: string | null;
};
export type SearchPageRow = { url: string; clicks: number; impressions: number; ctr: number; position: number };

export type BacklinkTarget = { target_url: string; count: number };
export type BacklinkSource = { source_url: string; anchor_text: string | null };

export type AiSettings = { brand_name: string; aliases: string[]; competitors: string[] };
export type AiPrompt = { id: string; text: string; created_at: string };

export type RunStatus = "queued" | "running" | "completed" | "failed";
export type AiRun = {
	id: string;
	status: RunStatus;
	trigger: "manual" | "scheduled";
	engines: EngineId[];
	answers_total: number;
	answers_done: number;
	error: string | null;
	started_at: string | null;
	finished_at: string | null;
	created_at: string;
};

export type AiAnswer = {
	id: string;
	prompt_id: string;
	prompt: string;
	engine: EngineId;
	model: string;
	mentioned: boolean;
	cited: boolean;
	citation_rank: number | null;
	cited_domains: string[];
	competitor_mentions: string[];
	error: string | null;
};
export type AiAnswerDetail = AiAnswer & {
	answer_text: string;
	citations: { url: string; title: string | null; domain: string }[];
	notes: string[];
};
export type AiDomain = { domain: string; citations: number; answers: number; is_you: boolean; is_competitor: boolean };

/* ---- Settings, usage, rankings, keyword ideas and page speed ---- */

export type ProviderId = "claude" | "chatgpt" | "grok" | "gemini" | "perplexity";
export type SettingSource = "workspace" | "environment" | "none";
export type TestStatus = "ok" | "error" | "unchecked";

export type ProviderOut = {
	provider: ProviderId;
	key_set: boolean;
	source: SettingSource;
	model: string;
	default_model: string;
	status: TestStatus;
	error: string | null;
	checked_at: string | null;
};

export type Settings = {
	providers: ProviderOut[];
	google: {
		client_id_set: boolean;
		client_secret_set: boolean;
		source: SettingSource;
		/** The exact address to register with Google. Missing from an older backend. */
		redirect_uri?: string;
	};
	dataforseo: {
		login_set: boolean;
		password_set: boolean;
		source: SettingSource;
		status: TestStatus;
		error: string | null;
		balance: number | null;
		checked_at: string | null;
	};
	pagespeed: { key_set: boolean; source: SettingSource };
	crawler: { contact: string | null; contact_source: SettingSource };
	secret_key_ready: boolean;
};

export type Choice = { code: string; name: string };
export type Device = "desktop" | "mobile";
export type RankingFrequency = "daily" | "weekly";

export type SiteSettings = {
	display_name: string;
	page_cap: number;
	country: string;
	language: string;
	device: Device;
	track_rankings: boolean;
	ranking_frequency: RankingFrequency;
	countries: Choice[];
	languages: Choice[];
};

export type SiteSettingsInput = Omit<SiteSettings, "countries" | "languages">;

export type UsageRow = {
	source: string;
	kind: string;
	calls: number;
	estimated_cost_usd: number | null;
	day: string;
};

export type AiOverviewStatus = "cited" | "present" | "absent" | "unknown";

export type RankingResult = { position: number; domain: string; url: string; title: string };

export type RankingKeyword = {
	keyword_id: string;
	keyword: string;
	position: number | null;
	previous: number | null;
	url: string | null;
	ai_overview: AiOverviewStatus;
	top: RankingResult[];
	history: { at: string; position: number | null }[];
};

export type Rankings = {
	location: { country: string; language: string; device: Device };
	/** The last run that stored results. */
	checked_at: string | null;
	/** The newest rank run of any outcome, including one that stored nothing. Missing from an older backend. */
	last_run?: RunState | null;
	keywords: RankingKeyword[];
};

export type RunState = {
	id: string;
	status: "queued" | "running" | "completed" | "failed";
	error: string | null;
	finished_at: string | null;
};

export type AiOverviewRow = {
	keyword_id: string;
	keyword: string;
	status: AiOverviewStatus;
	cited_domains: string[];
	your_urls: string[];
	checked_at: string | null;
};

export type KeywordIdea = { keyword: string; search_volume: number | null; competition: string | null; cpc: number | null };

export type SpeedPage = {
	url: string;
	strategy: "mobile";
	performance: number | null;
	lcp_ms: number | null;
	cls: number | null;
	inp_ms: number | null;
	tbt_ms: number | null;
	field_data: boolean;
	error: string | null;
};

export type SpeedReport = { checked_at: string | null; pages: SpeedPage[] };

export type VolumeStatus = { status: "idle" | "queued" | "running" | "failed"; checked_at: string | null; error: string | null };
