import { Link, useParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useKeywords } from "../api/hooks";
import { useVolumeStatus } from "../api/researchHooks";
import type { Keyword, LandingSource } from "../api/types";
import { CrawledLink } from "../ui/CrawledLink";
import { formatMoney, formatNumber, titleCase } from "../ui/format";
import { Empty, ErrorState, Loading } from "../ui/States";
import { Table, type Column } from "../ui/Table";
import { KeywordActions } from "./KeywordActions";
import { IdeasPanel } from "./IdeasPanel";
import { KeywordForm } from "./KeywordForm";
import { RefreshVolumes } from "./RefreshVolumes";

const SOURCE_WORDS: Record<LandingSource, string> = {
	set: "Chosen by you",
	search_console: "Most shown in Google",
	best_match: "Best match by title and headings",
	home: "Home page, no better match",
};

const NONE = "–";
const number = (value: number | null) => (value === null ? NONE : formatNumber(value));

function columnsFor(siteId: string): Column<Keyword>[] {
	return [
		{ header: "Keyword", cell: (row) => <span className="break-words font-medium text-ink">{row.keyword}</span> },
		{
			header: "Target page",
			cell: (row) => (row.target_url ? <span className="break-all">{row.target_url}</span> : <span className="text-ink-muted">{NONE}</span>),
		},
		{ header: "Landing page", cell: (row) => (row.landing_url ? <CrawledLink url={row.landing_url} /> : <span className="text-ink-muted">{NONE}</span>) },
		{ header: "How it was chosen", cell: (row) => (row.landing_source ? SOURCE_WORDS[row.landing_source] : "Chosen after the first crawl") },
		{ header: "Position", numeric: true, cell: (row) => (row.position === null ? NONE : row.position.toFixed(1)) },
		{ header: "Clicks", numeric: true, cell: (row) => number(row.clicks) },
		{ header: "Impressions", numeric: true, cell: (row) => number(row.impressions) },
		{ header: "Searches a month (estimate)", numeric: true, cell: (row) => number(row.search_volume ?? null) },
		{ header: "Competition", cell: (row) => (row.competition ? titleCase(row.competition) : NONE) },
		{ header: "Cost per click (estimate)", numeric: true, cell: (row) => (row.cpc === null || row.cpc === undefined ? NONE : formatMoney(row.cpc)) },
		{ header: "Actions", cell: (row) => <KeywordActions siteId={siteId} keyword={row} /> },
	];
}

/** Target keywords: add, remove, choose the page each is judged on, and see how they do in search. */
export function KeywordsPage({ pollMs }: { pollMs?: number }) {
	const { id = "" } = useParams();
	const keywords = useKeywords(id);
	const volumeFailed = useVolumeStatus(id, false).data?.status === "failed";

	return (
		<div className="grid gap-6">
			<div className="flex flex-wrap items-start justify-between gap-3">
				<div>
					<h2 className="text-[15px] font-medium text-ink">Keywords</h2>
					<p className="mt-0.5 text-xs text-ink-muted">The searches you want to be found for. Up to 50. The next crawl checks them.</p>
				</div>
				<RefreshVolumes siteId={id} pollMs={pollMs} />
			</div>
			<KeywordForm siteId={id} />
			{keywords.isPending && <Loading label="Loading the keywords…" />}
			{keywords.isError && <ErrorState message={errorMessage(keywords.error)} onRetry={() => void keywords.refetch()} />}
			{keywords.data && keywords.data.length === 0 && (
				<Empty title="No keywords yet">Add the words people would search for to find your site. The next crawl checks your pages for them.</Empty>
			)}
			{keywords.data && keywords.data.length > 0 && (
				<div className="grid gap-3">
					{keywords.data.every((row) => row.position === null) && (
						<p className="text-xs leading-relaxed text-ink-muted">
							Position, clicks and impressions come from Google Search Console (last 28 days). They appear once it is connected
							and has synced; see{" "}
							<Link to={`/sites/${id}/connections`} className="font-medium text-ink">
								Connections
							</Link>
							.
						</p>
					)}
					{!volumeFailed && keywords.data.every((row) => row.search_volume == null) && (
						<p className="text-xs leading-relaxed text-ink-muted">
							Search volume, competition and cost per click come from DataForSEO. Add it in{" "}
							<Link to="/settings" className="font-medium text-ink">
								Settings
							</Link>
							, then press Refresh volumes. These are estimates.
						</p>
					)}
					<Table caption="Target keywords" columns={columnsFor(id)} rows={keywords.data} rowKey={(row) => row.id} />
				</div>
			)}
			<IdeasPanel siteId={id} keywords={keywords.data ?? []} />
		</div>
	);
}
