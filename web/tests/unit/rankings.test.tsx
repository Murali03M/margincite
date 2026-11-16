import { act, fireEvent, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { Route, Routes } from "react-router-dom";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { keyword, rankingKeyword, rankings, settings, settingsReady, site, summary } from "../mocks/fixtures";
import type { Rankings, Settings } from "../../src/api/types";
import { RankingsPage } from "../../src/rankings/RankingsPage";
import { PositionChart } from "../../src/rankings/PositionChart";
import { renderApp, renderAt } from "../render";

function mockRankings(initial: Rankings, config: Settings = settingsReady()) {
	const state = { data: initial, gets: 0, posts: 0, onPost: null as null | (() => Response | undefined) };
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/settings", () => ok(config)),
		http.get("/api/v1/sites/s1/keywords", () => ok([keyword({ id: "k1", search_volume: 2400 })])),
		http.get("/api/v1/sites/s1/rankings", () => {
			state.gets += 1;
			return ok(state.data);
		}),
		http.post("/api/v1/sites/s1/rankings/check", () => {
			state.posts += 1;
			return state.onPost?.() ?? ok({ queued: true }, null, 202);
		}),
	);
	return state;
}

const row = (name: string) => screen.getByText(name, { selector: "[data-keyword]" }).closest("tr") as HTMLElement;

test("without search data the screen explains what it needs and links to Settings, and asks for no rankings", async () => {
	const state = mockRankings(rankings(), settings());
	renderApp("/sites/s1/rankings");
	expect(await screen.findByRole("heading", { name: "Search data is not set up" })).toBeInTheDocument();
	expect(screen.getByRole("link", { name: "Open Settings" })).toHaveAttribute("href", "/settings");
	expect(screen.queryByRole("button", { name: "Check now" })).toBeNull();
	expect(state.gets).toBe(0);
});

test("a settings failure says so and Retry loads again", async () => {
	mockRankings(rankings());
	let calls = 0;
	server.use(
		http.get("/api/v1/settings", () => {
			calls += 1;
			return calls === 1 ? fail(500, "Settings could not be read.") : ok(settingsReady());
		}),
	);
	renderApp("/sites/s1/rankings");
	expect(await screen.findByText("Settings could not be read.")).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("table", { name: "Keyword rankings" })).toBeInTheDocument();
});

test("a rankings failure says so and Retry loads again", async () => {
	mockRankings(rankings());
	let calls = 0;
	server.use(
		http.get("/api/v1/sites/s1/rankings", () => {
			calls += 1;
			return calls === 1 ? fail(502, "Rankings could not be read.") : ok(rankings());
		}),
	);
	renderApp("/sites/s1/rankings");
	expect(await screen.findByText("Rankings could not be read.")).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("table", { name: "Keyword rankings" })).toBeInTheDocument();
});

test("with no keywords the screen says to add some", async () => {
	mockRankings(rankings({ keywords: [], checked_at: null }));
	renderApp("/sites/s1/rankings");
	expect(await screen.findByRole("heading", { name: "No keywords to track yet" })).toBeInTheDocument();
	expect(screen.getByRole("link", { name: "Add keywords" })).toHaveAttribute("href", "/sites/s1/keywords");
});

test("each keyword shows its position, the change in words, where it lands, its AI Overview and its volume", async () => {
	mockRankings(
		rankings({
			keywords: [
				rankingKeyword({ keyword_id: "k1", keyword: "road maps", position: 4, previous: 7, ai_overview: "cited" }),
				rankingKeyword({ keyword_id: "k2", keyword: "atlas", position: 12, previous: 9, url: "javascript:alert(1)", ai_overview: "present" }),
				rankingKeyword({ keyword_id: "k3", keyword: "globes", position: null, previous: 30, url: null, ai_overview: "absent" }),
				rankingKeyword({ keyword_id: "k4", keyword: "compass", position: null, previous: null, url: null, ai_overview: "unknown" }),
				rankingKeyword({ keyword_id: "k5", keyword: "city guides", position: 2, previous: 2 }),
				rankingKeyword({ keyword_id: "k6", keyword: "tide tables", position: 55, previous: null, history: [{ at: "2026-11-14T08:00:00Z", position: 55 }] }),
				rankingKeyword({ keyword_id: "k7", keyword: "fresh keyword", position: null, previous: null, top: [], history: [] }),
			],
		}),
	);
	renderApp("/sites/s1/rankings");
	await screen.findByRole("table", { name: "Keyword rankings" });

	expect(row("road maps")).toHaveTextContent("4");
	expect(row("road maps")).toHaveTextContent("Up 3 places");
	expect(row("road maps")).toHaveTextContent("Cited");
	expect(row("road maps")).toHaveTextContent("2,400");
	expect(within(row("road maps")).getByRole("link", { name: "https://example.com/maps" })).toHaveAttribute("rel", expect.stringContaining("noopener"));
	expect(row("atlas")).toHaveTextContent("Down 3 places");
	expect(within(row("atlas")).queryByRole("link", { name: /javascript/ })).toBeNull();
	expect(row("atlas")).toHaveTextContent("javascript:alert(1)");
	expect(row("atlas")).toHaveTextContent("AI Overview shown without you");
	expect(row("globes")).toHaveTextContent("Not in the top 100");
	expect(row("globes")).toHaveTextContent("Dropped out of the top 100");
	expect(row("globes")).not.toHaveTextContent(/\b0\b/);
	expect(row("globes")).toHaveTextContent("No AI Overview");
	expect(row("compass")).toHaveTextContent("Not in the top 100");
	expect(row("compass")).toHaveTextContent("Not known");
	expect(row("city guides")).toHaveTextContent("No change");
	expect(row("tide tables")).toHaveTextContent("First check");
	expect(row("fresh keyword")).toHaveTextContent("Not checked yet");
	expect(row("fresh keyword")).not.toHaveTextContent("Not in the top 100");
});

test("the location and date of the check are stated, and positions are called a snapshot", async () => {
	mockRankings(rankings({ location: { country: "GB", language: "en", device: "mobile" } }));
	renderApp("/sites/s1/rankings");
	await screen.findByRole("table", { name: "Keyword rankings" });
	expect(screen.getByText(/GB, en, mobile/)).toBeInTheDocument();
	expect(screen.getByText(/14 Nov 2026/)).toBeInTheDocument();
	expect(screen.getByText(/snapshot/i)).toBeInTheDocument();
});

test("opening a row shows the top 10 as text with your site marked, and the position history", async () => {
	mockRankings(
		rankings({
			keywords: [
				rankingKeyword({
					top: [
						{ position: 1, domain: "maps.example.org", url: "https://maps.example.org/", title: "<b>Best</b> road maps" },
						{ position: 2, domain: "bad.example.net", url: "javascript:alert(1)", title: "Odd result" },
						{ position: 4, domain: "www.example.com", url: "https://example.com/maps", title: "Road maps from Example" },
					],
				}),
			],
		}),
	);
	renderApp("/sites/s1/rankings");
	await userEvent.click(await screen.findByRole("button", { name: "Show top 10 for road maps" }));
	const table = await screen.findByRole("table", { name: "Top 10 results for road maps" });
	expect(within(table).getByText("<b>Best</b> road maps")).toBeInTheDocument();
	expect(table.querySelector("b")).toBeNull();
	expect(within(table).queryByRole("link", { name: /javascript/ })).toBeNull();
	expect(within(table).getByText("www.example.com").closest("tr")).toHaveTextContent("You");
	expect(within(table).getByText("maps.example.org").closest("tr")).not.toHaveTextContent("You");
	expect(within(table).getByRole("link", { name: "https://maps.example.org/" })).toHaveAttribute("rel", expect.stringContaining("nofollow"));
	const chart = screen.getByRole("img", { name: /position history/i });
	expect(chart.querySelectorAll("circle")).toHaveLength(3);
	await userEvent.click(screen.getByRole("button", { name: "Hide top 10 for road maps" }));
	expect(screen.queryByRole("table", { name: "Top 10 results for road maps" })).toBeNull();
});

test("a keyword with no saved results says so in the detail", async () => {
	mockRankings(rankings({ keywords: [rankingKeyword({ top: [], history: [] })] }));
	renderApp("/sites/s1/rankings");
	await userEvent.click(await screen.findByRole("button", { name: "Show top 10 for road maps" }));
	expect(await screen.findByText(/no results saved/i)).toBeInTheDocument();
	expect(screen.getByText(/no history yet/i)).toBeInTheDocument();
});

test("the position chart draws gaps for checks where the site was not in the top 100", () => {
	const { container } = renderAt(
		<PositionChart
			keyword="road maps"
			history={[
				{ at: "2026-11-01T08:00:00Z", position: 20 },
				{ at: "2026-11-08T08:00:00Z", position: null },
				{ at: "2026-11-14T08:00:00Z", position: 5 },
			]}
		/>,
	);
	const chart = screen.getByRole("img", { name: /position history for road maps/i });
	expect(chart).toHaveAccessibleName(/not in the top 100/);
	expect(container.querySelectorAll("circle")).toHaveLength(2);
	expect(container.querySelectorAll("[data-gap]")).toHaveLength(1);
	expect(container.querySelectorAll("path[data-line]")).toHaveLength(2);
});

function renderRankings(pollMs: number) {
	return renderAt(
		<Routes>
			<Route path="/sites/:id/rankings" element={<RankingsPage pollMs={pollMs} />} />
		</Routes>,
		"/sites/s1/rankings",
	);
}

test("Check now starts a check, shows that it is working, and shows the new numbers when the check date changes", async () => {
	const state = mockRankings(rankings());
	state.onPost = () => {
		state.data = rankings({ checked_at: "2026-11-15T08:00:00Z", keywords: [rankingKeyword({ position: 3, previous: 4 })] });
		return undefined;
	};
	// The new data appears only after two more reads, as a running job would.
	let reads = 0;
	server.use(
		http.get("/api/v1/sites/s1/rankings", () => {
			reads += 1;
			return ok(reads <= 2 && state.posts > 0 ? rankings() : state.data);
		}),
	);
	renderRankings(20);
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	expect(await screen.findByRole("status")).toHaveTextContent(/checking your keywords/i);
	expect(screen.getByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument();
	expect(await screen.findByText(/15 Nov 2026/)).toBeInTheDocument();
	expect(within(row("road maps")).getByText("Up 1 place")).toBeInTheDocument();
	expect(screen.queryByRole("progressbar", { name: "Check in progress" })).toBeNull();
	expect(state.posts).toBe(1);
});

test("Check now shows the server's refusal, with a link to Settings when credentials are missing", async () => {
	const state = mockRankings(rankings());
	state.onPost = () => fail(422, "Add your DataForSEO login in Settings first.");
	renderApp("/sites/s1/rankings");
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	expect(await screen.findByText("Add your DataForSEO login in Settings first.")).toBeInTheDocument();
	expect(screen.getAllByRole("link", { name: "Settings" }).some((link) => link.getAttribute("href") === "/settings")).toBe(true);
	expect(screen.queryByRole("progressbar")).toBeNull();
});

test("a 422 that does not mention Settings has no Settings link", async () => {
	const state = mockRankings(rankings());
	state.onPost = () => fail(422, "Add at least one keyword before checking rankings.");
	renderApp("/sites/s1/rankings");
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	const alert = await screen.findByRole("alert");
	expect(alert).toHaveTextContent("Add at least one keyword before checking rankings.");
	expect(within(alert).queryByRole("link", { name: "Settings" })).toBeNull();
});

const RUN_BEFORE = { id: "r1", status: "completed" as const, error: null, finished_at: "2026-11-14T08:00:00Z" };

test("a run that failed ends the watch at once and its error is shown as the reason", async () => {
	const state = mockRankings(rankings({ last_run: RUN_BEFORE }));
	state.onPost = () => {
		state.data = rankings({ last_run: { id: "r2", status: "failed", error: "DataForSEO says the account is out of credit.", finished_at: "2026-11-15T08:00:00Z" } });
		return undefined;
	};
	renderRankings(20);
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	expect(await screen.findByText(/DataForSEO says the account is out of credit\./)).toBeInTheDocument();
	expect(screen.queryByRole("progressbar", { name: "Check in progress" })).toBeNull();
	expect(screen.getByRole("button", { name: "Check now" })).toBeEnabled();
	expect(screen.queryByText(/taking longer than usual/i)).toBeNull();
});

test("the run that was there before the click does not end the watch, a queued new run does not either, a finished new one does", async () => {
	const state = mockRankings(rankings({ last_run: RUN_BEFORE }));
	renderRankings(20);
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	await vi.waitFor(() => expect(state.gets).toBeGreaterThanOrEqual(4));
	expect(screen.getByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument();

	state.data = rankings({ last_run: { id: "r2", status: "running", error: null, finished_at: null } });
	const seen = state.gets;
	await vi.waitFor(() => expect(state.gets).toBeGreaterThanOrEqual(seen + 3));
	expect(screen.getByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument();

	state.data = rankings({ last_run: { id: "r2", status: "completed", error: null, finished_at: "2026-11-15T08:01:00Z" } });
	await vi.waitFor(() => expect(screen.queryByRole("progressbar", { name: "Check in progress" })).toBeNull());
	expect(screen.getByRole("button", { name: "Check now" })).toBeEnabled();
	expect(screen.queryByRole("alert")).toBeNull();
});

test("a finished run with a note shows the note without calling it a stop", async () => {
	const state = mockRankings(rankings({ last_run: RUN_BEFORE }));
	state.onPost = () => {
		state.data = rankings({ last_run: { id: "r2", status: "completed", error: "1 of 2 keywords could not be checked.", finished_at: "2026-11-15T08:00:00Z" } });
		return undefined;
	};
	renderRankings(20);
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	expect(await screen.findByText("The last check reported: 1 of 2 keywords could not be checked.")).toBeInTheDocument();
	expect(screen.queryByText(/stopped/i)).toBeNull();
});

test("without last_run an older backend is still followed by the check date", async () => {
	const state = mockRankings(rankings());
	state.onPost = () => {
		state.data = rankings({ checked_at: "2026-11-15T08:00:00Z" });
		return undefined;
	};
	renderRankings(20);
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	expect(await screen.findByText(/15 Nov 2026/)).toBeInTheDocument();
	expect(screen.queryByRole("progressbar", { name: "Check in progress" })).toBeNull();
});

test("Check now says when the daily limit is reached", async () => {
	const state = mockRankings(rankings());
	state.onPost = () => fail(429, "You have used all 5 manual checks for today.");
	renderApp("/sites/s1/rankings");
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	expect(await screen.findByText("You have used all 5 manual checks for today.")).toBeInTheDocument();
});

test("a check that is already running is followed instead of refused", async () => {
	const state = mockRankings(rankings());
	state.onPost = () => fail(409, "A check is already queued or running.");
	renderApp("/sites/s1/rankings");
	await userEvent.click(await screen.findByRole("button", { name: "Check now" }));
	expect(await screen.findByText("A check is already queued or running.")).toBeInTheDocument();
	expect(screen.getByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument();
});

test("a check that takes too long stops being watched and says it is still going", async () => {
	vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval"] });
	const advance = (ms: number) =>
		act(async () => {
			await vi.advanceTimersByTimeAsync(ms);
		});
	mockRankings(rankings());
	renderRankings(60_000);
	await vi.waitFor(() => expect(screen.getByRole("button", { name: "Check now" })).toBeInTheDocument());
	fireEvent.click(screen.getByRole("button", { name: "Check now" }));
	await vi.waitFor(() => expect(screen.getByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument());
	await advance(3 * 60 * 1000 + 100);
	expect(screen.getByText(/taking longer than usual/i)).toBeInTheDocument();
	expect(screen.queryByRole("progressbar")).toBeNull();
});
