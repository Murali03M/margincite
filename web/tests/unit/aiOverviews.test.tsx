import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { aiOverviewRow, integrations, settings, settingsReady, site, summary } from "../mocks/fixtures";
import type { AiOverviewRow, Settings } from "../../src/api/types";
import { renderApp } from "../render";

function mock(rows: AiOverviewRow[], config: Settings = settingsReady()) {
	const state = { gets: 0 };
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/integrations", () => ok(integrations())),
		http.get("/api/v1/sites/s1/ai/runs", () => ok([], { total: 0, page: 1, limit: 10 })),
		http.get("/api/v1/sites/s1/ai/prompts", () => ok([])),
		http.get("/api/v1/sites/s1/ai/settings", () => ok({ brand_name: "", aliases: [], competitors: [] })),
		http.get("/api/v1/settings", () => ok(config)),
		http.get("/api/v1/sites/s1/ai-overviews", () => {
			state.gets += 1;
			return ok(rows);
		}),
	);
	return state;
}

const section = () => screen.findByRole("region", { name: "AI Overviews" });

test("without search data the section explains what it needs and does not ask for overviews", async () => {
	const state = mock([], settings());
	renderApp("/sites/s1/ai");
	const region = within(await section());
	expect(await region.findByRole("link", { name: "Open Settings" })).toHaveAttribute("href", "/settings");
	expect(state.gets).toBe(0);
});

test("each keyword's AI Overview status and the domains it cites are listed as text", async () => {
	mock([
		aiOverviewRow(),
		aiOverviewRow({ keyword_id: "k2", keyword: "atlas", status: "present", cited_domains: ["<b>evil</b>.example.net"], your_urls: ["javascript:alert(1)"] }),
		aiOverviewRow({ keyword_id: "k3", keyword: "globes", status: "absent", cited_domains: [], your_urls: [] }),
		aiOverviewRow({ keyword_id: "k4", keyword: "compass", status: "unknown", cited_domains: [], your_urls: [], checked_at: null }),
	]);
	renderApp("/sites/s1/ai");
	const table = await within(await section()).findByRole("table", { name: "AI Overviews by keyword" });
	const first = within(table).getByText("road maps").closest("tr") as HTMLElement;
	expect(first).toHaveTextContent("Cited");
	expect(first).toHaveTextContent("example.com, maps.example.org");
	expect(within(first).getByRole("link", { name: "https://example.com/maps" })).toHaveAttribute("rel", expect.stringContaining("noopener"));
	const second = within(table).getByText("atlas").closest("tr") as HTMLElement;
	expect(second).toHaveTextContent("AI Overview shown without you");
	expect(second).toHaveTextContent("<b>evil</b>.example.net");
	expect(second.querySelector("b")).toBeNull();
	expect(within(second).queryByRole("link")).toBeNull();
	expect(within(table).getByText("globes").closest("tr")).toHaveTextContent("No AI Overview");
	expect(within(table).getByText("compass").closest("tr")).toHaveTextContent("Not known");
});

test("no overviews checked yet says to check rankings", async () => {
	mock([]);
	renderApp("/sites/s1/ai");
	const region = within(await section());
	expect(await region.findByText(/not checked yet/i)).toBeInTheDocument();
	expect(region.getByRole("link", { name: "Open Rankings" })).toHaveAttribute("href", "/sites/s1/rankings");
});

test("an overviews failure says so and Retry loads again", async () => {
	mock([aiOverviewRow()]);
	let calls = 0;
	server.use(
		http.get("/api/v1/sites/s1/ai-overviews", () => {
			calls += 1;
			return calls === 1 ? fail(502, "AI Overviews could not be read.") : ok([aiOverviewRow()]);
		}),
	);
	renderApp("/sites/s1/ai");
	const region = within(await section());
	expect(await region.findByText("AI Overviews could not be read.")).toBeInTheDocument();
	await userEvent.click(region.getByRole("button", { name: "Retry" }));
	expect(await region.findByRole("table", { name: "AI Overviews by keyword" })).toBeInTheDocument();
});

test("a settings failure inside the section says so", async () => {
	mock([]);
	server.use(http.get("/api/v1/settings", () => fail(500, "Settings could not be read.")));
	renderApp("/sites/s1/ai");
	expect(await within(await section()).findByText("Settings could not be read.")).toBeInTheDocument();
});

test("AI Visibility lists Grok among the engines that are not set up", async () => {
	mock([]);
	renderApp("/sites/s1/ai");
	const list = await screen.findByRole("list", { name: "Engines not set up" });
	expect(within(list).getByText("Grok")).toBeInTheDocument();
});
