import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { emptyMetrics, metrics, site } from "../mocks/fixtures";
import { renderApp } from "../render";

const targets = [
	{ target_url: "https://example.com/", count: 400 },
	{ target_url: "https://example.com/maps", count: 130 },
];

function mockBacklinks() {
	const asked: string[] = [];
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/backlinks", () => ok(targets, { total: 45, page: 1, limit: 20 })),
		http.get("/api/v1/sites/s1/backlinks/sources", ({ request }) => {
			const target = new URL(request.url).searchParams.get("target_url") ?? "";
			asked.push(target);
			if (target.endsWith("/maps")) return ok([]);
			return ok([
				{ source_url: "https://blog.test/post", anchor_text: "great <i>maps</i>" },
				{ source_url: "javascript:alert(1)", anchor_text: null },
			]);
		}),
	);
	return asked;
}

test("backlinks show the totals and the most linked pages", async () => {
	mockBacklinks();
	renderApp("/sites/s1/backlinks");
	const totals = await screen.findByRole("list", { name: "Backlink totals" });
	expect(totals).toHaveTextContent("530");
	expect(totals).toHaveTextContent("12");
	expect(screen.getByText(/from bing webmaster tools/i)).toBeInTheDocument();
	const table = await screen.findByRole("table", { name: "Most linked pages" });
	const row = within(table).getByText("https://example.com/maps").closest("tr") as HTMLElement;
	expect(row).toHaveTextContent("130");
	expect(screen.getByText(/Page 1 of 3/)).toBeInTheDocument();
});

test("a page's sources load on demand and are shown safely", async () => {
	const asked = mockBacklinks();
	renderApp("/sites/s1/backlinks");
	await screen.findByRole("table", { name: "Most linked pages" });
	expect(asked).toEqual([]);
	await userEvent.click(screen.getByRole("button", { name: "Show sources for https://example.com/" }));
	const list = await screen.findByRole("list", { name: "Pages linking to https://example.com/" });
	expect(asked).toEqual(["https://example.com/"]);
	const link = within(list).getByRole("link", { name: "https://blog.test/post" });
	expect(link).toHaveAttribute("href", "https://blog.test/post");
	expect(link).toHaveAttribute("rel", "noopener noreferrer nofollow");
	expect(within(list).getByText("great <i>maps</i>")).toBeInTheDocument();
	expect(list.querySelector("i")).toBeNull();
	expect(within(list).getByText("javascript:alert(1)").closest("a")).toBeNull();
});

test("a page without stored sources explains why", async () => {
	mockBacklinks();
	renderApp("/sites/s1/backlinks");
	await userEvent.click(await screen.findByRole("button", { name: "Show sources for https://example.com/maps" }));
	expect(await screen.findByText(/only the 10 most linked pages/i)).toBeInTheDocument();
});

test("sources can be closed and a failing request shows its message", async () => {
	mockBacklinks();
	server.use(http.get("/api/v1/sites/s1/backlinks/sources", () => fail(422, "Bing sources are unavailable.")));
	renderApp("/sites/s1/backlinks");
	await userEvent.click(await screen.findByRole("button", { name: "Show sources for https://example.com/" }));
	expect(await screen.findByText("Bing sources are unavailable.")).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: "Close sources" }));
	expect(screen.queryByText("Bing sources are unavailable.")).toBeNull();
});

test("not connected explains and links to Connections", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/metrics", () => ok(emptyMetrics())),
	);
	renderApp("/sites/s1/backlinks");
	expect(await screen.findByText("No backlink numbers yet")).toBeInTheDocument();
	expect(screen.getByText("Connect Bing Webmaster Tools to see backlinks.")).toBeInTheDocument();
	expect(screen.getByRole("link", { name: "Open Connections" })).toHaveAttribute("href", "/sites/s1/connections");
});

test("a synced site with no links shows zero and an empty list", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/metrics", () =>
			ok(metrics({ backlinks: { total: 0, linked_pages: 0, top_pages: [], source: "bing", synced_at: "2026-11-12T10:00:00Z", reason: null } })),
		),
		http.get("/api/v1/sites/s1/backlinks", () => ok([], { total: 0, page: 1, limit: 20 })),
	);
	renderApp("/sites/s1/backlinks");
	expect(await screen.findByText("No backlinks yet")).toBeInTheDocument();
	expect(screen.getByRole("list", { name: "Backlink totals" })).toHaveTextContent("0");
});

test("backlinks show loading and an error with retry", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/metrics", () => HttpResponse.error()),
	);
	renderApp("/sites/s1/backlinks");
	expect(await screen.findByText("Loading the backlinks…")).toBeInTheDocument();
	expect(await screen.findByText(/can't be reached/i)).toBeInTheDocument();
	mockBacklinks();
	server.use(http.get("/api/v1/sites/s1/metrics", () => ok(metrics())));
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("list", { name: "Backlink totals" })).toBeInTheDocument();
});

test("a failing list shows its error", async () => {
	mockBacklinks();
	server.use(http.get("/api/v1/sites/s1/backlinks", () => fail(500, "Backlinks could not be read.")));
	renderApp("/sites/s1/backlinks");
	expect(await screen.findByText("Backlinks could not be read.")).toBeInTheDocument();
});
