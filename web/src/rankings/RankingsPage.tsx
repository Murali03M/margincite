import { useState } from "react";
import { useParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useCheckWatch, useWatchEnd } from "../api/checkWatch";
import { useKeywords } from "../api/hooks";
import { useCheckRankings, useRankings, useRefreshComputed } from "../api/researchHooks";
import { useSettings } from "../api/settingsHooks";
import type { RankingKeyword } from "../api/types";
import { Button, ButtonLink } from "../ui/Button";
import { CheckControl } from "../ui/CheckControl";
import { CrawledLink } from "../ui/CrawledLink";
import { formatNumber, shortDate } from "../ui/format";
import { Empty, ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";
import { Table, type Column } from "../ui/Table";
import { RankingDetail } from "./RankingDetail";
import { ARROW, overviewWord, positionText, rankChange } from "./words";

const CHANGE_CLASS = { up: "text-ink-soft", down: "text-ink-soft", flat: "text-ink-muted", none: "text-ink-muted" } as const;
const NONE = "–";

function PositionCell({ row }: { row: RankingKeyword }) {
	const change = rankChange(row);
	return (
		<div className="grid justify-items-end gap-0.5">
			<span className={row.position === null ? "text-sm text-ink-soft" : "text-lg font-medium text-ink"}>{positionText(row)}</span>
			{change.text && (
				<span className={`text-xs font-medium ${CHANGE_CLASS[change.direction]}`}>
					<span aria-hidden>{ARROW[change.direction]} </span>
					{change.text}
				</span>
			)}
		</div>
	);
}

function columnsFor(volumes: Map<string, number | null>, openId: string | null, toggle: (id: string) => void): Column<RankingKeyword>[] {
	return [
		{ header: "Keyword", cell: (row) => <span data-keyword className="break-words font-medium text-ink">{row.keyword}</span> },
		{ header: "Position", numeric: true, cell: (row) => <PositionCell row={row} /> },
		{ header: "Landing page", cell: (row) => (row.url ? <CrawledLink url={row.url} /> : <span className="text-ink-muted">{NONE}</span>) },
		{
			header: "AI Overview",
			cell: (row) => {
				const { word, tone } = overviewWord(row.ai_overview);
				return <StatusTag tone={tone}>{word}</StatusTag>;
			},
		},
		{
			header: "Searches a month (estimate)",
			numeric: true,
			cell: (row) => {
				const volume = volumes.get(row.keyword_id);
				return volume === null || volume === undefined ? NONE : formatNumber(volume);
			},
		},
		{
			header: "Details",
			cell: (row) => {
				const open = openId === row.keyword_id;
				return (
					<Button variant="line" compact aria-expanded={open} aria-label={`${open ? "Hide" : "Show"} top 10 for ${row.keyword}`} onClick={() => toggle(row.keyword_id)}>
						{open ? "Hide" : "Top 10"}
					</Button>
				);
			},
		},
	];
}

/**
 * Rankings: where the site appears in Google for each keyword, how that moved, and whether Google's
 * AI Overview cites it. Needs search data from Settings.
 */
export function RankingsPage({ pollMs }: { pollMs?: number }) {
	const { id = "" } = useParams();
	const settings = useSettings();
	const ready = Boolean(settings.data?.dataforseo.login_set && settings.data.dataforseo.password_set);
	const watch = useCheckWatch();
	const rankings = useRankings(id, ready, watch.watching, pollMs);
	const keywords = useKeywords(id);
	const check = useCheckRankings(id);
	const refresh = useRefreshComputed(id);
	const [openId, setOpenId] = useState<string | null>(null);
	useWatchEnd(watch, rankings.data?.checked_at, refresh, rankings.data?.last_run);

	if (settings.isPending) return <Loading label="Loading the settings…" />;
	if (settings.isError) return <ErrorState message={errorMessage(settings.error)} onRetry={() => void settings.refetch()} />;
	if (!ready) {
		return (
			<Empty title="Search data is not set up" action={<ButtonLink to="/settings">Open Settings</ButtonLink>}>
				Rankings come from Google search results, which Margincite reads through DataForSEO with your own login. Add it in Settings to
				track where each keyword ranks and whether Google's AI Overview cites you. It is paid per use.
			</Empty>
		);
	}
	if (rankings.isPending) return <Loading label="Loading the rankings…" />;
	if (rankings.isError) return <ErrorState message={errorMessage(rankings.error)} onRetry={() => void rankings.refetch()} />;

	const data = rankings.data;
	const volumes = new Map((keywords.data ?? []).map((k) => [k.id, k.search_volume ?? null]));
	const open = data.keywords.find((row) => row.keyword_id === openId);
	const loc = data.location;

	return (
		<div className="grid gap-6">
			<div>
				<h2 className="text-[15px] font-medium text-ink">Rankings</h2>
				<p className="mt-0.5 text-xs text-ink-muted">
					Where your site appears in Google for each keyword.
					{data.checked_at ? ` Checked ${shortDate(data.checked_at)} for ${loc.country}, ${loc.language}, ${loc.device}.` : ""} Positions are a snapshot
					of the results on that day and can differ from what one person sees.
				</p>
			</div>
			<CheckControl label="Check now" working="Checking your keywords" start={() => check.mutateAsync()} watch={watch} checkedAt={data.checked_at} lastRun={data.last_run} />
			{data.keywords.length === 0 ? (
				<Empty title="No keywords to track yet" action={<ButtonLink to={`/sites/${id}/keywords`}>Add keywords</ButtonLink>}>
					Add the searches you want to be found for, then check them here.
				</Empty>
			) : (
				<Table
					caption="Keyword rankings"
					columns={columnsFor(volumes, openId, (key) => setOpenId((now) => (now === key ? null : key)))}
					rows={data.keywords}
					rowKey={(row) => row.keyword_id}
				/>
			)}
			{open && <RankingDetail siteId={id} row={open} />}
		</div>
	);
}
