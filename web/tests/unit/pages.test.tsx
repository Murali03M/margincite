import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import {
	crawl,
	finding,
	pageDetail,
	pageFacts,
	pageSummary,
	site,
	suggestion,
	summary,
} from "../mocks/fixtures";
import { renderApp } from "../render";

const HOSTILE = "<img src=x onerror=alert(1)>";

function mockPagesList(rows = [pageSummary()], total = rows.length) {
	const pagesAsked: string[] = [];
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/crawls/c1/pages", ({ request }) => {
			const page = new URL(request.url).searchParams.get("page") ?? "1";
			pagesAsked.push(page);
			return ok(rows, { total, page: Number(page), limit: 20 });
		}),
	);
	return pagesAsked;
}

test("pages_are_listed_with_status_and_paging", async () => {
	const asked = mockPagesList(
		[
			pageSummary(),
			pageSummary({ id: "p2", url: "https://example.com/gone", status_code: 404, depth: 2, in_sitemap: false }),
			pageSummary({ id: "p3", url: "https://example.com/slow", status_code: null, fetch_error: "Timed out", needed_javascript: true }),
		],
		45,
	);
	renderApp("/sites/s1/pages");
	const table = await screen.findByRole("table", { name: "Pages from the latest crawl" });
	const rows = within(table).getAllByRole("row").slice(1);
	expect(rows).toHaveLength(3);
	expect(within(rows[0]).getByRole("link", { name: "https://example.com/about" })).toHaveAttribute("href", "/pages/p1");
	expect(rows[0]).toHaveTextContent("200");
	expect(rows[1]).toHaveTextContent("404");
	expect(rows[1]).toHaveTextContent("2");
	expect(rows[2]).toHaveTextContent("Timed out");

	expect(screen.getByText("Page 1 of 3")).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: "Next page" }));
	expect(await screen.findByText("Page 2 of 3")).toBeInTheDocument();
	expect(asked).toContain("2");
});

test("a site never crawled has no pages to list", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary({ latest_crawl: null, score: null }))),
	);
	renderApp("/sites/s1/pages");
	expect(await screen.findByRole("heading", { name: "No pages yet" })).toBeInTheDocument();
});

test("a crawl that read no pages says so", async () => {
	mockPagesList([], 0);
	renderApp("/sites/s1/pages");
	expect(await screen.findByRole("heading", { name: "No pages yet" })).toBeInTheDocument();
});

test("the pages list shows an error with retry", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/crawls/c1/pages", () => fail(500, "Pages are unavailable.")),
	);
	renderApp("/sites/s1/pages");
	expect(await screen.findByText("Pages are unavailable.")).toBeInTheDocument();
	mockPagesList();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("table", { name: "Pages from the latest crawl" })).toBeInTheDocument();
});

test("the pages screen reports a summary that cannot load", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => fail(500, "The summary could not be built.")),
	);
	renderApp("/sites/s1/pages");
	expect(await screen.findByText("The summary could not be built.")).toBeInTheDocument();
});

function mockPage(
	page = pageDetail(),
	findings = [finding()],
	suggestions = [suggestion({ id: "g1", check_id: "title_missing" })],
) {
	server.use(
		http.get("/api/v1/pages/p1", () => ok(page)),
		http.get("/api/v1/pages/p1/findings", () => ok(findings)),
		http.get("/api/v1/crawls/c1", () => ok(crawl())),
		http.get("/api/v1/sites/s1/suggestions", () => ok(suggestions, { total: suggestions.length, page: 1, limit: 100 })),
	);
}

test("a_page_shows_its_facts_and_its_findings", async () => {
	mockPage();
	renderApp("/pages/p1");
	expect(await screen.findByRole("heading", { level: 1, name: "About us" })).toBeInTheDocument();

	const facts = screen.getByRole("list", { name: "Page facts" });
	const fact = (label: string) => within(facts).getByText(label).closest("li") as HTMLElement;
	expect(fact("Description")).toHaveTextContent("Who we are.");
	expect(fact("Canonical")).toHaveTextContent("https://example.com/about");
	expect(fact("Word count")).toHaveTextContent("320");
	expect(fact("Status")).toHaveTextContent("200");
	expect(fact("Response time")).toHaveTextContent("180 ms");
	expect(fact("Needed JavaScript")).toHaveTextContent("No");
	expect(fact("Structured data")).toHaveTextContent("1 block");
	expect(fact("Structured data")).toHaveTextContent("Organization");
	expect(fact("Headings")).toHaveTextContent("H1 About us");
	expect(fact("Headings")).toHaveTextContent("H2 Our story");

	const findings = await screen.findByRole("list", { name: "Findings on this page" });
	expect(within(findings).getByText("This page has no title")).toBeInTheDocument();
	expect(within(findings).getByText("Title length: 0")).toBeInTheDocument();
	expect(await within(findings).findByRole("link", { name: /see the suggestion/i })).toHaveAttribute(
		"href",
		"/suggestions/g1?site=s1",
	);

	const text = screen.getByRole("region", { name: "Text read from this page" });
	expect(text).toHaveTextContent("We make maps.");
	expect(text).toHaveTextContent("Come and see.");

	expect(await screen.findByRole("link", { name: /back to pages/i })).toHaveAttribute("href", "/sites/s1/pages");
});

test("a page with no findings or details says so plainly", async () => {
	mockPage(pageDetail({ facts: null, text: null, status_code: 404, fetch_error: "Not found", response_ms: null }), [], []);
	renderApp("/pages/p1");
	expect(await screen.findByText(/no details were read from this page/i)).toBeInTheDocument();
	expect(await screen.findByText(/no findings against this page/i)).toBeInTheDocument();
	expect(screen.getByText(/no text was stored for this page/i)).toBeInTheDocument();
});

test("a finding without a matching suggestion is shown without a link", async () => {
	mockPage(pageDetail(), [finding({ check_id: "unknown_check" })], []);
	renderApp("/pages/p1");
	const findings = await screen.findByRole("list", { name: "Findings on this page" });
	expect(within(findings).queryByRole("link")).not.toBeInTheDocument();
});

test("crawled_markup_is_shown_as_characters", async () => {
	mockPage(
		pageDetail({
			facts: pageFacts({ title: HOSTILE, meta_description: `${HOSTILE} desc`, headings: [{ level: 2, text: HOSTILE }] }),
			text: `${HOSTILE}\nbody`,
		}),
		[finding({ title: `${HOSTILE} finding`, evidence: { note: HOSTILE } })],
	);
	renderApp("/pages/p1");
	expect(await screen.findByRole("heading", { level: 1, name: HOSTILE })).toBeInTheDocument();
	expect(screen.getByText(`${HOSTILE} desc`)).toBeInTheDocument();
	expect(document.querySelector("main img")).toBeNull();
	expect(screen.getByRole("region", { name: "Text read from this page" })).toHaveTextContent(HOSTILE);
	await screen.findByText(`${HOSTILE} finding`);
});

test("a_crawled_javascript_link_is_not_made_clickable", async () => {
	mockPage(
		pageDetail({
			url: "javascript:alert(1)",
			facts: pageFacts({ canonical: "javascript:alert(2)" }),
		}),
	);
	renderApp("/pages/p1");
	await screen.findByRole("list", { name: "Page facts" });
	expect(screen.getAllByText("javascript:alert(1)").length).toBeGreaterThan(0);
	expect(screen.getByText("javascript:alert(2)")).toBeInTheDocument();
	for (const link of screen.queryAllByRole("link")) {
		expect(link.getAttribute("href") ?? "").not.toMatch(/^javascript:/i);
	}
});

test("a page address that is safe links out with the safe rel", async () => {
	mockPage();
	renderApp("/pages/p1");
	await screen.findByRole("list", { name: "Page facts" });
	const links = screen.getAllByRole("link", { name: "https://example.com/about" });
	for (const link of links) {
		expect(link).toHaveAttribute("rel", "noopener noreferrer nofollow");
	}
	expect(links.length).toBeGreaterThan(0);
});

test("an unknown page shows not found", async () => {
	server.use(http.get("/api/v1/pages/nope", () => fail(404, "Page not found.")));
	renderApp("/pages/nope");
	expect(await screen.findByText(/that page was not found/i)).toBeInTheDocument();
});

test("a page that fails to load shows the error and retries", async () => {
	server.use(http.get("/api/v1/pages/p1", () => fail(500, "The page could not be read.")));
	renderApp("/pages/p1");
	expect(await screen.findByText("The page could not be read.")).toBeInTheDocument();
	mockPage();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("heading", { level: 1, name: "About us" })).toBeInTheDocument();
});

test("a page whose findings cannot load says so and offers retry", async () => {
	mockPage();
	server.use(http.get("/api/v1/pages/p1/findings", () => fail(500, "Findings are unavailable.")));
	renderApp("/pages/p1");
	expect(await screen.findByText("Findings are unavailable.")).toBeInTheDocument();
	expect(screen.getByRole("heading", { level: 1, name: "About us" })).toBeInTheDocument();
});

function mockSettings() {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
	);
}

test("delete_stays_disabled_until_the_host_is_typed", async () => {
	mockSettings();
	renderApp("/sites/s1/settings");
	const button = await screen.findByRole("button", { name: "Delete site" });
	expect(button).toBeDisabled();
	const field = screen.getByLabelText(/type example\.com to confirm/i);
	await userEvent.type(field, "example.co");
	expect(button).toBeDisabled();
	await userEvent.type(field, "m");
	expect(button).toBeEnabled();
	await userEvent.type(field, "x");
	expect(button).toBeDisabled();
});

test("deleting_a_site_returns_to_the_list", async () => {
	mockSettings();
	let deleted = false;
	server.use(
		http.delete("/api/v1/sites/s1", () => {
			deleted = true;
			return ok({ deleted: true });
		}),
		http.get("/api/v1/sites", () => ok(deleted ? [] : [site()])),
	);
	renderApp("/sites/s1/settings");
	await userEvent.type(await screen.findByLabelText(/type example\.com to confirm/i), "example.com");
	await userEvent.click(screen.getByRole("button", { name: "Delete site" }));
	expect(await screen.findByRole("heading", { name: /add your first site/i })).toBeInTheDocument();
	expect(deleted).toBe(true);
});

test("a refused delete shows the reason and keeps the site", async () => {
	mockSettings();
	server.use(http.delete("/api/v1/sites/s1", () => fail(404, "Site not found.")));
	renderApp("/sites/s1/settings");
	await userEvent.type(await screen.findByLabelText(/type example\.com to confirm/i), "example.com");
	await userEvent.click(screen.getByRole("button", { name: "Delete site" }));
	expect(await screen.findByRole("alert")).toHaveTextContent("Site not found.");
	await waitFor(() => expect(screen.getByRole("button", { name: "Delete site" })).toBeEnabled());
});
