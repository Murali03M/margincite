import { useState } from "react";
import { errorMessage } from "../api/client";
import { useSearchPages, useSearchQueries, type QuerySort } from "../api/hooks";
import type { SearchPageRow, SearchQuery } from "../api/types";
import { CrawledLink } from "../ui/CrawledLink";
import { SelectField } from "../ui/Field";
import { formatNumber } from "../ui/format";
import { Pager } from "../ui/Pager";
import { Empty, ErrorState, Loading } from "../ui/States";
import { Table, type Column } from "../ui/Table";

const SORTS = [
	{ value: "clicks", label: "Clicks" },
	{ value: "impressions", label: "Impressions" },
	{ value: "position", label: "Position, best first" },
] as const;

const percent = (ctr: number) => `${(ctr * 100).toFixed(1)}%`;

const queryColumns: Column<SearchQuery>[] = [
	{ header: "Query", cell: (row) => <span className="break-words font-medium text-ink">{row.query}</span> },
	{ header: "Clicks", numeric: true, cell: (row) => formatNumber(row.clicks) },
	{ header: "Impressions", numeric: true, cell: (row) => formatNumber(row.impressions) },
	{ header: "CTR", numeric: true, cell: (row) => percent(row.ctr) },
	{ header: "Position", numeric: true, cell: (row) => row.position.toFixed(1) },
	{ header: "Top page", cell: (row) => (row.top_page ? <CrawledLink url={row.top_page} /> : "–") },
];

const pageColumns: Column<SearchPageRow>[] = [
	{ header: "Page", cell: (row) => <CrawledLink url={row.url} /> },
	{ header: "Clicks", numeric: true, cell: (row) => formatNumber(row.clicks) },
	{ header: "Impressions", numeric: true, cell: (row) => formatNumber(row.impressions) },
	{ header: "CTR", numeric: true, cell: (row) => percent(row.ctr) },
	{ header: "Position", numeric: true, cell: (row) => row.position.toFixed(1) },
];

type Result<T> = {
	isPending: boolean;
	isError: boolean;
	error: unknown;
	refetch: () => unknown;
	data?: { items: T[]; meta: { total: number; page: number; limit: number } };
};

/** The shared loading, error, empty and table states of a paged list. */
function ListBody<T>({ result, empty, caption, columns, rowKey, onPage }: {
	result: Result<T>;
	empty: string;
	caption: string;
	columns: Column<T>[];
	rowKey: (row: T) => string;
	onPage: (page: number) => void;
}) {
	if (result.isPending) return <Loading label={`Loading ${caption.toLowerCase()}…`} />;
	if (result.isError) return <ErrorState message={errorMessage(result.error)} onRetry={() => void result.refetch()} />;
	if (!result.data || result.data.items.length === 0) return <Empty title={empty}>Nothing has been synced yet. Try Sync now on Connections.</Empty>;
	return (
		<div className="grid gap-4">
			<Table caption={caption} columns={columns} rows={result.data.items} rowKey={rowKey} />
			<Pager meta={result.data.meta} onPage={onPage} />
		</div>
	);
}

/** Top queries (sortable) and top pages from Search Console, last 28 days. */
export function SearchTables({ siteId }: { siteId: string }) {
	const [sort, setSort] = useState<QuerySort>("clicks");
	const [queryPage, setQueryPage] = useState(1);
	const [pagesPage, setPagesPage] = useState(1);
	const queries = useSearchQueries(siteId, queryPage, sort, true);
	const pages = useSearchPages(siteId, pagesPage, true);

	return (
		<div className="grid gap-8">
			<section className="grid gap-4">
				<div className="flex flex-wrap items-end justify-between gap-3">
					<h3 className="text-sm font-medium text-ink">Top queries</h3>
					<SelectField
						label="Sort queries by"
						value={sort}
						options={SORTS}
						onChange={(value) => {
							setSort(value as QuerySort);
							setQueryPage(1);
						}}
					/>
				</div>
				<ListBody result={queries} empty="No queries yet" caption="Top queries" columns={queryColumns} rowKey={(row) => row.query} onPage={setQueryPage} />
			</section>
			<section className="grid gap-4">
				<h3 className="text-sm font-medium text-ink">Top pages</h3>
				<ListBody result={pages} empty="No pages yet" caption="Top pages" columns={pageColumns} rowKey={(row) => row.url} onPage={setPagesPage} />
			</section>
		</div>
	);
}
