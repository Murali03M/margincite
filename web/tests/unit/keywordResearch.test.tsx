import { act, fireEvent, screen, within } from "@testing-library/react";
import { Route, Routes } from "react-router-dom";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { idea, keyword, site, summary } from "../mocks/fixtures";
import type { Keyword, KeywordIdea, VolumeStatus } from "../../src/api/types";
import { KeywordsPage } from "../../src/keywords/KeywordsPage";
import { renderApp, renderAt } from "../render";

function mock(initial: Keyword[], ideas: KeywordIdea[] = []) {
	const state = {
		list: initial,
		volumePosts: 0,
		volumeGets: 0,
		volume: { status: "idle", checked_at: null, error: null } as VolumeStatus,
		onVolumePost: null as null | (() => Response | undefined),
		seeds: [] as string[], added: [] as string[], ideaReply: null as null | (() => Response) };
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/sites/s1/keywords", () => ok(state.list)),
		http.post("/api/v1/sites/s1/keywords", async ({ request }) => {
			const body = (await request.json()) as { keyword: string };
			state.added.push(body.keyword);
			const added = keyword({ id: `n${state.added.length}`, keyword: body.keyword });
			state.list = [...state.list, added];
			return ok(added, null, 201);
		}),
		http.get("/api/v1/sites/s1/keywords/volumes", () => {
			state.volumeGets += 1;
			return ok(state.volume);
		}),
		http.post("/api/v1/sites/s1/keywords/volumes", () => {
			state.volumePosts += 1;
			return state.onVolumePost?.() ?? ok({ queued: true }, null, 202);
		}),
		http.post("/api/v1/sites/s1/keywords/ideas", async ({ request }) => {
			const body = (await request.json()) as { seed: string };
			state.seeds.push(body.seed);
			return state.ideaReply?.() ?? ok({ ideas });
		}),
	);
	return state;
}

test("volume, competition and cost per click are columns, marked as estimates", async () => {
	mock([
		keyword({ search_volume: 2400, competition: "LOW", cpc: 0.8 }),
		keyword({ id: "k2", keyword: "atlas", search_volume: null, competition: null, cpc: null }),
	]);
	renderApp("/sites/s1/keywords");
	const table = await screen.findByRole("table", { name: "Target keywords" });
	const headers = within(table).getAllByRole("columnheader").map((h) => h.textContent);
	expect(headers).toEqual(expect.arrayContaining(["Searches a month (estimate)", "Competition", "Cost per click (estimate)"]));
	const first = within(table).getByText("road maps").closest("tr") as HTMLElement;
	expect(first).toHaveTextContent("2,400");
	expect(first).toHaveTextContent("Low");
	expect(first).toHaveTextContent("$0.80");
	const second = within(table).getByText("atlas").closest("tr") as HTMLElement;
	expect(second).not.toHaveTextContent("$");
	expect(second).not.toHaveTextContent(/\b0\b/);
});

test("without any volumes the screen says where they come from and links to Settings", async () => {
	mock([keyword()]);
	renderApp("/sites/s1/keywords");
	const note = await screen.findByText(/search volume, competition and cost per click come from dataforseo/i);
	expect(within(note.parentElement as HTMLElement).getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
});

function renderKeywords(pollMs: number) {
	return renderAt(
		<Routes>
			<Route path="/sites/:id/keywords" element={<KeywordsPage pollMs={pollMs} />} />
		</Routes>,
		"/sites/s1/keywords",
	);
}

test("Refresh volumes shows progress until the status is idle, then the table has the new numbers and no reload is asked for", async () => {
	const state = mock([keyword({ search_volume: null })]);
	state.onVolumePost = () => {
		state.volume = { status: "queued", checked_at: null, error: null };
		return undefined;
	};
	renderKeywords(20);
	await userEvent.click(await screen.findByRole("button", { name: "Refresh volumes" }));
	expect(await screen.findByRole("status")).toHaveTextContent(/refreshing the search numbers/i);
	expect(screen.getByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument();
	expect(screen.queryByText(/reload/i)).toBeNull();
	state.volume = { status: "running", checked_at: null, error: null };
	await vi.waitFor(() => expect(state.volumeGets).toBeGreaterThanOrEqual(4));
	expect(screen.getByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument();

	state.list = [keyword({ search_volume: 2400 })];
	state.volume = { status: "idle", checked_at: "2026-11-15T09:00:00Z", error: null };
	await vi.waitFor(() => expect(screen.queryByRole("progressbar", { name: "Check in progress" })).toBeNull());
	expect(await within(await screen.findByRole("table", { name: "Target keywords" })).findByText("2,400")).toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Refresh volumes" })).toBeEnabled();
	expect(state.volumePosts).toBe(1);
});

test("Refresh volumes shows the error when the refresh failed", async () => {
	const state = mock([keyword()]);
	state.onVolumePost = () => {
		state.volume = { status: "failed", checked_at: null, error: "DataForSEO says the account is out of credit." };
		return undefined;
	};
	renderKeywords(20);
	await userEvent.click(await screen.findByRole("button", { name: "Refresh volumes" }));
	expect(await screen.findByText(/DataForSEO says the account is out of credit\./)).toBeInTheDocument();
	expect(screen.queryByRole("progressbar")).toBeNull();
});

test("Refresh volumes says plainly when the daily limit is reached", async () => {
	const state = mock([keyword()]);
	state.onVolumePost = () => fail(429, "You have used all 5 volume refreshes for today.");
	renderKeywords(20);
	await userEvent.click(await screen.findByRole("button", { name: "Refresh volumes" }));
	expect(await screen.findByText("You have used all 5 volume refreshes for today.")).toBeInTheDocument();
	expect(screen.queryByRole("progressbar")).toBeNull();
});

test("a refresh that already failed shows its error on first render instead of the Add it in Settings hint", async () => {
	const state = mock([keyword({ search_volume: null })]);
	state.volume = { status: "failed", checked_at: null, error: "DataForSEO refused the request." };
	renderKeywords(20);
	expect(await screen.findByText(/DataForSEO refused the request\./)).toBeInTheDocument();
	await screen.findByRole("table", { name: "Target keywords" });
	expect(screen.queryByText(/Add it in/)).toBeNull();
});

test("a refresh that is already running on first render is followed", async () => {
	const state = mock([keyword()]);
	state.volume = { status: "running", checked_at: null, error: null };
	renderKeywords(20);
	expect(await screen.findByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument();
	state.volume = { status: "idle", checked_at: "2026-11-15T09:00:00Z", error: null };
	await vi.waitFor(() => expect(screen.queryByRole("progressbar")).toBeNull());
});

test("a refresh that takes too long stops being watched and says it keeps running", async () => {
	vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval"] });
	const state = mock([keyword()]);
	state.onVolumePost = () => {
		state.volume = { status: "running", checked_at: null, error: null };
		return undefined;
	};
	renderKeywords(60_000);
	await vi.waitFor(() => expect(screen.getByRole("button", { name: "Refresh volumes" })).toBeInTheDocument());
	fireEvent.click(screen.getByRole("button", { name: "Refresh volumes" }));
	await vi.waitFor(() => expect(screen.getByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument());
	await act(async () => {
		await vi.advanceTimersByTimeAsync(3 * 60 * 1000 + 100);
	});
	expect(screen.getByText(/taking longer than usual/i)).toBeInTheDocument();
	expect(screen.queryByRole("progressbar")).toBeNull();
	vi.useRealTimers();
});

test("Refresh volumes shows the server's refusal and a link to Settings", async () => {
	mock([keyword()]);
	server.use(http.post("/api/v1/sites/s1/keywords/volumes", () => fail(422, "Add your DataForSEO login in Settings first.")));
	renderApp("/sites/s1/keywords");
	await userEvent.click(await screen.findByRole("button", { name: "Refresh volumes" }));
	const alert = await screen.findByText(/Add your DataForSEO login in Settings first\./);
	expect(within(alert).getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
});

test("Find keyword ideas sends the seed and lists ideas with Add buttons that add them", async () => {
	const state = mock([keyword()], [idea(), idea({ keyword: "road maps", search_volume: 500 }), idea({ keyword: "old atlas", search_volume: null, competition: null, cpc: null })]);
	renderApp("/sites/s1/keywords");
	const panel = within(await screen.findByRole("region", { name: "Find keyword ideas" }));
	await userEvent.type(panel.getByLabelText("Seed keyword"), "  city maps ");
	await userEvent.click(panel.getByRole("button", { name: "Find ideas" }));
	const table = await panel.findByRole("table", { name: "Keyword ideas" });
	expect(state.seeds).toEqual(["city maps"]);
	const first = within(table).getByText("printed city maps").closest("tr") as HTMLElement;
	expect(first).toHaveTextContent("1,900");
	expect(first).toHaveTextContent("Low");
	expect(first).toHaveTextContent("$0.80");
	expect(within(table).getByText("road maps").closest("tr")).toHaveTextContent("Added");
	expect(within(table).queryByRole("button", { name: "Add road maps" })).toBeNull();

	await userEvent.click(within(table).getByRole("button", { name: "Add printed city maps" }));
	await vi.waitFor(() => expect(within(table).getByText("printed city maps").closest("tr")).toHaveTextContent("Added"));
	expect(within(table).queryByRole("button", { name: "Add printed city maps" })).toBeNull();
	expect(state.added).toEqual(["printed city maps"]);
	expect(within(await screen.findByRole("table", { name: "Target keywords" })).getByText("printed city maps")).toBeInTheDocument();
});

test("ideas need a seed, say when there are none, and show the server's refusals", async () => {
	const state = mock([keyword()], []);
	renderApp("/sites/s1/keywords");
	const panel = within(await screen.findByRole("region", { name: "Find keyword ideas" }));
	await userEvent.click(panel.getByRole("button", { name: "Find ideas" }));
	expect(await panel.findByText("Type a word or phrase to start from.")).toBeInTheDocument();
	expect(state.seeds).toHaveLength(0);

	await userEvent.type(panel.getByLabelText("Seed keyword"), "zzz");
	await userEvent.click(panel.getByRole("button", { name: "Find ideas" }));
	expect(await panel.findByText(/no ideas for that/i)).toBeInTheDocument();

	state.ideaReply = () => fail(429, "You have used all 20 idea searches for today.") as unknown as Response;
	await userEvent.click(panel.getByRole("button", { name: "Find ideas" }));
	expect(await panel.findByText("You have used all 20 idea searches for today.")).toBeInTheDocument();

	state.ideaReply = () => fail(422, "Add your DataForSEO login in Settings first.") as unknown as Response;
	await userEvent.click(panel.getByRole("button", { name: "Find ideas" }));
	const message = await panel.findByText(/Add your DataForSEO login/);
	expect(within(message).getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
});

test("the seed field stops at the 80 characters the server accepts, and a bad seed gets no Settings link", async () => {
	const state = mock([keyword()]);
	state.ideaReply = () => fail(422, "Enter a seed of 1 to 80 characters.") as unknown as Response;
	renderApp("/sites/s1/keywords");
	const panel = within(await screen.findByRole("region", { name: "Find keyword ideas" }));
	expect(panel.getByLabelText("Seed keyword")).toHaveAttribute("maxlength", "80");
	await userEvent.type(panel.getByLabelText("Seed keyword"), "maps");
	await userEvent.click(panel.getByRole("button", { name: "Find ideas" }));
	expect(await panel.findByText("Enter a seed of 1 to 80 characters.")).toBeInTheDocument();
	expect(panel.queryByRole("link", { name: "Settings" })).toBeNull();
});

test("adding an idea that the server refuses shows why and keeps the Add button", async () => {
	mock([], [idea()]);
	server.use(http.post("/api/v1/sites/s1/keywords", () => fail(422, "You can track up to 50 keywords.")));
	renderApp("/sites/s1/keywords");
	const panel = within(await screen.findByRole("region", { name: "Find keyword ideas" }));
	await userEvent.type(panel.getByLabelText("Seed keyword"), "maps");
	await userEvent.click(panel.getByRole("button", { name: "Find ideas" }));
	await userEvent.click(await panel.findByRole("button", { name: "Add printed city maps" }));
	expect(await panel.findByText("You can track up to 50 keywords.")).toBeInTheDocument();
	expect(panel.getByRole("button", { name: "Add printed city maps" })).toBeEnabled();
});
