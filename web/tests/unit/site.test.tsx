import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { crawl, summary, suggestion } from "../mocks/fixtures";
import { renderApp } from "../render";

function mockSite(data = summary()) {
	server.use(
		http.get("/api/v1/sites", () => ok([data.site])),
		http.get("/api/v1/sites/s1", () => ok(data.site)),
		http.get("/api/v1/sites/s1/summary", () => ok(data)),
	);
}

test("the_overview_shows_score_counts_and_top_suggestions", async () => {
	mockSite(
		summary({
			top: [
				suggestion(),
				suggestion({ id: "g2", title: "Pages have no meta description", severity: 1, effort: 2, open: 2 }),
			],
		}),
	);
	renderApp("/sites/s1");

	expect(await screen.findByRole("heading", { level: 1, name: "example.com" })).toBeInTheDocument();
	expect(await screen.findByText(/out of 100/i)).toBeInTheDocument();
	expect(screen.getByText(/out of 100/i)).toBeInTheDocument();
	expect(
		screen.getByText("100 minus the share of pages affected by each open check, weighted by how serious it is."),
	).toBeInTheDocument();

	const counts = screen.getByRole("list", { name: "Suggestion counts" });
	expect(within(counts).getByText("Open").nextSibling).toHaveTextContent("7");
	expect(within(counts).getByText("Done").nextSibling).toHaveTextContent("2");
	expect(within(counts).getByText("Dismissed").nextSibling).toHaveTextContent("1");

	const categories = screen.getByRole("table", { name: "Open suggestions by category" });
	expect(within(categories).getByText("Technical").closest("tr")).toHaveTextContent("4");
	expect(within(categories).getByText("AI readiness").closest("tr")).toHaveTextContent("2");
	expect(within(categories).getByText("Content").closest("tr")).toHaveTextContent("1");

	const top = screen.getByRole("list", { name: "Top suggestions" });
	expect(within(top).getByRole("link", { name: "Pages are missing a title" })).toHaveAttribute(
		"href",
		"/suggestions/g1?site=s1",
	);
	expect(within(top).getByRole("link", { name: "Pages have no meta description" })).toHaveAttribute(
		"href",
		"/suggestions/g2?site=s1",
	);

	const facts = screen.getByRole("list", { name: "Crawl facts" });
	expect(within(facts).getByText("Pages crawled").nextSibling).toHaveTextContent("42");
	expect(within(facts).getByText("Sitemap").nextSibling).toHaveTextContent("Found");
});

test("the sidebar sections lead to every section of the site", async () => {
	mockSite();
	renderApp("/sites/s1");
	const sidebar = await screen.findByRole("complementary", { name: "Sidebar" });
	const tabs = await within(sidebar).findByRole("navigation", { name: "Site sections" });
	const names = within(tabs).getAllByRole("link").map((link) => link.textContent);
	expect(names).toEqual([
		"Overview",
		"Suggestions",
		"Checks",
		"Keywords",
		"Rankings",
		"AI Visibility",
		"Search",
		"Backlinks",
		"Pages",
		"Connections",
		"Site settings",
	]);
	expect(within(tabs).getByRole("link", { name: "Overview" })).toHaveAttribute("href", "/sites/s1");
	expect(within(tabs).getByRole("link", { name: "Overview" })).toHaveAttribute("aria-current", "page");
	const paths: Record<string, string> = {
		Suggestions: "suggestions",
		Checks: "checks",
		Keywords: "keywords",
		Rankings: "rankings",
		"AI Visibility": "ai",
		Search: "search",
		Backlinks: "backlinks",
		Pages: "pages",
		Connections: "connections",
		"Site settings": "settings",
	};
	for (const [name, path] of Object.entries(paths)) {
		expect(within(tabs).getByRole("link", { name })).toHaveAttribute("href", `/sites/s1/${path}`);
	}
});

test("the site page has no tab row of its own: the sections live only in the sidebar", async () => {
	mockSite();
	renderApp("/sites/s1");
	await screen.findByRole("heading", { level: 1, name: "example.com" });
	const navs = screen.getAllByRole("navigation", { name: "Site sections" });
	expect(navs).toHaveLength(1);
	expect(navs[0].closest("aside")).not.toBeNull();
	expect(screen.getByRole("main").querySelector("nav")).toBeNull();
});

test("the overview opens with the project card", async () => {
	mockSite();
	renderApp("/sites/s1");
	const card = await screen.findByRole("list", { name: "Project card" });
	expect(within(card).getAllByRole("listitem")).toHaveLength(9);
});

test("a_site_still_crawling_shows_progress_and_no_score", async () => {
	mockSite(
		summary({
			latest_crawl: crawl({ id: "c2", status: "running", pages_crawled: 5, score: null, stop_reason: null }),
			score: null,
			counts: { open: 0, done: 0, dismissed: 0 },
			top: [],
			by_category: [
				{ category: "technical", open: 0 },
				{ category: "ai_readiness", open: 0 },
				{ category: "content", open: 0 },
			],
		}),
	);
	server.use(
		http.get("/api/v1/crawls/c2", () =>
			ok(crawl({ id: "c2", status: "running", pages_crawled: 5, score: null, stop_reason: null })),
		),
	);
	renderApp("/sites/s1");
	expect(await screen.findByText(/5 pages crawled so far/i)).toBeInTheDocument();
	expect(screen.getByText(/no score yet/i)).toBeInTheDocument();
	expect(screen.queryByText(/out of 100/i)).not.toBeInTheDocument();
	expect(screen.getByRole("button", { name: /crawl again/i })).toBeDisabled();
});

test("a_site_with_no_readable_pages_explains_why_there_is_no_score", async () => {
	mockSite(
		summary({
			latest_crawl: crawl({ stop_reason: "robots_blocked", pages_crawled: 0, html_pages: 0, score: null }),
			score: null,
			counts: { open: 0, done: 0, dismissed: 0 },
			top: [],
			info: { pages_crawled: 0, html_pages: 0, sitemap_found: false, llms_txt_found: false, reach_basis: "pages" },
		}),
	);
	renderApp("/sites/s1");
	expect(await screen.findByText("No score: no pages could be read.")).toBeInTheDocument();
	expect(screen.getByText(/robots\.txt blocks margincite/i)).toBeInTheDocument();
	expect(screen.queryByText(/out of 100/i)).not.toBeInTheDocument();
});

test("a site with nothing open says there is nothing to fix", async () => {
	mockSite(summary({ counts: { open: 0, done: 3, dismissed: 0 }, top: [] }));
	renderApp("/sites/s1");
	expect(await screen.findByText(/nothing is open right now/i)).toBeInTheDocument();
});

test("a site that was never crawled invites a crawl", async () => {
	mockSite(summary({ latest_crawl: null, score: null, top: [] }));
	renderApp("/sites/s1");
	expect(await screen.findByText(/no score yet/i)).toBeInTheDocument();
	expect(screen.getByRole("button", { name: /crawl again/i })).toBeEnabled();
});

test("crawl_again_starts_a_crawl_and_shows_progress", async () => {
	mockSite();
	server.use(
		http.post("/api/v1/sites/s1/crawls", () =>
			ok(crawl({ id: "c2", status: "queued", trigger: "manual", pages_crawled: 0, score: null, stop_reason: null }), null, 202),
		),
		http.get("/api/v1/crawls/c2", () =>
			ok(crawl({ id: "c2", status: "running", pages_crawled: 3, score: null, stop_reason: null })),
		),
	);
	renderApp("/sites/s1");
	await userEvent.click(await screen.findByRole("button", { name: /crawl again/i }));
	expect(await screen.findByText(/3 pages crawled so far/i)).toBeInTheDocument();
	expect(screen.getByRole("button", { name: /crawl again|starting/i })).toBeDisabled();
});

test("crawl_again_shows_the_daily_limit_message", async () => {
	mockSite();
	server.use(
		http.post("/api/v1/sites/s1/crawls", () => fail(429, "You have used all 10 crawls for today. Try again tomorrow.")),
	);
	renderApp("/sites/s1");
	await userEvent.click(await screen.findByRole("button", { name: /crawl again/i }));
	expect(await screen.findByText("You have used all 10 crawls for today. Try again tomorrow.")).toBeInTheDocument();
	expect(screen.getByRole("button", { name: /crawl again/i })).toBeEnabled();
});

test("crawl again shows the already-running message as the server wrote it", async () => {
	mockSite();
	server.use(http.post("/api/v1/sites/s1/crawls", () => fail(409, "A crawl is already running for this site.")));
	renderApp("/sites/s1");
	await userEvent.click(await screen.findByRole("button", { name: /crawl again/i }));
	expect(await screen.findByText("A crawl is already running for this site.")).toBeInTheDocument();
});

test("llms_txt_is_shown_as_information_not_as_a_problem", async () => {
	mockSite(summary({ info: { pages_crawled: 42, html_pages: 40, sitemap_found: true, llms_txt_found: false, reach_basis: "pages" } }));
	renderApp("/sites/s1");
	const facts = await screen.findByRole("list", { name: "Crawl facts" });
	const row = within(facts).getByText("llms.txt").closest("div") as HTMLElement;
	expect(row).toHaveTextContent(/not found/i);
	expect(row).toHaveTextContent(/for information/i);
	expect(screen.queryByRole("alert")).not.toBeInTheDocument();
	expect(within(screen.getByRole("list", { name: "Top suggestions" })).queryByText(/llms/i)).not.toBeInTheDocument();
});

test("an_unknown_site_shows_not_found", async () => {
	server.use(http.get("/api/v1/sites/nope", () => fail(404, "Site not found.")));
	renderApp("/sites/nope");
	expect(await screen.findByText(/that site was not found/i)).toBeInTheDocument();
	expect(screen.getByRole("link", { name: /back to your sites/i })).toHaveAttribute("href", "/");
});

test("an overview that fails to load shows the error and retries", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(summary().site)),
		http.get("/api/v1/sites/s1/summary", () => fail(500, "The summary could not be built.")),
	);
	renderApp("/sites/s1");
	expect(await screen.findByText("The summary could not be built.")).toBeInTheDocument();
	mockSite();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByText(/out of 100/i)).toBeInTheDocument();
});

test("a site page that fails to load shows the error", async () => {
	server.use(http.get("/api/v1/sites/s1", () => fail(500, "Sites are unavailable.")));
	renderApp("/sites/s1");
	expect(await screen.findByText("Sites are unavailable.")).toBeInTheDocument();
});

test("crawled text in a suggestion title is shown as characters", async () => {
	mockSite(summary({ top: [suggestion({ title: "<img src=x onerror=alert(1)> is missing" })] }));
	renderApp("/sites/s1");
	expect(await screen.findByText("<img src=x onerror=alert(1)> is missing")).toBeInTheDocument();
	expect(document.querySelector("img")).toBeNull();
});
