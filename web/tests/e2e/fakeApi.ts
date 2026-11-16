import type { Page, Route } from "@playwright/test";
import { envelope, failure } from "./envelope";
import { makeSettingsRoutes } from "./fakeSettings";
import { makeSiteRoutes } from "./fakeSite";

export const TOKEN = "dev-token";

type Item = { target: string; status: "open" | "done" | "dismissed" };

/** What the test can change while the app is open. */
export type FakeApi = {
	/** Aborts every request, as if the backend were off. */
	down: boolean;
	/** Answers every request with 401, as if the token had expired. */
	expired: boolean;
	/** Whether the site has been added. */
	siteAdded: boolean;
};

const SITE = { id: "s1", root_url: "https://example.com/", host: "example.com", path_prefix: "/", display_name: "example.com", page_cap: 200 };
const LONG = `https://example.com/${"a-very-long-address-segment-".repeat(6)}end`;

function crawl(status: "running" | "completed", pages: number) {
	return {
		id: "c1",
		site_id: "s1",
		status,
		trigger: "first",
		stop_reason: status === "completed" ? "finished" : null,
		robots_txt_found: false,
		sitemap_found: true,
		llms_txt_found: false,
		pages_crawled: pages,
		pages_failed: 0,
		error: null,
		score: status === "completed" ? 72 : null,
		html_pages: pages,
	};
}

const SUGGESTION = {
	id: "g1",
	check_id: "title_missing",
	category: "technical",
	severity: 3,
	effort: 1,
	title: "Pages are missing a title",
	open: 2,
	done: 0,
	dismissed: 0,
	priority: 9,
};

const PAGE = {
	id: "p1",
	crawl_id: "c1",
	url: LONG,
	depth: 1,
	in_sitemap: true,
	status_code: 200,
	content_type: "text/html",
	fetch_error: null,
	needed_javascript: false,
};

/**
 * Answers the API with Playwright routes, because the real backend correctly refuses to crawl a
 * local test site. The crawl reports 3, then 9, then 24 pages and ends; items remember their status.
 */
export async function fakeApi(page: Page): Promise<FakeApi> {
	const state: FakeApi = { down: false, expired: false, siteAdded: false };
	const siteRoutes = makeSiteRoutes();
	const settingsRoutes = makeSettingsRoutes();
	let polls = 0;
	let items: Item[] = [
		{ target: LONG, status: "open" },
		{ target: "https://example.com/contact", status: "open" },
	];
	const counts = () => ({
		open: items.filter((item) => item.status === "open").length,
		done: items.filter((item) => item.status === "done").length,
		dismissed: items.filter((item) => item.status === "dismissed").length,
	});
	const ended = () => polls >= 3;

	function summary() {
		const c = ended() ? crawl("completed", 24) : crawl("running", 3);
		return {
			site: SITE,
			latest_crawl: c,
			score: ended() ? 72 : null,
			counts: counts(),
			by_category: [
				{ category: "technical", open: counts().open },
				{ category: "ai_readiness", open: 0 },
				{ category: "content", open: 0 },
			],
			top: ended() ? [{ ...SUGGESTION, ...counts() }] : [],
			info: { pages_crawled: 24, html_pages: 24, sitemap_found: true, llms_txt_found: false, reach_basis: "pages" },
		};
	}

	async function answer(route: Route) {
		const request = route.request();
		const url = new URL(request.url());
		const path = url.pathname.replace("/api/v1", "");
		const method = request.method();
		if (state.down) return route.abort();
		if (state.expired) return route.fulfill(failure(401, "Token not recognised"));
		if (request.headers().authorization !== `Bearer ${TOKEN}`) return route.fulfill(failure(401, "Token not recognised"));

		if (path === "/me") return route.fulfill(envelope({ user_id: "u1", workspace_id: "w1", email: "you@example.com" }));
		if (path === "/sites" && method === "GET") return route.fulfill(envelope(state.siteAdded ? [SITE] : []));
		if (path === "/sites" && method === "POST") {
			state.siteAdded = true;
			return route.fulfill(envelope({ ...SITE, first_crawl_id: "c1" }, null, 201));
		}
		if (path === "/sites/s1") return route.fulfill(envelope(SITE));
		if (path === "/sites/s1/summary") return route.fulfill(envelope(summary()));
		if (path === "/crawls/c1") {
			polls += 1;
			const pages = [3, 9, 24][Math.min(polls, 3) - 1];
			return route.fulfill(envelope(crawl(polls >= 3 ? "completed" : "running", pages)));
		}
		if (path === "/crawls/c1/pages") return route.fulfill(envelope([PAGE], { total: 1, page: 1, limit: 20 }));
		if (path === "/sites/s1/suggestions") {
			return route.fulfill(envelope([{ ...SUGGESTION, ...counts() }], { total: 1, page: 1, limit: 20 }));
		}
		if (path === "/suggestions/g1" && method === "PATCH") {
			const body = request.postDataJSON() as { status: Item["status"]; targets?: string[] };
			items = items.map((item) => (!body.targets || body.targets.includes(item.target) ? { ...item, status: body.status } : item));
			return route.fulfill(envelope({ changed: 1 }));
		}
		if (path === "/suggestions/g1") {
			const detail = {
				suggestion: { ...SUGGESTION, ...counts() },
				why: "A page title is the first line a reader and a crawler see.",
				fix: "Give every page a short title that says what the page is about.",
				items: items.map((item) => ({
					target: item.target,
					target_kind: "page",
					status: item.status,
					evidence: { title_length: 0, found_in: ["head", "body"] },
					first_seen: "2026-11-01T10:00:00Z",
					last_seen: "2026-11-13T10:00:00Z",
					resolved_at: null,
				})),
			};
			return route.fulfill(envelope(detail, { total: items.length, page: 1, limit: 100 }));
		}
		if (path === "/pages/p1") {
			const facts = {
				title: "A page with a very long title that must wrap and never push the screen sideways at any width",
				meta_description: "Who we are.",
				h1s: [],
				headings: [{ level: 1, text: "About us" }],
				canonical: LONG,
				robots_meta: null,
				has_viewport: true,
				word_count: 320,
				structured_data: [],
				structured_data_errors: null,
				links: [],
				images: [],
				has_scripts: false,
				published: null,
				modified: null,
				author: null,
			};
			return route.fulfill(envelope({ ...PAGE, response_ms: 180, bytes: 1, is_https: true, x_robots_tag: null, text: `${"word ".repeat(300)}\n${LONG}`, facts }));
		}
		if (path === "/pages/p1/findings") {
			return route.fulfill(envelope([{ check_id: "title_missing", category: "technical", severity: 3, title: "This page has no title", evidence: { title_length: 0 } }]));
		}
		const body = request.postData() ? (request.postDataJSON() as Record<string, unknown>) : {};
		const more = settingsRoutes(path, method, body) ?? siteRoutes(path, method, body);
		if (more) return route.fulfill(more);
		return route.fulfill(failure(404, "Not found."));
	}

	await page.route("**/api/v1/**", answer);
	return state;
}

/** Signs in the way a person would, by pasting the token. */
export async function signIn(page: Page, token = TOKEN) {
	await page.goto("/");
	await page.getByLabel("Access token").fill(token);
	await page.getByRole("button", { name: "Sign in" }).click();
}

/** Starts already signed in, for tests that are not about signing in. */
export async function startSignedIn(page: Page) {
	await page.addInitScript((token) => sessionStorage.setItem("margincite.token", token), TOKEN);
}
