import { envelope, failure } from "./envelope";

type Reply = { status: number; contentType: string; body: string };

type Keyword = { id: string; keyword: string; target_url: string | null };
type Prompt = { id: string; text: string; created_at: string };

const LONG = `https://example.com/${"a-very-long-address-segment-".repeat(6)}end`;
const ENGINES = [
	{ engine: "claude", configured: true, model: "claude-sonnet-5-5" },
	{ engine: "chatgpt", configured: false, model: "gpt-5" },
	{ engine: "perplexity", configured: false, model: "sonar" },
	{ engine: "gemini", configured: false, model: "gemini-3-pro" },
];

const METRICS = {
	site_health: { score: 72, previous: 70, history: [], reason: null },
	ai_visibility: { percent: null, mentions: null, citations: null, answers: 0, last_run_at: null, engines: [], reason: "Run an AI visibility check to see this." },
	visibility: { percent: null, in_top_10: null, keywords: 0, reason: "Add target keywords to see this." },
	organic: {
		clicks: 1200,
		impressions: 30000,
		keywords: 80,
		avg_position: 14.2,
		clicks_previous: 1000,
		keywords_previous: 85,
		daily: Array.from({ length: 90 }, (_, i) => ({ date: `2026-08-${String((i % 28) + 1).padStart(2, "0")}`, clicks: 10 + ((i * 7) % 30), impressions: 300 + ((i * 13) % 200) })),
		source: "search_console",
		reason: null,
	},
	backlinks: { total: 530, linked_pages: 12, top_pages: [], source: "bing", synced_at: "2026-11-12T10:00:00Z", reason: null },
	rankings: { tracked: 0, in_top_3: null, in_top_10: null, average_position: null, checked_at: null, reason: "Add search data in Settings to track rankings." },
	ai_overviews: { tracked: 0, present: null, cited: null, checked_at: null, reason: "Add search data in Settings to see AI Overviews." },
	speed: { performance: 64, checked_at: "2026-11-14T09:00:00Z", reason: null },
};

/**
 * The fake answers for everything the metrics, keywords, checks, search, backlinks, connections and
 * AI screens ask. Returns null for a path it does not know. State lives for one test.
 */
export function makeSiteRoutes() {
	let keywords: Keyword[] = [];
	let prompts: Prompt[] = [];
	let runStarted = false;
	let runPolls = 0;

	const runBody = () => {
		const status = runPolls === 0 ? "queued" : runPolls === 1 ? "running" : "completed";
		return {
			id: "r1",
			status,
			trigger: "manual",
			engines: ["claude"],
			answers_total: prompts.length,
			answers_done: status === "completed" ? prompts.length : status === "running" ? 1 : 0,
			error: null,
			started_at: "2026-11-14T10:00:00Z",
			finished_at: status === "completed" ? "2026-11-14T10:01:00Z" : null,
			created_at: "2026-11-14T10:00:00Z",
		};
	};
	const answers = () =>
		prompts.map((p, i) => ({
			id: `a${i + 1}`,
			prompt_id: p.id,
			prompt: p.text,
			engine: "claude",
			model: "claude-sonnet-5-5",
			mentioned: i === 0,
			cited: false,
			citation_rank: null,
			cited_domains: [],
			competitor_mentions: [],
			error: null,
		}));

	return (path: string, method: string, body: Record<string, unknown>): Reply | null => {
		const get = method === "GET";
		if (path === "/integrations") return envelope({ secret_key_set: true, google: { configured: true }, engines: ENGINES });
		if (path === "/sites/s1/metrics") {
			const done = runStarted && runPolls >= 2;
			return envelope({
				...METRICS,
				ai_visibility: done ? { percent: 100, mentions: 1, citations: 0, answers: prompts.length, last_run_at: "2026-11-14T10:01:00Z", engines: [], reason: null } : METRICS.ai_visibility,
				visibility: { ...METRICS.visibility, keywords: keywords.length },
			});
		}
		if (path === "/sites/s1/rankings" && get)
			return envelope({
				location: { country: "US", language: "en", device: "desktop" },
				checked_at: "2026-11-14T08:00:00Z",
				keywords: keywords.map((k, i) => ({
					keyword_id: k.id,
					keyword: k.keyword,
					position: 4 + i,
					previous: 6 + i,
					url: LONG,
					ai_overview: "absent",
					top: [{ position: 1, domain: "maps.example.org", url: "https://maps.example.org/", title: "The best road maps" }],
					history: [
						{ at: "2026-11-01T08:00:00Z", position: 9 },
						{ at: "2026-11-14T08:00:00Z", position: 4 + i },
					],
				})),
			});
		if (path === "/sites/s1/ai-overviews") return envelope([]);
		if (path === "/sites/s1/speed" && get) return envelope({ checked_at: null, pages: [] });
		if (path === "/sites/s1/checks") {
			return envelope({
				crawl_id: "c1",
				checks: [
					{ check_id: "title_missing", title: "Every page has a title", category: "technical", status: "failed", open: 2, reason: null },
					{ check_id: "ai_not_mentioned", title: "AI assistants mention the site", category: "ai_readiness", status: "not_run", open: 0, reason: "Run an AI visibility check first." },
				],
			});
		}
		if (path === "/sites/s1/keywords" && get)
			return envelope(keywords.map((k) => ({ ...k, landing_url: LONG, landing_source: "home", position: null, clicks: null, impressions: null })));
		if (path === "/sites/s1/keywords") {
			const added = { id: `k${keywords.length + 1}`, keyword: String(body.keyword), target_url: (body.target_url as string | undefined) ?? null };
			keywords = [...keywords, added];
			return envelope({ ...added, landing_url: LONG, landing_source: "home", position: null, clicks: null, impressions: null }, null, 201);
		}
		if (path === "/sites/s1/integrations/google") return envelope({ status: "not_connected", property: null, synced_at: null, error: null });
		if (path === "/sites/s1/integrations/bing") return envelope({ status: "not_connected", site_url: null, synced_at: null, error: null });
		if (path === "/sites/s1/search/queries")
			return envelope([{ query: "road maps", clicks: 90, impressions: 2000, ctr: 0.045, position: 3.2, top_page: LONG }], { total: 1, page: 1, limit: 20 });
		if (path === "/sites/s1/search/pages")
			return envelope([{ url: LONG, clicks: 95, impressions: 2400, ctr: 0.04, position: 5.5 }], { total: 1, page: 1, limit: 20 });
		if (path === "/sites/s1/backlinks") return envelope([{ target_url: LONG, count: 130 }], { total: 1, page: 1, limit: 20 });
		if (path === "/sites/s1/ai/settings") return envelope({ brand_name: "Example", aliases: [], competitors: [] });
		if (path === "/sites/s1/ai/prompts" && get) return envelope(prompts);
		if (path === "/sites/s1/ai/prompts") {
			const added = { id: `q${prompts.length + 1}`, text: String(body.text), created_at: "2026-11-14T10:00:00Z" };
			prompts = [...prompts, added];
			return envelope(added, null, 201);
		}
		if (path === "/sites/s1/ai/prompts/suggest")
			return envelope({ suggestions: ["Which company makes the best printed city maps?", "How do I pick a hiking map for Scotland?"] });
		if (path === "/sites/s1/ai/runs" && get) return envelope(runStarted ? [runBody()] : [], { total: runStarted ? 1 : 0, page: 1, limit: 10 });
		if (path === "/sites/s1/ai/runs") {
			runStarted = true;
			runPolls = 0;
			return envelope(runBody(), null, 202);
		}
		if (path === "/ai/runs/r1") {
			const reply = envelope(runBody());
			runPolls += 1;
			return reply;
		}
		if (path === "/ai/runs/r1/answers") return envelope(answers());
		if (path === "/sites/s1/ai/domains") return envelope([{ domain: "example.com", citations: 1, answers: 1, is_you: true, is_competitor: false }]);
		const answer = /^\/ai\/answers\/a(\d+)$/.exec(path);
		if (answer) {
			const row = answers()[Number(answer[1]) - 1];
			if (!row) return failure(404, "Answer not found.");
			return envelope({
				...row,
				answer_text: `Example makes good maps.\n${"A long line of answer text. ".repeat(20)}`,
				citations: [{ url: LONG, title: null, domain: "example.com" }],
				notes: [],
			});
		}
		return null;
	};
}
