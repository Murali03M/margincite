import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { Route, Routes } from "react-router-dom";
import { AddSitePage } from "../../src/sites/AddSitePage";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { emptyMetrics, site, summary } from "../mocks/fixtures";
import { renderApp, renderAt } from "../render";
import { setToken } from "../../src/auth/session";

const two = [site(), site({ id: "s2", host: "shop.test", root_url: "https://shop.test/", display_name: "shop.test" })];

function mockSites() {
	server.use(
		http.get("/api/v1/sites", () => ok(two)),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/sites/s2/summary", () =>
			ok(
				summary({
					site: two[1],
					latest_crawl: null,
					score: null,
					counts: { open: 0, done: 0, dismissed: 0 },
				}),
			),
		),
	);
}

test("the_sites_list_shows_a_project_card_for_each_site", async () => {
	mockSites();
	server.use(
		http.get("/api/v1/sites/s2/metrics", () => ok(emptyMetrics())),
	);
	renderApp("/");
	const first = (await screen.findByRole("link", { name: "example.com" })).closest("article") as HTMLElement;
	expect(screen.getByRole("link", { name: "example.com" })).toHaveAttribute("href", "/sites/s1");
	expect(await within(first).findByText("7 open suggestions")).toBeInTheDocument();
	expect(within(first).getByText("Finished")).toBeInTheDocument();
	const card = await within(first).findByRole("list", { name: "Project card, example.com" });
	expect(within(card).getAllByRole("listitem")).toHaveLength(9);
	expect(within(card).getByText("72")).toBeInTheDocument();

	const second = screen.getByRole("link", { name: "shop.test" }).closest("article") as HTMLElement;
	expect(await within(second).findByText("0 open suggestions")).toBeInTheDocument();
	expect(within(second).getByText("Not crawled yet")).toBeInTheDocument();
	const empty = await within(second).findByRole("list", { name: "Project card, shop.test" });
	expect(within(empty).getAllByText("n/a")).toHaveLength(9);
});

test("an_empty_account_is_invited_to_add_a_site", async () => {
	server.use(http.get("/api/v1/sites", () => ok([])));
	renderApp("/");
	expect(await screen.findByRole("heading", { name: /add your first site/i })).toBeInTheDocument();
	expect(screen.getAllByRole("link", { name: /add a site/i })[0]).toHaveAttribute("href", "/sites/new");
});

test("the_list_shows_an_error_with_retry_when_the_backend_is_down", async () => {
	server.use(http.get("/api/v1/sites", () => HttpResponse.error()));
	renderApp("/");
	expect(await screen.findByText("Margincite can't be reached. Check that the backend is running.")).toBeInTheDocument();
	mockSites();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("link", { name: "example.com" })).toBeInTheDocument();
});

test("a_site_whose_summary_fails_still_has_its_row", async () => {
	server.use(
		http.get("/api/v1/sites", () => ok([site()])),
		http.get("/api/v1/sites/s1/summary", () => fail(500, "Something broke.")),
	);
	renderApp("/");
	expect(await screen.findByRole("link", { name: "example.com" })).toBeInTheDocument();
	expect((await screen.findAllByText("Not loaded")).length).toBeGreaterThan(0);
});

function renderAddSite() {
	setToken("good-token");
	return renderAt(
		<Routes>
			<Route path="/sites/new" element={<AddSitePage />} />
			<Route path="/sites/:id" element={<p>The site page</p>} />
			<Route path="/" element={<p>The sites list</p>} />
		</Routes>,
		"/sites/new",
	);
}

test("adding_a_site_goes_to_its_page", async () => {
	let body: unknown = null;
	server.use(
		http.post("/api/v1/sites", async ({ request }) => {
			body = await request.json();
			return ok({ ...site({ id: "s9" }), first_crawl_id: "c9" }, null, 201);
		}),
	);
	renderAddSite();
	await userEvent.type(screen.getByLabelText("Website address"), "example.com");
	await userEvent.click(screen.getByRole("button", { name: /add site/i }));
	expect(await screen.findByText("The site page")).toBeInTheDocument();
	expect(body).toEqual({ url: "example.com" });
});

test("an_invalid_address_shows_the_reason_and_keeps_the_input", async () => {
	server.use(http.post("/api/v1/sites", () => fail(422, "That address is not a public website.")));
	renderAddSite();
	await userEvent.type(screen.getByLabelText("Website address"), "http://localhost");
	await userEvent.click(screen.getByRole("button", { name: /add site/i }));
	expect(await screen.findByText("That address is not a public website.")).toBeInTheDocument();
	expect(screen.getByLabelText("Website address")).toHaveValue("http://localhost");
	expect(screen.getByLabelText("Website address")).toHaveAttribute("aria-invalid", "true");
});

test("an_already_added_site_links_to_the_existing_one", async () => {
	server.use(
		http.post("/api/v1/sites", () => fail(409, "That site is already added.")),
		http.get("/api/v1/sites", () => ok(two)),
	);
	renderAddSite();
	await userEvent.type(screen.getByLabelText("Website address"), "https://www.Example.com/");
	await userEvent.click(screen.getByRole("button", { name: /add site/i }));
	expect(await screen.findByText(/already added/i)).toBeInTheDocument();
	expect(await screen.findByRole("link", { name: /open example\.com/i })).toHaveAttribute("href", "/sites/s1");
});

test("an empty address is refused without asking the server", async () => {
	renderAddSite();
	await userEvent.click(screen.getByRole("button", { name: /add site/i }));
	expect(await screen.findByText("Enter the address of your website.")).toBeInTheDocument();
});

test("an unexpected failure while adding is shown and the input is kept", async () => {
	server.use(http.post("/api/v1/sites", () => HttpResponse.error()));
	renderAddSite();
	await userEvent.type(screen.getByLabelText("Website address"), "example.com");
	await userEvent.click(screen.getByRole("button", { name: /add site/i }));
	expect(await screen.findByText(/can't be reached/i)).toBeInTheDocument();
	expect(screen.getByLabelText("Website address")).toHaveValue("example.com");
});

