import type { Crawl, StopReason } from "../api/types";

/** One plain sentence per way a crawl can end. */
export const STOP_SENTENCES: Record<StopReason, string> = {
	finished: "The crawl finished. Every page it could reach was read.",
	page_cap: "The crawl stopped at the page limit, so some pages were not read.",
	robots_blocked: "This site's robots.txt blocks Margincite, so no pages were read.",
	site_unreachable: "The site could not be reached, so no pages were read.",
	time_limit: "Stopped at the 30 minute limit. The pages crawled so far are kept.",
};

export function pagesText(count: number): string {
	return `${count} ${count === 1 ? "page" : "pages"}`;
}

/** The sentence for a crawl that has ended. */
export function endedSentence(crawl: Crawl): string {
	if (crawl.stop_reason) return STOP_SENTENCES[crawl.stop_reason];
	return crawl.status === "failed" ? "The crawl failed." : "The crawl ended.";
}

/** A short word for a crawl's state, for lists. */
export function crawlWord(crawl: Crawl | null): string {
	if (!crawl) return "Not crawled yet";
	if (crawl.status === "queued") return "Waiting";
	if (crawl.status === "running") return "Crawling now";
	if (crawl.status === "failed") return "Failed";
	const words: Record<StopReason, string> = {
		finished: "Finished",
		page_cap: "Stopped at limit",
		robots_blocked: "Blocked",
		site_unreachable: "Unreachable",
		time_limit: "Stopped at limit",
	};
	return crawl.stop_reason ? words[crawl.stop_reason] : "Finished";
}
