import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { site, siteSettings, summary } from "../mocks/fixtures";
import type { SiteSettings } from "../../src/api/types";
import { renderApp } from "../render";

function mockSite(initial: SiteSettings = siteSettings()) {
	const state = { data: initial, puts: [] as unknown[] };
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/sites/s1/settings", () => ok(state.data)),
		http.put("/api/v1/sites/s1/settings", async ({ request }) => {
			const body = (await request.json()) as Partial<SiteSettings>;
			state.puts.push(body);
			state.data = { ...state.data, ...body };
			return ok(state.data);
		}),
	);
	return state;
}

const form = () => screen.getByRole("form", { name: "Site options" });

test("the site options are prefilled, with the countries and languages the server offers", async () => {
	mockSite(siteSettings({ display_name: "My shop", page_cap: 500, country: "GB", device: "mobile", track_rankings: true, ranking_frequency: "daily" }));
	renderApp("/sites/s1/settings");
	await screen.findByRole("form", { name: "Site options" });
	const f = within(form());
	expect(f.getByLabelText("Name")).toHaveValue("My shop");
	expect(f.getByLabelText("Page limit")).toHaveValue(500);
	expect(f.getByLabelText("Country")).toHaveValue("GB");
	expect(within(f.getByLabelText("Country")).getByRole("option", { name: "United Kingdom" })).toBeInTheDocument();
	expect(within(f.getByLabelText("Language")).getByRole("option", { name: "German" })).toBeInTheDocument();
	expect(f.getByLabelText("Device")).toHaveValue("mobile");
	expect(f.getByLabelText("Track rankings")).toBeChecked();
	expect(f.getByLabelText("How often")).toHaveValue("daily");
	expect(f.getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
	expect(f.getByText(/new ranking history/i)).toBeInTheDocument();
});

test("saving sends every option and says it was saved", async () => {
	const state = mockSite();
	renderApp("/sites/s1/settings");
	await screen.findByRole("form", { name: "Site options" });
	const f = within(form());
	await userEvent.clear(f.getByLabelText("Name"));
	await userEvent.type(f.getByLabelText("Name"), "Example shop");
	await userEvent.clear(f.getByLabelText("Page limit"));
	await userEvent.type(f.getByLabelText("Page limit"), "300");
	await userEvent.selectOptions(f.getByLabelText("Country"), "GB");
	await userEvent.selectOptions(f.getByLabelText("Language"), "de");
	await userEvent.selectOptions(f.getByLabelText("Device"), "mobile");
	expect(f.getByLabelText("How often")).toBeDisabled();
	await userEvent.click(f.getByLabelText("Track rankings"));
	await userEvent.selectOptions(f.getByLabelText("How often"), "daily");
	await userEvent.click(f.getByRole("button", { name: "Save options" }));
	expect(await f.findByText("Saved")).toBeInTheDocument();
	expect(state.puts).toEqual([
		{ display_name: "Example shop", page_cap: 300, country: "GB", language: "de", device: "mobile", track_rankings: true, ranking_frequency: "daily" },
	]);
});

test("a page limit outside 10 to 5000 is refused on screen without a request", async () => {
	const state = mockSite();
	renderApp("/sites/s1/settings");
	await screen.findByRole("form", { name: "Site options" });
	const f = within(form());
	for (const bad of ["9", "5001", "", "12.5"]) {
		await userEvent.clear(f.getByLabelText("Page limit"));
		if (bad) await userEvent.type(f.getByLabelText("Page limit"), bad);
		await userEvent.click(f.getByRole("button", { name: "Save options" }));
		expect(await f.findByText("Page limit must be a whole number from 10 to 5000.")).toBeInTheDocument();
	}
	expect(state.puts).toHaveLength(0);
});

test("the server's validation message is shown and the typed values stay", async () => {
	mockSite();
	server.use(http.put("/api/v1/sites/s1/settings", () => fail(422, "That country is not one Margincite can track.")));
	renderApp("/sites/s1/settings");
	await screen.findByRole("form", { name: "Site options" });
	const f = within(form());
	await userEvent.clear(f.getByLabelText("Name"));
	await userEvent.type(f.getByLabelText("Name"), "Kept name");
	await userEvent.click(f.getByRole("button", { name: "Save options" }));
	expect(await f.findByText("That country is not one Margincite can track.")).toBeInTheDocument();
	expect(f.getByLabelText("Name")).toHaveValue("Kept name");
	expect(f.queryByText("Saved")).toBeNull();
});

test("site options that fail to load say so and Retry loads them", async () => {
	let calls = 0;
	mockSite();
	server.use(
		http.get("/api/v1/sites/s1/settings", () => {
			calls += 1;
			return calls === 1 ? fail(500, "The options could not be read.") : ok(siteSettings());
		}),
	);
	renderApp("/sites/s1/settings");
	expect(await screen.findByText("The options could not be read.")).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("form", { name: "Site options" })).toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Delete site" })).toBeInTheDocument();
});
