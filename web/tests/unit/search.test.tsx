import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { chartGeometry } from "../../src/search/TrafficChart";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { emptyMetrics, metrics, site, summary } from "../mocks/fixtures";
import { renderApp } from "../render";

const queries = [
	{ query: "road maps", clicks: 90, impressions: 2000, ctr: 0.045, position: 3.24, top_page: "https://example.com/maps" },
	{ query: "<b>atlas</b>", clicks: 5, impressions: 400, ctr: 0.0125, position: 18, top_page: "javascript:alert(1)" },
];
const pages = [{ url: "https://example.com/maps", clicks: 95, impressions: 2400, ctr: 0.0396, position: 5.5 }];

function mockSearch() {
	const seen: string[] = [];
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/sites/s1/search/queries", ({ request }) => {
			seen.push(new URL(request.url).search);
			return ok(queries, { total: 45, page: Number(new URL(request.url).searchParams.get("page") ?? 1), limit: 20 });
		}),
		http.get("/api/v1/sites/s1/search/pages", () => ok(pages, { total: 1, page: 1, limit: 20 })),
	);
	return seen;
}

test("chart geometry scales values into the box and starts and ends on the data", () => {
	const g = chartGeometry([0, 10, 5], 100, 50, 5);
	expect(g.max).toBe(10);
	expect(g.line.startsWith("M5,45")).toBe(true);
	expect(g.line).toContain("L50,5");
	expect(g.line.endsWith("95,25")).toBe(true);
	expect(g.area.endsWith("Z")).toBe(true);
	expect(chartGeometry([4], 100, 50, 5).line).toBe("M50,5");
	expect(chartGeometry([0, 0], 100, 50, 5).max).toBe(0);
});

test("the search screen shows totals with change, two charts and the tables", async () => {
	mockSearch();
	renderApp("/sites/s1/search");
	const totals = await screen.findByRole("list", { name: "Search totals" });
	expect(totals).toHaveTextContent("1,200");
	expect(totals).toHaveTextContent("+200 vs the previous 28 days");
	expect(totals).toHaveTextContent("30,000");
	expect(totals).toHaveTextContent("14.2");
	expect(screen.getByText(/from google search console/i)).toBeInTheDocument();

	expect(screen.getByRole("img", { name: /Clicks per day/ })).toBeInTheDocument();
	expect(screen.getByRole("img", { name: /Impressions per day/ })).toBeInTheDocument();

	const q = await screen.findByRole("table", { name: "Top queries" });
	const row = within(q).getByText("road maps").closest("tr") as HTMLElement;
	expect(row).toHaveTextContent("4.5%");
	expect(row).toHaveTextContent("3.2");
	expect(within(row).getByRole("link", { name: "https://example.com/maps" })).toHaveAttribute("rel", "noopener noreferrer nofollow");
	const odd = within(q).getByText("<b>atlas</b>").closest("tr") as HTMLElement;
	expect(odd.querySelector("b")).toBeNull();
	expect(within(odd).queryByRole("link")).toBeNull();

	const p = await screen.findByRole("table", { name: "Top pages" });
	expect(within(p).getByRole("link", { name: "https://example.com/maps" })).toBeInTheDocument();
});

test("queries can be sorted and paged", async () => {
	const seen = mockSearch();
	renderApp("/sites/s1/search");
	await screen.findByRole("table", { name: "Top queries" });
	await userEvent.selectOptions(screen.getByLabelText("Sort queries by"), "position");
	await screen.findByRole("table", { name: "Top queries" });
	expect(seen.some((s) => s.includes("sort=position"))).toBe(true);
	await userEvent.click(screen.getByRole("button", { name: "Next page" }));
	await screen.findByText(/Page 2 of 3/);
	expect(seen.some((s) => s.includes("page=2"))).toBe(true);
});

test("not connected explains and links to Connections", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/metrics", () => ok(emptyMetrics())),
	);
	renderApp("/sites/s1/search");
	expect(await screen.findByText("No search numbers yet")).toBeInTheDocument();
	expect(screen.getByText("Connect Google Search Console to see organic traffic.")).toBeInTheDocument();
	expect(screen.getByRole("link", { name: "Open Connections" })).toHaveAttribute("href", "/sites/s1/connections");
});

test("connected with no rows yet says so", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/metrics", () => ok(metrics({ organic: { ...metrics().organic, daily: [] } }))),
		http.get("/api/v1/sites/s1/search/queries", () => ok([], { total: 0, page: 1, limit: 20 })),
		http.get("/api/v1/sites/s1/search/pages", () => ok([], { total: 0, page: 1, limit: 20 })),
	);
	renderApp("/sites/s1/search");
	expect(await screen.findAllByText("No daily numbers yet.")).toHaveLength(2);
	expect(await screen.findAllByText(/no queries yet|no pages yet/i)).toHaveLength(2);
});

test("search shows loading and an error with retry", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/metrics", () => HttpResponse.error()),
	);
	renderApp("/sites/s1/search");
	expect(await screen.findByText("Loading the search numbers…")).toBeInTheDocument();
	expect(await screen.findByText(/can't be reached/i)).toBeInTheDocument();
	mockSearch();
	server.use(http.get("/api/v1/sites/s1/metrics", () => ok(metrics())));
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("list", { name: "Search totals" })).toBeInTheDocument();
});

test("a failing table shows its own error and retry", async () => {
	mockSearch();
	server.use(http.get("/api/v1/sites/s1/search/pages", () => fail(502, "Could not reach Google.")));
	renderApp("/sites/s1/search");
	expect(await screen.findByText("Could not reach Google.")).toBeInTheDocument();
	expect(await screen.findByRole("table", { name: "Top queries" })).toBeInTheDocument();
});
