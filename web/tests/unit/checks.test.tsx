import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { checkRow, site, suggestion, summary } from "../mocks/fixtures";
import { renderApp } from "../render";

function mockChecks(checks = defaultChecks, crawl_id: string | null = "c1") {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/sites/s1/checks", () => ok({ crawl_id, checks })),
		http.get("/api/v1/sites/s1/suggestions", () => ok([suggestion({ id: "g7", check_id: "meta_missing" })], { total: 1, page: 1, limit: 100 })),
	);
}

const defaultChecks = [
	checkRow(),
	checkRow({ check_id: "meta_missing", title: "Every page has a description", status: "failed", open: 3 }),
	checkRow({ check_id: "llms", title: "AI crawlers are welcome", category: "ai_readiness" }),
	checkRow({
		check_id: "search_low_ctr",
		title: "Pages get clicks for their impressions",
		category: "content",
		status: "not_run",
		reason: "Connect Google Search Console to run this check.",
	}),
];

test("checks are grouped by category with passed, failed and not run", async () => {
	mockChecks();
	renderApp("/sites/s1/checks");
	expect(await screen.findByRole("heading", { name: "Technical" })).toBeInTheDocument();
	expect(screen.getByRole("heading", { name: "AI readiness" })).toBeInTheDocument();
	expect(screen.getByRole("heading", { name: "Content" })).toBeInTheDocument();

	const summaryLine = screen.getByRole("list", { name: "Check totals" });
	expect(within(summaryLine).getByText("Passed").nextSibling).toHaveTextContent("2");
	expect(within(summaryLine).getByText("Failed").nextSibling).toHaveTextContent("1");
	expect(within(summaryLine).getByText("Not run").nextSibling).toHaveTextContent("1");

	const technical = screen.getByRole("list", { name: "Technical checks" });
	const passed = within(technical).getByText("Every page has a title").closest("li") as HTMLElement;
	expect(passed).toHaveTextContent("Passed");
	const failed = within(technical).getByText("Every page has a description").closest("li") as HTMLElement;
	expect(failed).toHaveTextContent("Failed");
	expect(within(failed).getByRole("link", { name: "3 open" })).toHaveAttribute("href", "/suggestions/g7?site=s1");

	const content = screen.getByRole("list", { name: "Content checks" });
	const notRun = within(content).getByText("Pages get clicks for their impressions").closest("li") as HTMLElement;
	expect(notRun).toHaveTextContent("Not run");
	expect(notRun).toHaveTextContent("Connect Google Search Console to run this check.");
});

test("a failed check without a matching suggestion still shows its open count as text", async () => {
	mockChecks([checkRow({ check_id: "zzz", status: "failed", open: 2, title: "Something" })]);
	renderApp("/sites/s1/checks");
	const row = (await screen.findByText("Something")).closest("li") as HTMLElement;
	expect(within(row).queryByRole("link")).toBeNull();
	expect(row).toHaveTextContent("2 open");
});

test("before the first crawl the checks are listed as not run with a note", async () => {
	mockChecks([checkRow({ status: "not_run", reason: "Crawl the site first." })], null);
	renderApp("/sites/s1/checks");
	expect(await screen.findByText(/appear once a crawl has finished/i)).toBeInTheDocument();
	expect(screen.getByText("Crawl the site first.")).toBeInTheDocument();
});

test("an empty list says so", async () => {
	mockChecks([]);
	renderApp("/sites/s1/checks");
	expect(await screen.findByText(/no checks to show/i)).toBeInTheDocument();
});

test("checks show an error with retry", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/checks", () => HttpResponse.error()),
		http.get("/api/v1/sites/s1/suggestions", () => ok([], { total: 0, page: 1, limit: 100 })),
	);
	renderApp("/sites/s1/checks");
	expect(await screen.findByText(/can't be reached/i)).toBeInTheDocument();
	mockChecks();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("heading", { name: "Technical" })).toBeInTheDocument();
});

test("checks show a loading state first", async () => {
	mockChecks();
	renderApp("/sites/s1/checks");
	expect(await screen.findByText("Loading the checks…")).toBeInTheDocument();
});

test("a server message is shown as written", async () => {
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/checks", () => fail(500, "The checks could not be listed.")),
		http.get("/api/v1/sites/s1/suggestions", () => ok([], { total: 0, page: 1, limit: 100 })),
	);
	renderApp("/sites/s1/checks");
	expect(await screen.findByText("The checks could not be listed.")).toBeInTheDocument();
});
