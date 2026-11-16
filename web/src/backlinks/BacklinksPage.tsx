import { useState } from "react";
import { useParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useBacklinks, useMetrics } from "../api/hooks";
import type { BacklinkTarget } from "../api/types";
import { Button, ButtonLink } from "../ui/Button";
import { CrawledLink } from "../ui/CrawledLink";
import { formatNumber } from "../ui/format";
import { Pager } from "../ui/Pager";
import { Empty, ErrorState, Loading } from "../ui/States";
import { Table, type Column } from "../ui/Table";
import { BacklinkSources } from "./BacklinkSources";

function targetColumns(onShow: (url: string) => void): Column<BacklinkTarget>[] {
	return [
		{ header: "Linked page", cell: (row) => <CrawledLink url={row.target_url} /> },
		{ header: "Links", numeric: true, cell: (row) => formatNumber(row.count) },
		{
			header: "Sources",
			cell: (row) => (
				<Button variant="line" compact aria-label={`Show sources for ${row.target_url}`} onClick={() => onShow(row.target_url)}>
					Show sources
				</Button>
			),
		},
	];
}

/** Backlinks from Bing Webmaster Tools: totals, the most linked pages, and a page's sources on demand. */
export function BacklinksPage() {
	const { id = "" } = useParams();
	const metrics = useMetrics(id);
	const connected = metrics.data?.backlinks.total !== undefined && metrics.data.backlinks.total !== null;
	const [page, setPage] = useState(1);
	const [target, setTarget] = useState<string | null>(null);
	const targets = useBacklinks(id, page, connected);

	if (metrics.isPending) return <Loading label="Loading the backlinks…" />;
	if (metrics.isError) return <ErrorState message={errorMessage(metrics.error)} onRetry={() => void metrics.refetch()} />;
	const { backlinks } = metrics.data;

	if (backlinks.total === null) {
		return (
			<Empty title="No backlink numbers yet" action={<ButtonLink to={`/sites/${id}/connections`}>Open Connections</ButtonLink>}>
				{backlinks.reason ?? "Connect Bing Webmaster Tools to see who links to your site."}
			</Empty>
		);
	}

	return (
		<div className="grid gap-6">
			<div>
				<h2 className="text-[15px] font-medium text-ink">Backlinks</h2>
				<p className="mt-0.5 text-xs text-ink-muted">
					From Bing Webmaster Tools, so it shows the links Bing knows about.
					{backlinks.synced_at ? ` Last synced ${backlinks.synced_at.slice(0, 10)}.` : ""}
				</p>
			</div>
			<ul aria-label="Backlink totals" className="grid grid-cols-2 gap-3">
				{[
					["Total backlinks", backlinks.total],
					["Linked pages", backlinks.linked_pages ?? 0],
				].map(([label, value]) => (
					<li key={label} className="flex flex-col gap-1 rounded-md border border-line bg-raised p-4">
						<span className="label font-medium">{label}</span>
						<span className="text-2xl font-medium text-ink">{formatNumber(Number(value))}</span>
					</li>
				))}
			</ul>
			{targets.isPending && <Loading label="Loading the linked pages…" />}
			{targets.isError && <ErrorState message={errorMessage(targets.error)} onRetry={() => void targets.refetch()} />}
			{targets.data && targets.data.items.length === 0 && (
				<Empty title="No backlinks yet">Bing has not listed any links to this site yet.</Empty>
			)}
			{targets.data && targets.data.items.length > 0 && (
				<div className="grid gap-4">
					<Table caption="Most linked pages" columns={targetColumns(setTarget)} rows={targets.data.items} rowKey={(row) => row.target_url} />
					<Pager meta={targets.data.meta} onPage={setPage} />
				</div>
			)}
			{target && <BacklinkSources siteId={id} target={target} onClose={() => setTarget(null)} />}
		</div>
	);
}
