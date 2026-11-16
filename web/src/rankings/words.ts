import type { AiOverviewStatus, RankingKeyword } from "../api/types";
import type { Tone } from "../ui/StatusTag";

export type RankChange = { text: string; direction: "up" | "down" | "flat" | "none" };

/** What the position cell says: a number, or why there is none. Never 0. */
export function positionText(row: RankingKeyword): string {
	if (row.position !== null) return String(row.position);
	return hasData(row) ? "Not in the top 100" : "Not checked yet";
}

/** A keyword that was never checked has no results and no history; anything else was looked up. */
export function hasData(row: RankingKeyword): boolean {
	return row.top.length > 0 || row.history.length > 0 || row.previous !== null;
}

const places = (n: number) => `${n} ${n === 1 ? "place" : "places"}`;

/** The change since the previous check, in words. A lower position number is better. */
export function rankChange(row: RankingKeyword): RankChange {
	const { position, previous } = row;
	if (position !== null && previous !== null) {
		const gained = previous - position;
		if (gained > 0) return { text: `Up ${places(gained)}`, direction: "up" };
		if (gained < 0) return { text: `Down ${places(-gained)}`, direction: "down" };
		return { text: "No change", direction: "flat" };
	}
	if (position !== null) {
		return { text: row.history.length <= 1 ? "First check" : "Newly in the top 100", direction: "none" };
	}
	if (previous !== null) return { text: "Dropped out of the top 100", direction: "down" };
	return { text: "", direction: "none" };
}

export const ARROW = { up: "▲", down: "▼", flat: "–", none: "" } as const;

const OVERVIEW: Record<AiOverviewStatus, { word: string; tone: Tone }> = {
	cited: { word: "Cited", tone: "course" },
	present: { word: "AI Overview shown without you", tone: "open" },
	absent: { word: "No AI Overview", tone: "line" },
	unknown: { word: "Not known", tone: "line" },
};

export const overviewWord = (status: AiOverviewStatus) => OVERVIEW[status] ?? OVERVIEW.unknown;

/** A site's host without a leading www., so "www.example.com" and "example.com" match. */
export const bareHost = (host: string): string => host.toLowerCase().replace(/^www\./, "");
