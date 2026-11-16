import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { Route, Routes } from "react-router-dom";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { checkRow, site, speedPage, speedReport, summary } from "../mocks/fixtures";
import type { SpeedReport } from "../../src/api/types";
import { SpeedPanel } from "../../src/speed/SpeedPanel";
import { renderApp, renderAt } from "../render";

function mockSpeed(initial: SpeedReport) {
	const state = { data: initial, posts: 0, reply: null as null | (() => Response) };
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/sites/s1/checks", () => ok({ crawl_id: "c1", checks: [checkRow()] })),
		http.get("/api/v1/sites/s1/suggestions", () => ok([], { total: 0, page: 1, limit: 100 })),
		http.get("/api/v1/sites/s1/speed", () => ok(state.data)),
		http.post("/api/v1/sites/s1/speed/check", () => {
			state.posts += 1;
			return state.reply?.() ?? ok({ queued: true }, null, 202);
		}),
	);
	return state;
}

const renderPanel = (pollMs: number) =>
	renderAt(
		<Routes>
			<Route path="/sites/:id/checks" element={<SpeedPanel siteId="s1" pollMs={pollMs} />} />
		</Routes>,
		"/sites/s1/checks",
	);

test("page speed shows each page's score in words, the three Core Web Vitals and where the data comes from", async () => {
	mockSpeed(
		speedReport({
			pages: [
				speedPage({ url: "https://example.com/", performance: 95, lcp_ms: 1800, cls: 0.02, inp_ms: 120, tbt_ms: 90, field_data: true }),
				speedPage({ url: "https://example.com/shop", performance: 64, field_data: false }),
				speedPage({ url: "https://example.com/slow", performance: 31, lcp_ms: 6100 }),
				speedPage({ url: "javascript:alert(1)", performance: null, lcp_ms: null, cls: null, inp_ms: null, tbt_ms: null, error: "Google could not test this page." }),
			],
		}),
	);
	renderApp("/sites/s1/checks");
	const table = await screen.findByRole("table", { name: "Page speed on mobile" });
	const home = within(table).getByRole("link", { name: "https://example.com/" }).closest("tr") as HTMLElement;
	expect(home).toHaveTextContent("95");
	expect(home).toHaveTextContent("Good");
	expect(home).toHaveTextContent("1.8 s");
	expect(home).toHaveTextContent("0.02");
	expect(home).toHaveTextContent("120 ms");
	expect(home).toHaveTextContent("90 ms");
	expect(home).toHaveTextContent("Real visitors");
	const shop = within(table).getByRole("link", { name: "https://example.com/shop" }).closest("tr") as HTMLElement;
	expect(shop).toHaveTextContent("Needs work");
	expect(shop).toHaveTextContent("Lab test only");
	expect(within(table).getByRole("link", { name: "https://example.com/slow" }).closest("tr")).toHaveTextContent("Poor");
	const bad = within(table).getByText("javascript:alert(1)").closest("tr") as HTMLElement;
	expect(within(bad).queryByRole("link")).toBeNull();
	expect(bad).toHaveTextContent("Google could not test this page.");
	expect(bad).toHaveTextContent("No score");
	expect(bad).not.toHaveTextContent(/\b0\b/);
	expect(screen.getByText(/estimate/i)).toBeInTheDocument();
	expect(screen.getByText(/14 Nov 2026/)).toBeInTheDocument();
});

test("a site that was never checked says so and offers the check", async () => {
	mockSpeed({ checked_at: null, pages: [] });
	renderApp("/sites/s1/checks");
	expect(await screen.findByText(/not checked yet/i)).toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Check page speed" })).toBeEnabled();
});

test("a failed load says so and Retry loads again", async () => {
	mockSpeed(speedReport());
	let calls = 0;
	server.use(
		http.get("/api/v1/sites/s1/speed", () => {
			calls += 1;
			return calls === 1 ? fail(502, "Page speed could not be read.") : ok(speedReport());
		}),
	);
	renderApp("/sites/s1/checks");
	expect(await screen.findByText("Page speed could not be read.")).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("table", { name: "Page speed on mobile" })).toBeInTheDocument();
});

test("Check page speed starts a check, shows that it is working and shows the new scores when they arrive", async () => {
	const state = mockSpeed(speedReport());
	let reads = 0;
	server.use(
		http.get("/api/v1/sites/s1/speed", () => {
			reads += 1;
			return ok(state.posts > 0 && reads > 3 ? speedReport({ checked_at: "2026-11-15T09:00:00Z", pages: [speedPage({ performance: 88 })] }) : state.data);
		}),
	);
	renderPanel(20);
	await userEvent.click(await screen.findByRole("button", { name: "Check page speed" }));
	expect(await screen.findByRole("progressbar", { name: "Check in progress" })).toBeInTheDocument();
	expect(await screen.findByText(/15 Nov 2026/)).toBeInTheDocument();
	expect(screen.getByText("88")).toBeInTheDocument();
	expect(screen.queryByRole("progressbar")).toBeNull();
});

test("the daily limit message is shown", async () => {
	const state = mockSpeed(speedReport());
	state.reply = () => fail(429, "You have used all 5 speed checks for today.") as unknown as Response;
	renderApp("/sites/s1/checks");
	await userEvent.click(await screen.findByRole("button", { name: "Check page speed" }));
	expect(await screen.findByText("You have used all 5 speed checks for today.")).toBeInTheDocument();
});

test("opening the checks screen at #page-speed puts page speed above the checks", async () => {
	mockSpeed(speedReport());
	renderApp("/sites/s1/checks#page-speed");
	const speed = await screen.findByRole("region", { name: "Page speed" });
	const technical = await screen.findByRole("heading", { name: "Technical" });
	expect(speed.compareDocumentPosition(technical) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

test("without the #page-speed address the checks come first", async () => {
	mockSpeed(speedReport());
	renderApp("/sites/s1/checks");
	const speed = await screen.findByRole("region", { name: "Page speed" });
	const technical = await screen.findByRole("heading", { name: "Technical" });
	expect(technical.compareDocumentPosition(speed) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});
