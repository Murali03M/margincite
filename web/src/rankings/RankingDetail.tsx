import { useSite } from "../api/hooks";
import type { RankingKeyword, RankingResult } from "../api/types";
import { CrawledLink } from "../ui/CrawledLink";
import { Panel } from "../ui/Panel";
import { StatusTag } from "../ui/StatusTag";
import { Table, type Column } from "../ui/Table";
import { PositionChart } from "./PositionChart";
import { bareHost } from "./words";

function columnsFor(host: string | undefined): Column<RankingResult>[] {
	return [
		{ header: "Position", numeric: true, cell: (row) => String(row.position) },
		{
			header: "Result",
			cell: (row) => (
				<div className="grid gap-0.5">
					<span className="break-words font-medium text-ink">{row.title}</span>
					<CrawledLink url={row.url} />
				</div>
			),
		},
		{
			header: "Domain",
			cell: (row) => (
				<span className="inline-flex flex-wrap items-center gap-2">
					<span className="break-all">{row.domain}</span>
					{host && bareHost(row.domain) === bareHost(host) && <StatusTag tone="course">You</StatusTag>}
				</span>
			),
		},
	];
}

/** The ten results Google showed for a keyword at the last check, and how the site's position moved. */
export function RankingDetail({ siteId, row }: { siteId: string; row: RankingKeyword }) {
	const site = useSite(siteId);
	return (
		<Panel title={`Detail for ${row.keyword}`}>
			<div className="grid gap-2">
				<h3 className="text-sm font-medium text-ink">Top 10 results at the last check</h3>
				{row.top.length === 0 ? (
					<p className="text-sm text-ink-muted">No results saved for this keyword yet. Check now to fetch them.</p>
				) : (
					<Table caption={`Top 10 results for ${row.keyword}`} columns={columnsFor(site.data?.host)} rows={row.top.slice(0, 10)} rowKey={(r) => `${r.position}-${r.url}`} />
				)}
			</div>
			<div className="grid gap-2">
				<h3 className="text-sm font-medium text-ink">Position over time</h3>
				<PositionChart keyword={row.keyword} history={row.history} />
			</div>
		</Panel>
	);
}
