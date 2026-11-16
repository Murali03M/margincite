import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { MetricCards } from "../../src/metrics/MetricCards";
import { server } from "../mocks/server";
import { ok } from "../mocks/handlers";
import { emptyMetrics, metrics } from "../mocks/fixtures";
import { renderAt } from "../render";

const ORDER = ["AI Visibility", "Mentions", "Site Health", "Page speed", "Visibility", "Rankings", "Organic Traffic", "Organic Keywords", "Backlinks"];

function tile(name: string): HTMLElement {
	return screen.getByText(name, { selector: "[data-tile-label]" }).closest("li") as HTMLElement;
}

test("the card shows nine tiles in order with numbers, changes and sources", async () => {
	renderAt(<MetricCards siteId="s1" />);
	const list = await screen.findByRole("list", { name: "Project card" });
	const labels = within(list).getAllByRole("listitem").map((li) => li.querySelector("[data-tile-label]")?.textContent);
	expect(labels).toEqual(ORDER);

	expect(tile("AI Visibility")).toHaveTextContent("40%");
	expect(tile("Mentions")).toHaveTextContent("6");
	expect(tile("Mentions")).toHaveTextContent("of 15 answers");
	expect(tile("Site Health")).toHaveTextContent("72");
	expect(tile("Site Health")).toHaveTextContent("+4 since the previous crawl");
	expect(tile("Visibility")).toHaveTextContent("50%");
	expect(tile("Visibility")).toHaveTextContent("2 of 4 keywords in the top 10");
	expect(tile("Organic Traffic")).toHaveTextContent("1,200");
	expect(tile("Organic Traffic")).toHaveTextContent("+200 vs the previous 28 days");
	expect(tile("Organic Traffic")).toHaveTextContent("From Google Search Console, last 28 days");
	expect(tile("Organic Keywords")).toHaveTextContent("80");
	expect(tile("Organic Keywords")).toHaveTextContent("-5 vs the previous 28 days");
	expect(tile("Backlinks")).toHaveTextContent("530");
	expect(tile("Backlinks")).toHaveTextContent("From Bing Webmaster Tools");
	expect(tile("Backlinks").getAttribute("title")).toMatch(/Bing/);
	expect(tile("Rankings")).toHaveTextContent("3");
	expect(tile("Rankings")).toHaveTextContent("of 4 keywords in the top 10");
	expect(tile("Rankings").getAttribute("title")).toMatch(/average position 8\.5/);
	expect(tile("Page speed")).toHaveTextContent("64");
	expect(tile("Page speed")).toHaveTextContent(/estimate/i);
	expect(tile("Visibility").getAttribute("title")).toMatch(/from your rankings/i);
});

test("visibility says it comes from Search Console when there are no rankings", async () => {
	server.use(
		http.get("/api/v1/sites/s1/metrics", () =>
			ok(metrics({ rankings: { tracked: 0, in_top_3: null, in_top_10: null, average_position: null, checked_at: null, reason: "No rankings yet." } })),
		),
	);
	renderAt(<MetricCards siteId="s1" />);
	await screen.findByRole("list", { name: "Project card" });
	expect(tile("Visibility").getAttribute("title")).toMatch(/Search Console/);
});

test("a number that cannot be shown says n/a with its reason and a link to fix it", async () => {
	server.use(http.get("/api/v1/sites/s1/metrics", () => ok(emptyMetrics())));
	renderAt(<MetricCards siteId="s1" />);
	await screen.findByRole("list", { name: "Project card" });

	expect(tile("AI Visibility")).toHaveTextContent("n/a");
	expect(tile("AI Visibility")).toHaveTextContent("Run an AI visibility check to see this.");
	expect(within(tile("AI Visibility")).getByRole("link", { name: /AI Visibility/ })).toHaveAttribute("href", "/sites/s1/ai");
	expect(within(tile("Mentions")).getByRole("link")).toHaveAttribute("href", "/sites/s1/ai");
	expect(tile("Site Health")).toHaveTextContent("Crawl the site to get a health score.");
	expect(within(tile("Visibility")).getByRole("link")).toHaveAttribute("href", "/sites/s1/keywords");
	expect(tile("Organic Traffic")).toHaveTextContent("Connect Google Search Console to see organic traffic.");
	expect(within(tile("Organic Traffic")).getByRole("link")).toHaveAttribute("href", "/sites/s1/connections");
	expect(within(tile("Organic Keywords")).getByRole("link")).toHaveAttribute("href", "/sites/s1/connections");
	expect(within(tile("Backlinks")).getByRole("link")).toHaveAttribute("href", "/sites/s1/connections");
	expect(tile("Rankings")).toHaveTextContent("Add search data in Settings to track rankings.");
	expect(within(tile("Rankings")).getByRole("link")).toHaveAttribute("href", "/sites/s1/rankings");
	expect(tile("Page speed")).toHaveTextContent("Page speed has not been checked yet.");
	expect(within(tile("Page speed")).getByRole("link", { name: /^Open Page speed/ })).toHaveAttribute("href", "/sites/s1/checks#page-speed");
});

test("visibility with keywords but no Search Console points to Connections", async () => {
	const data = emptyMetrics();
	server.use(
		http.get("/api/v1/sites/s1/metrics", () =>
			ok({ ...data, visibility: { percent: null, in_top_10: null, keywords: 3, reason: "Connect Google Search Console to see this." } }),
		),
	);
	renderAt(<MetricCards siteId="s1" />);
	await screen.findByRole("list", { name: "Project card" });
	expect(within(tile("Visibility")).getByRole("link")).toHaveAttribute("href", "/sites/s1/connections");
});

test("a zero is shown as a number, not as n/a, and no change is drawn without a previous value", async () => {
	server.use(
		http.get("/api/v1/sites/s1/metrics", () =>
			ok(
				metrics({
					site_health: { score: 0, previous: null, history: [], reason: null },
					backlinks: { total: 0, linked_pages: 0, top_pages: [], source: "bing", synced_at: null, reason: null },
				}),
			),
		),
	);
	renderAt(<MetricCards siteId="s1" />);
	await screen.findByRole("list", { name: "Project card" });
	expect(tile("Site Health")).not.toHaveTextContent("n/a");
	expect(tile("Site Health")).not.toHaveTextContent("since the previous");
	expect(tile("Backlinks")).toHaveTextContent("0");
	expect(tile("Backlinks")).not.toHaveTextContent("n/a");
});

test("an unchanged number says so", async () => {
	server.use(
		http.get("/api/v1/sites/s1/metrics", () =>
			ok(metrics({ site_health: { score: 70, previous: 70, history: [], reason: null } })),
		),
	);
	renderAt(<MetricCards siteId="s1" />);
	await screen.findByRole("list", { name: "Project card" });
	expect(tile("Site Health")).toHaveTextContent("No change since the previous crawl");
});

test("a card that fails to load shows the message and retries", async () => {
	server.use(http.get("/api/v1/sites/s1/metrics", () => HttpResponse.error()));
	renderAt(<MetricCards siteId="s1" />);
	expect(await screen.findByText(/can't be reached/i)).toBeInTheDocument();
	server.use(http.get("/api/v1/sites/s1/metrics", () => ok(metrics())));
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("list", { name: "Project card" })).toBeInTheDocument();
});

test("while loading the card says so", () => {
	renderAt(<MetricCards siteId="s1" />);
	expect(screen.getByRole("status")).toHaveTextContent(/loading/i);
});
