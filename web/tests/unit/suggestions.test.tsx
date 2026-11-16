import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { Route, Routes, useLocation } from "react-router-dom";
import type { ItemStatus } from "../../src/api/types";
import { SuggestionsPage } from "../../src/suggestions/SuggestionsPage";
import { evidenceLines } from "../../src/suggestions/evidence";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { item, suggestion, suggestionDetail } from "../mocks/fixtures";
import { renderApp, renderAt } from "../render";

const list = [
	suggestion({ id: "g1", title: "Pages are missing a title", open: 4, severity: 3 }),
	suggestion({ id: "g2", title: "Pages have no meta description", open: 2, severity: 1, category: "content" }),
	suggestion({ id: "g3", title: "Images have no alt text", open: 9, severity: 2, category: "ai_readiness" }),
];

function Probe() {
	const location = useLocation();
	return <p data-testid="where">{location.pathname + location.search}</p>;
}

function renderList(path = "/sites/s1/suggestions") {
	return renderAt(
		<>
			<Routes>
				<Route path="/sites/:id/suggestions" element={<SuggestionsPage />} />
			</Routes>
			<Probe />
		</>,
		path,
	);
}

test("suggestions_are_listed_in_the_servers_order_with_their_counts", async () => {
	server.use(
		http.get("/api/v1/sites/s1/suggestions", () => ok(list, { total: 3, page: 1, limit: 20 })),
	);
	renderList();
	const table = await screen.findByRole("table", { name: "Suggestions" });
	const rows = within(table).getAllByRole("row").slice(1);
	expect(rows).toHaveLength(3);
	expect(within(rows[0]).getByRole("link", { name: "Pages are missing a title" })).toHaveAttribute(
		"href",
		"/suggestions/g1?site=s1",
	);
	expect(rows[0]).toHaveTextContent("1");
	expect(rows[0]).toHaveTextContent("Technical");
	expect(rows[0]).toHaveTextContent("High impact");
	expect(rows[0]).toHaveTextContent("4");
	expect(within(rows[1]).getByRole("link")).toHaveTextContent("Pages have no meta description");
	expect(rows[2]).toHaveTextContent("AI readiness");
	expect(rows[2]).toHaveTextContent("9");
});

test("changing_a_filter_updates_the_address_and_the_request", async () => {
	const seen: string[] = [];
	server.use(
		http.get("/api/v1/sites/s1/suggestions", ({ request }) => {
			seen.push(new URL(request.url).search);
			return ok(list, { total: 3, page: 1, limit: 20 });
		}),
	);
	renderList();
	await screen.findByRole("table", { name: "Suggestions" });
	expect(seen[0]).toContain("status=open");

	await userEvent.selectOptions(screen.getByLabelText("Status"), "done");
	await userEvent.selectOptions(screen.getByLabelText("Category"), "content");

	await waitFor(() => expect(seen.at(-1)).toMatch(/status=done/));
	expect(seen.at(-1)).toMatch(/category=content/);
	expect(screen.getByTestId("where")).toHaveTextContent("/sites/s1/suggestions?status=done&category=content");
});

test("a_filtered_address_loads_with_that_filter_applied", async () => {
	const seen: string[] = [];
	server.use(
		http.get("/api/v1/sites/s1/suggestions", ({ request }) => {
			seen.push(new URL(request.url).search);
			return ok([list[1]], { total: 1, page: 1, limit: 20 });
		}),
	);
	renderList("/sites/s1/suggestions?status=dismissed&category=content&page=1");
	await screen.findByRole("table", { name: "Suggestions" });
	expect(screen.getByLabelText("Status")).toHaveValue("dismissed");
	expect(screen.getByLabelText("Category")).toHaveValue("content");
	expect(seen[0]).toContain("status=dismissed");
	expect(seen[0]).toContain("category=content");
});

test("a list that spans pages can move to the next page", async () => {
	const seen: string[] = [];
	server.use(
		http.get("/api/v1/sites/s1/suggestions", ({ request }) => {
			const page = new URL(request.url).searchParams.get("page") ?? "1";
			seen.push(page);
			return ok(list, { total: 45, page: Number(page), limit: 20 });
		}),
	);
	renderList();
	expect(await screen.findByText("Page 1 of 3")).toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Previous page" })).toBeDisabled();
	await userEvent.click(screen.getByRole("button", { name: "Next page" }));
	expect(await screen.findByText("Page 2 of 3")).toBeInTheDocument();
	expect(screen.getByTestId("where")).toHaveTextContent("page=2");
	expect(seen).toContain("2");
});

test("no_open_suggestions_shows_a_clear_empty_state", async () => {
	server.use(http.get("/api/v1/sites/s1/suggestions", () => ok([], { total: 0, page: 1, limit: 20 })));
	renderList();
	expect(await screen.findByRole("heading", { name: "Nothing open" })).toBeInTheDocument();
	expect(screen.getByText(/crawl again after you change the site/i)).toBeInTheDocument();
	expect(screen.queryByRole("table")).not.toBeInTheDocument();
});

test("an empty filtered list offers to clear the filters", async () => {
	server.use(http.get("/api/v1/sites/s1/suggestions", () => ok([], { total: 0, page: 1, limit: 20 })));
	renderList("/sites/s1/suggestions?status=done");
	expect(await screen.findByRole("heading", { name: "No suggestions match" })).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: "Show open suggestions" }));
	await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("/sites/s1/suggestions"));
	expect(screen.getByTestId("where")).not.toHaveTextContent("status=done");
});

test("the suggestions list shows an error with retry", async () => {
	server.use(http.get("/api/v1/sites/s1/suggestions", () => fail(500, "Suggestions are unavailable.")));
	renderList();
	expect(await screen.findByText("Suggestions are unavailable.")).toBeInTheDocument();
	server.use(http.get("/api/v1/sites/s1/suggestions", () => ok(list, { total: 3, page: 1, limit: 20 })));
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("table", { name: "Suggestions" })).toBeInTheDocument();
});

/** A server that remembers: a PATCH changes what the next GET returns, as the real one does. */
function mockDetail(data = suggestionDetail()) {
	let items = data.items;
	const bodies: { status: ItemStatus; targets?: string[] }[] = [];
	server.use(
		http.get("/api/v1/suggestions/g1", () =>
			ok({ ...data, items }, { total: items.length, page: 1, limit: 100 }),
		),
		http.patch("/api/v1/suggestions/g1", async ({ request }) => {
			const body = (await request.json()) as { status: ItemStatus; targets?: string[] };
			bodies.push(body);
			items = items.map((entry) =>
				!body.targets || body.targets.includes(entry.target) ? { ...entry, status: body.status } : entry,
			);
			return ok({ changed: 1 });
		}),
	);
	return bodies;
}

test("a_suggestion_shows_why_fix_and_each_item", async () => {
	mockDetail();
	renderApp("/suggestions/g1?site=s1");
	expect(await screen.findByRole("heading", { level: 1, name: "Pages are missing a title" })).toBeInTheDocument();
	expect(screen.getByRole("heading", { name: "Why it matters" })).toBeInTheDocument();
	expect(screen.getByText("A page title is the first line a reader and a crawler see.")).toBeInTheDocument();
	expect(screen.getByRole("heading", { name: "How to fix it" })).toBeInTheDocument();
	expect(screen.getByText(/give every page a short title/i)).toBeInTheDocument();

	const table = screen.getByRole("table", { name: "Affected items" });
	const rows = within(table).getAllByRole("row").slice(1);
	expect(rows).toHaveLength(2);
	const first = rows[0];
	expect(within(first).getByRole("link", { name: "https://example.com/about" })).toHaveAttribute(
		"rel",
		"noopener noreferrer nofollow",
	);
	expect(first.querySelector(".tag")).toHaveTextContent("Open");
	expect(first).toHaveTextContent("1 Nov 2026");
	expect(first).toHaveTextContent("Title length: 0");

	expect(screen.getByRole("link", { name: /back to suggestions/i })).toHaveAttribute("href", "/sites/s1/suggestions");
});

test("a suggestion opened without a site links back to the sites list", async () => {
	mockDetail();
	renderApp("/suggestions/g1");
	expect(await screen.findByRole("link", { name: /back to your sites/i })).toHaveAttribute("href", "/");
});

test("an unknown suggestion shows not found", async () => {
	server.use(http.get("/api/v1/suggestions/nope", () => fail(404, "Suggestion not found.")));
	renderApp("/suggestions/nope");
	expect(await screen.findByText(/that suggestion was not found/i)).toBeInTheDocument();
});

test("marking_an_item_done_updates_its_row", async () => {
	const bodies = mockDetail();
	renderApp("/suggestions/g1?site=s1");
	const row = (await screen.findByRole("link", { name: "https://example.com/about" })).closest("tr") as HTMLElement;
	await userEvent.click(within(row).getByRole("button", { name: /^Done/ }));
	await waitFor(() => expect(row.querySelector(".tag")).toHaveTextContent("Done"));
	expect(within(row).getByRole("button", { name: /^Reopen/ })).toBeEnabled();
	expect(within(row).getByRole("button", { name: /^Done/ })).toBeDisabled();
	expect(bodies).toEqual([{ status: "done", targets: ["https://example.com/about"] }]);
});

test("dismissing and reopening one item sends its target", async () => {
	const bodies = mockDetail(suggestionDetail({ items: [item({ status: "dismissed" })] }));
	renderApp("/suggestions/g1?site=s1");
	const row = (await screen.findByRole("link", { name: "https://example.com/about" })).closest("tr") as HTMLElement;
	await userEvent.click(within(row).getByRole("button", { name: /^Reopen/ }));
	await waitFor(() => expect(bodies).toHaveLength(1));
	expect(bodies[0]).toEqual({ status: "open", targets: ["https://example.com/about"] });
	await userEvent.click(within(row).getByRole("button", { name: /^Dismiss/ }));
	await waitFor(() => expect(bodies).toHaveLength(2));
	expect(bodies[1]).toEqual({ status: "dismissed", targets: ["https://example.com/about"] });
});

test("marking every item done sends no targets", async () => {
	const bodies = mockDetail();
	renderApp("/suggestions/g1?site=s1");
	await userEvent.click(await screen.findByRole("button", { name: "Mark all done" }));
	await waitFor(() => expect(bodies).toEqual([{ status: "done" }]));
	const rows = within(screen.getByRole("table", { name: "Affected items" })).getAllByRole("row").slice(1);
	for (const row of rows) expect(row.querySelector(".tag")).toHaveTextContent("Done");
});

test("a_failed_change_rolls_back_and_says_so", async () => {
	mockDetail();
	server.use(http.patch("/api/v1/suggestions/g1", () => fail(422, "That status is not allowed.")));
	renderApp("/suggestions/g1?site=s1");
	const row = (await screen.findByRole("link", { name: "https://example.com/about" })).closest("tr") as HTMLElement;
	await userEvent.click(within(row).getByRole("button", { name: /^Done/ }));
	const alert = await screen.findByRole("alert");
	expect(alert).toHaveTextContent("That status is not allowed.");
	expect(alert).toHaveTextContent(/not saved/i);
	await waitFor(() => expect(within(row).getByRole("button", { name: /^Done/ })).toBeEnabled());
	expect(row.querySelector(".tag")).toHaveTextContent("Open");
});

test("dismissing_the_whole_suggestion_asks_first", async () => {
	const bodies = mockDetail();
	renderApp("/suggestions/g1?site=s1");
	await userEvent.click(await screen.findByRole("button", { name: "Dismiss all" }));
	expect(screen.getByText(/dismiss all 2 items/i)).toBeInTheDocument();
	expect(bodies).toHaveLength(0);

	await userEvent.click(screen.getByRole("button", { name: "Keep them" }));
	expect(screen.queryByText(/dismiss all 2 items/i)).not.toBeInTheDocument();
	expect(bodies).toHaveLength(0);

	await userEvent.click(screen.getByRole("button", { name: "Dismiss all" }));
	await userEvent.click(screen.getByRole("button", { name: "Yes, dismiss all" }));
	await waitFor(() => expect(bodies).toEqual([{ status: "dismissed" }]));
});

test("a suggestion with no items says so", async () => {
	mockDetail(suggestionDetail({ items: [] }));
	renderApp("/suggestions/g1?site=s1");
	expect(await screen.findByText(/no items to show/i)).toBeInTheDocument();
});

test("a suggestion that fails to load shows the error and retries", async () => {
	server.use(http.get("/api/v1/suggestions/g1", () => fail(500, "The suggestion could not be read.")));
	renderApp("/suggestions/g1?site=s1");
	expect(await screen.findByText("The suggestion could not be read.")).toBeInTheDocument();
	mockDetail();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("heading", { level: 1 })).toBeInTheDocument();
});

test("evidence_is_rendered_as_text_lines", async () => {
	mockDetail(
		suggestionDetail({
			items: [
				item({
					evidence: {
						title_length: 0,
						words: ["alpha", "beta"],
						found_in: { header: true, footer: null },
						"<img src=x onerror=alert(1)>": "<b>bold</b>",
					},
				}),
			],
		}),
	);
	renderApp("/suggestions/g1?site=s1");
	const cell = (await screen.findByRole("link", { name: "https://example.com/about" })).closest("tr") as HTMLElement;
	const lines = within(cell).getAllByRole("listitem").map((li) => li.textContent);
	expect(lines).toEqual([
		"Title length: 0",
		"Words: alpha, beta",
		"Found in header: yes",
		"Found in footer: none",
		"<img src=x onerror=alert(1)>: <b>bold</b>",
	]);
	expect(cell.querySelector("img, b")).toBeNull();
	expect(cell.textContent).not.toMatch(/[{}[\]"]/);
});

test("evidence lines handle empty and awkward values", () => {
	expect(evidenceLines({})).toEqual([]);
	expect(evidenceLines({ list: [], nested: [{ a: 1 }, { a: 2 }], ratio: 0.5, nothing: undefined })).toEqual([
		"List: none",
		"Nested a: 1",
		"Nested a: 2",
		"Ratio: 0.5",
		"Nothing: none",
	]);
});
