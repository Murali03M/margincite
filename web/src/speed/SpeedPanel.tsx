import { errorMessage } from "../api/client";
import { useCheckWatch, useWatchEnd } from "../api/checkWatch";
import { useCheckSpeed, useRefreshComputed, useSpeed } from "../api/researchHooks";
import type { SpeedPage } from "../api/types";
import { CheckControl } from "../ui/CheckControl";
import { CrawledLink } from "../ui/CrawledLink";
import { Panel } from "../ui/Panel";
import { shortDate } from "../ui/format";
import { ErrorState, Loading } from "../ui/States";
import { StatusTag, type Tone } from "../ui/StatusTag";
import { Table, type Column } from "../ui/Table";

const NONE = "–";

/** Google's own bands: 90 and up is good, 50 to 89 needs work, below 50 is poor. */
export function scoreWord(score: number | null): { word: string; tone: Tone } {
	if (score === null) return { word: "No score", tone: "line" };
	if (score >= 90) return { word: "Good", tone: "course" };
	if (score >= 50) return { word: "Needs work", tone: "open" };
	return { word: "Poor", tone: "ink" };
}

const seconds = (ms: number | null) => (ms === null ? NONE : `${(ms / 1000).toFixed(1)} s`);
const millis = (ms: number | null) => (ms === null ? NONE : `${Math.round(ms)} ms`);

const columns: Column<SpeedPage>[] = [
	{
		header: "Page",
		cell: (row) => (
			<div className="grid gap-1">
				<CrawledLink url={row.url} />
				{row.error && <span className="text-xs text-ink-soft">{row.error}</span>}
			</div>
		),
	},
	{
		header: "Score",
		cell: (row) => {
			const { word, tone } = scoreWord(row.performance);
			return (
				<span className="inline-flex flex-wrap items-center gap-2">
					{row.performance !== null && <span className="text-lg font-medium text-ink">{row.performance}</span>}
					<StatusTag tone={tone}>{word}</StatusTag>
				</span>
			);
		},
	},
	{ header: "Largest content (LCP)", numeric: true, cell: (row) => seconds(row.lcp_ms) },
	{ header: "Layout shift (CLS)", numeric: true, cell: (row) => (row.cls === null ? NONE : row.cls.toFixed(2)) },
	{ header: "Responsiveness (INP)", numeric: true, cell: (row) => millis(row.inp_ms) },
	{ header: "Blocking time (TBT)", numeric: true, cell: (row) => millis(row.tbt_ms) },
	{ header: "Data", cell: (row) => (row.field_data ? "Real visitors" : "Lab test only") },
];

/**
 * Page speed from Google PageSpeed Insights: the home page and a few busy pages, on a phone.
 * Lab numbers are a simulated test, so they are described as estimates.
 */
export function SpeedPanel({ siteId, pollMs }: { siteId: string; pollMs?: number }) {
	const watch = useCheckWatch();
	const speed = useSpeed(siteId, watch.watching, pollMs);
	const check = useCheckSpeed(siteId);
	const refresh = useRefreshComputed(siteId);
	useWatchEnd(watch, speed.data?.checked_at, refresh);

	let body;
	if (speed.isPending) body = <Loading label="Loading the page speed…" />;
	else if (speed.isError) body = <ErrorState message={errorMessage(speed.error)} onRetry={() => void speed.refetch()} />;
	else {
		const { checked_at: checkedAt, pages } = speed.data;
		body = (
			<>
				{pages.length === 0 ? (
					<p className="text-sm text-ink-muted">Not checked yet. Press the button to test your home page and a few busy pages.</p>
				) : (
					<>
						<p className="text-xs text-ink-muted">{checkedAt ? `Checked ${shortDate(checkedAt)}.` : ""} Scores are out of 100.</p>
						<Table caption="Page speed on mobile" columns={columns} rows={pages} rowKey={(row) => row.url} />
					</>
				)}
				<CheckControl label="Check page speed" working="Testing your pages" start={() => check.mutateAsync()} watch={watch} checkedAt={checkedAt} />
			</>
		);
	}

	return (
		<Panel title="Page speed">
			<p className="text-sm leading-relaxed text-ink-soft">
				Slow pages lose visitors. Margincite asks Google PageSpeed Insights to load your home page and the four busiest pages on a
				phone. The scores come from a simulated test, so treat them as an estimate. Rows marked Real visitors also use data from Chrome users.
			</p>
			{body}
		</Panel>
	);
}
