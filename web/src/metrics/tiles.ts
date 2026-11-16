import type { Metrics } from "../api/types";
import { formatNumber, formatPercent } from "../ui/format";

export type Tile = {
	label: string;
	/** The number as text, or null when it cannot be shown. */
	value: string | null;
	/** A change against the previous period, or null when the API gives none. */
	change: { text: string; direction: "up" | "down" | "flat" } | null;
	/** Where the number comes from, or what it counts. */
	caption: string;
	/** Longer source text for the tooltip. */
	source: string;
	/** Why the number is missing, from the server. */
	reason: string | null;
	/** The screen that fixes a missing number. */
	fix: { to: string; label: string } | null;
};

/** "+4 since the previous crawl", "-5 vs the previous 28 days" or "No change ...". */
export function describeChange(current: number, previous: number | null, since: string): Tile["change"] {
	if (previous === null) return null;
	const diff = Math.round((current - previous) * 10) / 10;
	if (diff === 0) return { text: `No change ${since}`, direction: "flat" };
	const sign = diff > 0 ? "+" : "-";
	return { text: `${sign}${formatNumber(Math.abs(diff))} ${since}`, direction: diff > 0 ? "up" : "down" };
}

const GSC_SOURCE = "From Google Search Console, last 28 days";
const PREVIOUS_28 = "vs the previous 28 days";

/** The nine tiles of the project card, in order. */
export function buildTiles(siteId: string, m: Metrics): Tile[] {
	const base = `/sites/${siteId}`;
	const ai = { to: `${base}/ai`, label: "Open AI Visibility" };
	const connections = { to: `${base}/connections`, label: "Open Connections" };
	const keywords = { to: `${base}/keywords`, label: "Open Keywords" };
	const aiSource = "From the latest AI visibility run: answers that mention or cite your site.";

	const tile = (t: Omit<Tile, "change" | "reason" | "fix"> & Partial<Pick<Tile, "change" | "reason" | "fix">>): Tile => ({
		change: null,
		reason: null,
		fix: null,
		...t,
	});

	const { ai_visibility: av, site_health: health, visibility: vis, organic, backlinks, rankings, speed } = m;
	const fromRankings = rankings.in_top_10 !== null;

	return [
		tile({
			label: "AI Visibility",
			value: av.percent === null ? null : formatPercent(av.percent),
			caption: "Share of AI answers that mention or cite you",
			source: aiSource,
			reason: av.reason,
			fix: av.percent === null ? ai : null,
		}),
		tile({
			label: "Mentions",
			value: av.mentions === null ? null : formatNumber(av.mentions),
			caption: `of ${formatNumber(av.answers)} answers`,
			source: aiSource,
			reason: av.reason,
			fix: av.mentions === null ? ai : null,
		}),
		tile({
			label: "Site Health",
			value: health.score === null ? null : String(health.score),
			change: health.score === null ? null : describeChange(health.score, health.previous, "since the previous crawl"),
			caption: "Score from the latest crawl",
			source: "From the latest completed crawl, out of 100.",
			reason: health.reason,
		}),
		tile({
			label: "Page speed",
			value: speed.performance === null ? null : String(speed.performance),
			caption: "Mobile score from Google PageSpeed (an estimate)",
			source: "From Google PageSpeed Insights: a simulated mobile test of your home page and a few busy pages. Treat it as an estimate.",
			reason: speed.reason,
			fix: speed.performance === null ? { to: `${base}/checks#page-speed`, label: "Open Page speed" } : null,
		}),
		tile({
			label: "Visibility",
			value: vis.percent === null ? null : formatPercent(vis.percent),
			caption: `${formatNumber(vis.in_top_10 ?? 0)} of ${formatNumber(vis.keywords)} keywords in the top 10`,
			source: fromRankings
				? "From your rankings: the share of your tracked keywords that are in the top 10 of Google."
				: `${GSC_SOURCE}: the share of your target keywords with an average position of 10 or better.`,
			reason: vis.reason,
			fix: vis.percent === null ? (vis.keywords === 0 ? keywords : connections) : null,
		}),
		tile({
			label: "Rankings",
			value: rankings.in_top_10 === null ? null : formatNumber(rankings.in_top_10),
			caption: `of ${formatNumber(rankings.tracked)} keywords in the top 10`,
			source: `From the latest rankings check: where your site appears in Google for each tracked keyword${
				rankings.in_top_3 === null ? "" : `, ${formatNumber(rankings.in_top_3)} in the top 3`
			}${rankings.average_position === null ? "" : `, average position ${formatNumber(rankings.average_position)}`}.`,
			reason: rankings.reason,
			fix: rankings.in_top_10 === null ? { to: `${base}/rankings`, label: "Open Rankings" } : null,
		}),
		tile({
			label: "Organic Traffic",
			value: organic.clicks === null ? null : formatNumber(organic.clicks),
			change: organic.clicks === null ? null : describeChange(organic.clicks, organic.clicks_previous, PREVIOUS_28),
			caption: `${GSC_SOURCE}`,
			source: `${GSC_SOURCE}: clicks from Google search results.`,
			reason: organic.reason,
			fix: organic.clicks === null ? connections : null,
		}),
		tile({
			label: "Organic Keywords",
			value: organic.keywords === null ? null : formatNumber(organic.keywords),
			change: organic.keywords === null ? null : describeChange(organic.keywords, organic.keywords_previous, PREVIOUS_28),
			caption: `${GSC_SOURCE}`,
			source: `${GSC_SOURCE}: different searches that showed your site. Hidden queries are not counted.`,
			reason: organic.reason,
			fix: organic.keywords === null ? connections : null,
		}),
		tile({
			label: "Backlinks",
			value: backlinks.total === null ? null : formatNumber(backlinks.total),
			caption: "From Bing Webmaster Tools",
			source: "From Bing Webmaster Tools: links to your site that Bing knows about.",
			reason: backlinks.reason,
			fix: backlinks.total === null ? connections : null,
		}),
	];
}
