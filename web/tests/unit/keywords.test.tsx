import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { keyword, site, summary } from "../mocks/fixtures";
import type { Keyword } from "../../src/api/types";
import { renderApp } from "../render";

function mockKeywords(initial: Keyword[]) {
	const state = { list: initial, posted: null as unknown, patched: null as unknown, deleted: null as string | null };
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/sites/s1/keywords", () => ok(state.list)),
		http.post("/api/v1/sites/s1/keywords", async ({ request }) => {
			state.posted = await request.json();
			const body = state.posted as { keyword: string; target_url?: string };
			const added = keyword({ id: "k9", keyword: body.keyword, target_url: body.target_url ?? null });
			state.list = [...state.list, added];
			return ok(added, null, 201);
		}),
		http.patch("/api/v1/keywords/:id", async ({ request, params }) => {
			state.patched = await request.json();
			const target = (state.patched as { target_url: string | null }).target_url;
			state.list = state.list.map((k) => (k.id === params.id ? { ...k, target_url: target } : k));
			return ok(state.list.find((k) => k.id === params.id));
		}),
		http.delete("/api/v1/keywords/:id", ({ params }) => {
			state.deleted = String(params.id);
			state.list = state.list.filter((k) => k.id !== params.id);
			return ok({ deleted: true });
		}),
	);
	return state;
}

test("keywords are listed with their landing page, how it was chosen and search numbers", async () => {
	mockKeywords([
		keyword({ position: 4.26, clicks: 120, impressions: 3400, landing_source: "search_console" }),
		keyword({ id: "k2", keyword: "city guides", landing_url: "https://example.com/", landing_source: "home" }),
		keyword({ id: "k3", keyword: "atlas", landing_url: null, landing_source: null }),
	]);
	renderApp("/sites/s1/keywords");
	const table = await screen.findByRole("table", { name: "Target keywords" });
	const first = within(table).getByText("road maps").closest("tr") as HTMLElement;
	expect(within(first).getByRole("link", { name: "https://example.com/maps" })).toHaveAttribute("href", "https://example.com/maps");
	expect(first).toHaveTextContent("Most shown in Google");
	expect(first).toHaveTextContent("4.3");
	expect(first).toHaveTextContent("120");
	expect(first).toHaveTextContent("3,400");
	const second = within(table).getByText("city guides").closest("tr") as HTMLElement;
	expect(second).toHaveTextContent("Home page, no better match");
	expect(second).toHaveTextContent("–");
	const third = within(table).getByText("atlas").closest("tr") as HTMLElement;
	expect(third).toHaveTextContent("Chosen after the first crawl");
});

test("without Search Console data the screen says where position, clicks and impressions come from", async () => {
	mockKeywords([keyword()]);
	renderApp("/sites/s1/keywords");
	const note = await screen.findByText(/position, clicks and impressions come from google search console/i);
	expect(within(note.parentElement as HTMLElement).getByRole("link", { name: "Connections" })).toHaveAttribute("href", "/sites/s1/connections");
});

test("a keyword with a chosen page shows it as set", async () => {
	mockKeywords([keyword({ target_url: "https://example.com/pricing", landing_url: "https://example.com/pricing", landing_source: "set" })]);
	renderApp("/sites/s1/keywords");
	const row = (await screen.findByText("road maps")).closest("tr") as HTMLElement;
	expect(row).toHaveTextContent("Chosen by you");
});

test("adding a keyword sends it and shows it in the list", async () => {
	const state = mockKeywords([]);
	renderApp("/sites/s1/keywords");
	await userEvent.type(await screen.findByLabelText("Keyword"), "  road maps ");
	await userEvent.type(screen.getByLabelText("Target page (optional)"), "/maps");
	await userEvent.click(screen.getByRole("button", { name: "Add keyword" }));
	expect(await screen.findByRole("table", { name: "Target keywords" })).toBeInTheDocument();
	expect(state.posted).toEqual({ keyword: "road maps", target_url: "/maps" });
	expect(screen.getByLabelText("Keyword")).toHaveValue("");
});

test("a keyword without a target page sends no target", async () => {
	const state = mockKeywords([]);
	renderApp("/sites/s1/keywords");
	await userEvent.type(await screen.findByLabelText("Keyword"), "atlas");
	await userEvent.click(screen.getByRole("button", { name: "Add keyword" }));
	await screen.findByRole("table", { name: "Target keywords" });
	expect(state.posted).toEqual({ keyword: "atlas" });
});

test("an empty keyword is refused without asking the server", async () => {
	const state = mockKeywords([]);
	renderApp("/sites/s1/keywords");
	await userEvent.click(await screen.findByRole("button", { name: "Add keyword" }));
	expect(await screen.findByText("Type the keyword first.")).toBeInTheDocument();
	expect(state.posted).toBeNull();
});

test("the server's reason is shown and the typed keyword stays", async () => {
	mockKeywords([]);
	server.use(http.post("/api/v1/sites/s1/keywords", () => fail(409, "A site can have at most 50 keywords.")));
	renderApp("/sites/s1/keywords");
	await userEvent.type(await screen.findByLabelText("Keyword"), "atlas");
	await userEvent.click(screen.getByRole("button", { name: "Add keyword" }));
	expect(await screen.findByText("A site can have at most 50 keywords.")).toBeInTheDocument();
	expect(screen.getByLabelText("Keyword")).toHaveValue("atlas");
});

test("a target page outside the site is refused with the server's message", async () => {
	mockKeywords([]);
	server.use(http.post("/api/v1/sites/s1/keywords", () => fail(422, "That page is not on this site.")));
	renderApp("/sites/s1/keywords");
	await userEvent.type(await screen.findByLabelText("Keyword"), "atlas");
	await userEvent.type(screen.getByLabelText("Target page (optional)"), "https://other.test/");
	await userEvent.click(screen.getByRole("button", { name: "Add keyword" }));
	expect(await screen.findByText("That page is not on this site.")).toBeInTheDocument();
	expect(screen.getByLabelText("Target page (optional)")).toHaveValue("https://other.test/");
});

test("removing a keyword deletes it", async () => {
	const state = mockKeywords([keyword()]);
	renderApp("/sites/s1/keywords");
	await userEvent.click(await screen.findByRole("button", { name: "Remove road maps" }));
	expect(await screen.findByText(/no keywords yet/i)).toBeInTheDocument();
	expect(state.deleted).toBe("k1");
});

test("a failed removal shows the message", async () => {
	mockKeywords([keyword()]);
	server.use(http.delete("/api/v1/keywords/k1", () => fail(404, "Keyword not found.")));
	renderApp("/sites/s1/keywords");
	await userEvent.click(await screen.findByRole("button", { name: "Remove road maps" }));
	expect(await screen.findByText("Keyword not found.")).toBeInTheDocument();
});

test("a target page can be set and cleared", async () => {
	const state = mockKeywords([keyword()]);
	renderApp("/sites/s1/keywords");
	await userEvent.click(await screen.findByRole("button", { name: "Set target page for road maps" }));
	await userEvent.type(screen.getByLabelText("Target page for road maps"), "/maps");
	await userEvent.click(screen.getByRole("button", { name: "Save" }));
	expect(await screen.findByText("/maps")).toBeInTheDocument();
	expect(state.patched).toEqual({ target_url: "/maps" });

	await userEvent.click(screen.getByRole("button", { name: "Change target page for road maps" }));
	await userEvent.clear(screen.getByLabelText("Target page for road maps"));
	await userEvent.click(screen.getByRole("button", { name: "Save" }));
	await screen.findByRole("button", { name: "Set target page for road maps" });
	expect(state.patched).toEqual({ target_url: null });
});

test("editing a target page can be cancelled and shows the server's refusal", async () => {
	mockKeywords([keyword()]);
	server.use(http.patch("/api/v1/keywords/k1", () => fail(422, "That page is not on this site.")));
	renderApp("/sites/s1/keywords");
	await userEvent.click(await screen.findByRole("button", { name: "Set target page for road maps" }));
	await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
	expect(screen.queryByLabelText("Target page for road maps")).toBeNull();
	await userEvent.click(screen.getByRole("button", { name: "Set target page for road maps" }));
	await userEvent.type(screen.getByLabelText("Target page for road maps"), "https://other.test");
	await userEvent.click(screen.getByRole("button", { name: "Save" }));
	expect(await screen.findByText("That page is not on this site.")).toBeInTheDocument();
});

test("an empty site invites the first keyword", async () => {
	mockKeywords([]);
	renderApp("/sites/s1/keywords");
	expect(await screen.findByText(/no keywords yet/i)).toBeInTheDocument();
	expect(screen.getByLabelText("Keyword")).toBeInTheDocument();
});

test("keywords show loading, then an error with retry", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/keywords", () => HttpResponse.error()),
	);
	renderApp("/sites/s1/keywords");
	expect(await screen.findByText("Loading the keywords…")).toBeInTheDocument();
	expect(await screen.findByText(/can't be reached/i)).toBeInTheDocument();
	mockKeywords([keyword()]);
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("table", { name: "Target keywords" })).toBeInTheDocument();
});

test("a crawled landing address that is not http is not a link", async () => {
	mockKeywords([keyword({ landing_url: "javascript:alert(1)" })]);
	renderApp("/sites/s1/keywords");
	const row = (await screen.findByText("road maps")).closest("tr") as HTMLElement;
	expect(within(row).queryByRole("link")).toBeNull();
	expect(row).toHaveTextContent("javascript:alert(1)");
});
