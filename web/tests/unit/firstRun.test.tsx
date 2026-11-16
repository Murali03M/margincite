import { screen, within } from "@testing-library/react";
import { http } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { crawl, keyword, providerOut, settings, settingsReady, site, summary } from "../mocks/fixtures";
import type { Keyword, Settings } from "../../src/api/types";
import { renderApp } from "../render";

function mock({ config, sites = [], keywords = [] }: { config: Settings; sites?: ReturnType<typeof site>[]; keywords?: Keyword[] }) {
	server.use(
		http.get("/api/v1/settings", () => ok(config)),
		http.get("/api/v1/sites", () => ok(sites)),
		http.get("/api/v1/sites/s1/summary", () => ok(summary({ latest_crawl: crawl() }))),
		http.get("/api/v1/sites/s1/keywords", () => ok(keywords)),
	);
}

const guide = () => screen.findByRole("region", { name: "Get set up" });
const item = (name: string) => screen.getByText(name).closest("li") as HTMLElement;

test("with nothing set up the guide lists every step as to do, with a link for each, above the empty state", async () => {
	mock({ config: settings() });
	renderApp("/");
	const region = within(await guide());
	expect(region.getByText("0 of 4 steps done")).toBeInTheDocument();
	for (const step of ["Add a crawler contact", "Add your website", "Add keywords", "Connect an AI provider", "Connect search data"]) {
		expect(within(item(step)).getByText("To do")).toBeInTheDocument();
	}
	expect(within(item("Add a crawler contact")).getByRole("link")).toHaveAttribute("href", "/settings");
	expect(within(item("Add your website")).getByRole("link")).toHaveAttribute("href", "/sites/new");
	expect(within(item("Connect an AI provider")).getByRole("link")).toHaveAttribute("href", "/settings");
	expect(within(item("Connect search data")).getByRole("link")).toHaveAttribute("href", "/settings");
	expect(within(item("Connect search data")).getByText(/optional/i)).toBeInTheDocument();
	expect(screen.getByRole("heading", { name: "Add your first site" })).toBeInTheDocument();
});

test("steps tick off as they are done", async () => {
	mock({
		config: settings({
			crawler: { contact: "you@example.com", contact_source: "workspace" },
			providers: [providerOut("claude", { key_set: true, source: "workspace", status: "ok" }), ...settings().providers.slice(1)],
		}),
		sites: [site()],
		keywords: [],
	});
	renderApp("/");
	const region = within(await guide());
	expect(await region.findByText("3 of 4 steps done")).toBeInTheDocument();
	expect(within(item("Add a crawler contact")).getByText("Done")).toBeInTheDocument();
	expect(within(item("Add your website")).getByText("Done")).toBeInTheDocument();
	expect(within(item("Connect an AI provider")).getByText("Done")).toBeInTheDocument();
	expect(within(item("Add keywords")).getByText("To do")).toBeInTheDocument();
	expect(within(item("Add keywords")).getByRole("link")).toHaveAttribute("href", "/sites/s1/keywords");
	expect(within(item("Connect search data")).getByText("To do")).toBeInTheDocument();
	expect(screen.getByRole("link", { name: "example.com" })).toBeInTheDocument();
});

test("once the four required steps are done the guide goes away, even without search data", async () => {
	mock({ config: { ...settingsReady(), dataforseo: settings().dataforseo }, sites: [site()], keywords: [keyword()] });
	renderApp("/");
	await screen.findByRole("link", { name: "example.com" });
	await screen.findByRole("list", { name: /Project card/ });
	expect(screen.queryByRole("region", { name: "Get set up" })).toBeNull();
});

test("if the settings cannot be read the guide is left out and the page still works", async () => {
	mock({ config: settings() });
	server.use(http.get("/api/v1/settings", () => fail(500, "Settings could not be read.")));
	renderApp("/");
	expect(await screen.findByRole("heading", { name: "Add your first site" })).toBeInTheDocument();
	expect(screen.queryByRole("region", { name: "Get set up" })).toBeNull();
});
