import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { server } from "../mocks/server";
import { ok } from "../mocks/handlers";
import { site, summary } from "../mocks/fixtures";
import { renderApp } from "../render";
import { pageName } from "../../src/ui/shellRoute";

const shop = site({ id: "s2", host: "shop.test", root_url: "https://shop.test/", display_name: "shop.test" });

function mockSites() {
	server.use(
		http.get("/api/v1/sites", () => ok([site(), shop])),
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s2", () => ok(shop)),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/sites/s2/summary", () =>
			ok(summary({ site: shop, latest_crawl: null, score: null, counts: { open: 0, done: 0, dismissed: 0 } })),
		),
	);
}

const SECTIONS: [string, string][] = [
	["Overview", "/sites/s1"],
	["Suggestions", "/sites/s1/suggestions"],
	["Checks", "/sites/s1/checks"],
	["Keywords", "/sites/s1/keywords"],
	["Rankings", "/sites/s1/rankings"],
	["AI Visibility", "/sites/s1/ai"],
	["Search", "/sites/s1/search"],
	["Backlinks", "/sites/s1/backlinks"],
	["Pages", "/sites/s1/pages"],
	["Connections", "/sites/s1/connections"],
	["Site settings", "/sites/s1/settings"],
];

test("the sidebar lists every site with its score and open suggestions", async () => {
	mockSites();
	renderApp("/");
	const sites = await screen.findByRole("navigation", { name: "Sites" });
	const first = await within(sites).findByRole("link", { name: /^example\.com/ });
	expect(first).toHaveAttribute("href", "/sites/s1");
	expect(await within(first).findByText("Score 72 · 7 open")).toBeInTheDocument();
	const second = within(sites).getByRole("link", { name: /^shop\.test/ });
	expect(second).toHaveAttribute("href", "/sites/s2");
	expect(await within(second).findByText("No score · 0 open")).toBeInTheDocument();
	expect(within(sites).getByRole("link", { name: "Add site" })).toHaveAttribute("href", "/sites/new");
});

test("the sidebar highlights the current site and shows only its sections", async () => {
	mockSites();
	renderApp("/sites/s1/suggestions");
	const sites = await screen.findByRole("navigation", { name: "Sites" });
	const current = await within(sites).findByRole("link", { name: /^example\.com/ });
	expect(current).toHaveAttribute("aria-current", "true");
	expect(within(sites).getByRole("link", { name: /^shop\.test/ })).not.toHaveAttribute("aria-current");

	const sections = within(sites).getByRole("navigation", { name: "Site sections" });
	expect(within(sections).getAllByRole("link").map((link) => link.textContent)).toEqual(SECTIONS.map(([name]) => name));
	for (const [name, href] of SECTIONS) {
		expect(within(sections).getByRole("link", { name })).toHaveAttribute("href", href);
	}
	expect(within(sections).getByRole("link", { name: "Suggestions" })).toHaveAttribute("aria-current", "page");
	expect(within(sections).getByRole("link", { name: "Overview" })).not.toHaveAttribute("aria-current");
	expect(screen.getAllByRole("navigation", { name: "Site sections" })).toHaveLength(1);
});

test("the overview section is current on the site's own address", async () => {
	mockSites();
	renderApp("/sites/s2");
	const sections = await screen.findByRole("navigation", { name: "Site sections" });
	expect(within(sections).getByRole("link", { name: "Overview" })).toHaveAttribute("href", "/sites/s2");
	expect(within(sections).getByRole("link", { name: "Overview" })).toHaveAttribute("aria-current", "page");
});

test("a screen without a site shows no sections", async () => {
	mockSites();
	renderApp("/settings");
	await screen.findByRole("navigation", { name: "Sites" });
	expect(screen.queryByRole("navigation", { name: "Site sections" })).not.toBeInTheDocument();
});

test("the sidebar foot has settings, the signed-in email and sign out", async () => {
	mockSites();
	renderApp("/");
	const side = await screen.findByRole("complementary", { name: "Sidebar" });
	expect(within(side).getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
	expect(await within(side).findByText("owner@example.com")).toBeInTheDocument();
	await userEvent.click(within(side).getByRole("button", { name: "Sign out" }));
	expect(await screen.findByRole("button", { name: /sign in/i })).toBeInTheDocument();
});

test("the title bar names the site and the section, or just the page", async () => {
	mockSites();
	renderApp("/sites/s1/suggestions");
	const crumbs = await screen.findByRole("navigation", { name: "Breadcrumb" });
	expect(await within(crumbs).findByText("example.com")).toBeInTheDocument();
	expect(within(crumbs).getByText("Suggestions")).toHaveAttribute("aria-current", "page");
});

test("the title bar shows only the page name outside a site", async () => {
	mockSites();
	renderApp("/settings");
	const crumbs = await screen.findByRole("navigation", { name: "Breadcrumb" });
	expect(within(crumbs).getAllByRole("listitem")).toHaveLength(1);
	expect(within(crumbs).getByText("Settings")).toHaveAttribute("aria-current", "page");
});

test("the menu button opens the drawer, Escape closes it and focus returns to the button", async () => {
	mockSites();
	renderApp("/");
	const button = await screen.findByRole("button", { name: "Open menu" });
	const side = screen.getByRole("complementary", { name: "Sidebar" });
	expect(button).toHaveAttribute("aria-expanded", "false");
	expect(side).toHaveAttribute("data-open", "false");

	await userEvent.click(button);
	expect(button).toHaveAttribute("aria-expanded", "true");
	expect(side).toHaveAttribute("data-open", "true");
	expect(side.contains(document.activeElement)).toBe(true);

	await userEvent.keyboard("{Escape}");
	expect(side).toHaveAttribute("data-open", "false");
	expect(button).toHaveAttribute("aria-expanded", "false");
	expect(button).toHaveFocus();
});

test("the drawer closes when a link is followed and when the dimmed area is pressed", async () => {
	mockSites();
	renderApp("/sites/s1");
	const button = await screen.findByRole("button", { name: "Open menu" });
	const side = screen.getByRole("complementary", { name: "Sidebar" });

	await userEvent.click(button);
	await userEvent.click(within(side).getByRole("link", { name: "Checks" }));
	expect(side).toHaveAttribute("data-open", "false");
	expect(button).toHaveFocus();

	await userEvent.click(button);
	await userEvent.click(screen.getByTestId("scrim"));
	expect(side).toHaveAttribute("data-open", "false");
	expect(button).toHaveFocus();

	await userEvent.click(button);
	await userEvent.click(within(side).getByRole("button", { name: "Close menu" }));
	expect(side).toHaveAttribute("data-open", "false");
});

test("Tab stays inside the open drawer", async () => {
	mockSites();
	renderApp("/");
	await userEvent.click(await screen.findByRole("button", { name: "Open menu" }));
	const side = screen.getByRole("complementary", { name: "Sidebar" });
	for (let i = 0; i < 30; i++) {
		await userEvent.tab();
		expect(side.contains(document.activeElement)).toBe(true);
	}
	for (let i = 0; i < 30; i++) {
		await userEvent.tab({ shift: true });
		expect(side.contains(document.activeElement)).toBe(true);
	}
});

test("page names come from the address", () => {
	expect(pageName("/", "")).toEqual({ siteId: null, name: "Sites" });
	expect(pageName("/sites/new", "")).toEqual({ siteId: null, name: "Add site" });
	expect(pageName("/settings", "")).toEqual({ siteId: null, name: "Settings" });
	expect(pageName("/sites/s1", "")).toEqual({ siteId: "s1", name: "Overview" });
	expect(pageName("/sites/s1/settings", "")).toEqual({ siteId: "s1", name: "Site settings" });
	expect(pageName("/sites/s1/ai/answers/a1", "")).toEqual({ siteId: "s1", name: "AI Visibility" });
	expect(pageName("/suggestions/g1", "?site=s1")).toEqual({ siteId: "s1", name: "Suggestions" });
	expect(pageName("/suggestions/g1", "")).toEqual({ siteId: null, name: "Suggestions" });
	expect(pageName("/pages/p1", "")).toEqual({ siteId: null, name: "Page" });
	expect(pageName("/nowhere", "")).toEqual({ siteId: null, name: "Not found" });
});
