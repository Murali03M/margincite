import { act, screen } from "@testing-library/react";
import { http } from "msw";
import { CrawlStatus } from "../../src/sites/CrawlStatus";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { crawl } from "../mocks/fixtures";
import { renderAt } from "../render";
import { setToken } from "../../src/auth/session";
import type { StopReason } from "../../src/api/types";

/** Lets timers and the promises they start settle, without real waiting. */
async function advance(ms: number) {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(ms);
	});
}

test("crawl_status_polls_until_the_crawl_ends_then_stops", async () => {
	vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval"] });
	const responses = [
		crawl({ status: "queued", pages_crawled: 0, score: null, stop_reason: null }),
		crawl({ status: "running", pages_crawled: 5, score: null, stop_reason: null }),
		crawl({ status: "completed", pages_crawled: 12 }),
	];
	let requests = 0;
	server.use(
		http.get("/api/v1/crawls/c1", () => {
			const next = responses[Math.min(requests, responses.length - 1)];
			requests += 1;
			return ok(next);
		}),
	);
	setToken("good-token");
	renderAt(<CrawlStatus crawlId="c1" />);

	await advance(10);
	expect(requests).toBe(1);
	expect(screen.getByText(/waiting to start/i)).toBeInTheDocument();

	await advance(2000);
	expect(requests).toBe(2);
	expect(screen.getByText(/5 pages crawled so far/i)).toBeInTheDocument();

	await advance(2000);
	expect(requests).toBe(3);
	expect(screen.getByText(/the crawl finished/i)).toBeInTheDocument();

	await advance(20000);
	expect(requests).toBe(3);
});

test("each_stop_reason_has_its_own_sentence", async () => {
	const expected: Record<StopReason, RegExp> = {
		finished: /the crawl finished/i,
		page_cap: /stopped at the page limit/i,
		robots_blocked: /robots\.txt blocks/i,
		site_unreachable: /could not be reached/i,
		time_limit: /stopped at the 30 minute limit\. the pages crawled so far are kept\./i,
	};
	setToken("good-token");
	for (const [reason, sentence] of Object.entries(expected) as [StopReason, RegExp][]) {
		server.use(http.get("/api/v1/crawls/c1", () => ok(crawl({ stop_reason: reason, pages_crawled: 3 }))));
		const { unmount } = renderAt(<CrawlStatus crawlId="c1" />);
		expect(await screen.findByText(sentence)).toBeInTheDocument();
		unmount();
	}
});

test("a failed crawl says it failed and shows the reason as text", async () => {
	server.use(
		http.get("/api/v1/crawls/c1", () =>
			ok(crawl({ status: "failed", stop_reason: null, score: null, error: "<b>boom</b>" })),
		),
	);
	setToken("good-token");
	renderAt(<CrawlStatus crawlId="c1" />);
	expect(await screen.findByText(/the crawl failed/i)).toBeInTheDocument();
	expect(screen.getByText("<b>boom</b>")).toBeInTheDocument();
});

test("one page is not pluralised", async () => {
	server.use(http.get("/api/v1/crawls/c1", () => ok(crawl({ pages_crawled: 1 }))));
	setToken("good-token");
	renderAt(<CrawlStatus crawlId="c1" />);
	expect(await screen.findByText(/1 page crawled\./i)).toBeInTheDocument();
});

test("a crawl that cannot be loaded shows the error with Retry", async () => {
	server.use(http.get("/api/v1/crawls/c1", () => fail(500, "The crawl could not be loaded.")));
	setToken("good-token");
	renderAt(<CrawlStatus crawlId="c1" />);
	expect(await screen.findByText("The crawl could not be loaded.")).toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
});
