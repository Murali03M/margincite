import { Link, useParams, useSearchParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { usePages, useSummary } from "../api/hooks";
import type { PageSummary } from "../api/types";
import { Pager } from "../ui/Pager";
import { Empty, ErrorState, Loading } from "../ui/States";
import { Table, type Column } from "../ui/Table";

function statusBadge(page: PageSummary) {
	if (page.status_code !== null) {
		const code = page.status_code;
		const dot = code >= 200 && code < 300 ? "dot-good" : code >= 300 && code < 400 ? "dot-info" : code >= 400 && code < 500 ? "dot-warn" : "dot-bad";
		return (
			<span className="tag">
				<span className={`dot ${dot}`} aria-hidden />
				{code}
			</span>
		);
	}
	return (
		<span className="tag">
			<span className="dot dot-bad" aria-hidden />
			{page.fetch_error ? `Not read: ${page.fetch_error}` : "Not read"}
		</span>
	);
}

const columns: Column<PageSummary>[] = [
	{
		header: "Address",
		cell: (page) => (
			<Link to={`/pages/${page.id}`} className="break-all font-medium text-ink hover:underline transition-colors">
				{page.url}
			</Link>
		),
	},
	{ header: "Status", cell: statusBadge },
	{
		header: "Depth",
		numeric: true,
		cell: (page) => <span className="mono text-ink-soft">{page.depth}</span>,
	},
	{
		header: "In sitemap",
		cell: (page) => (
			<span className={`text-xs font-medium ${page.in_sitemap ? "text-ink-soft" : "text-ink-muted"}`}>
				{page.in_sitemap ? "Yes" : "No"}
			</span>
		),
	},
	{
		header: "Needed JavaScript",
		cell: (page) => (
			<span className={`text-xs font-medium ${page.needed_javascript ? "text-ink-soft" : "text-ink-muted"}`}>
				{page.needed_javascript ? "Yes" : "No"}
			</span>
		),
	},
];

const NO_PAGES = (
	<Empty title="No pages yet">
		No pages were read. Crawl the site from the Overview tab, then look here for every page Margincite read.
	</Empty>
);

/** The pages of the site's latest crawl, as a paged table. */
export function PagesPage() {
	const { id = "" } = useParams();
	const [params, setParams] = useSearchParams();
	const page = Math.max(1, Number.parseInt(params.get("page") ?? "1", 10) || 1);
	const summary = useSummary(id);
	const crawlId = summary.data?.latest_crawl?.id ?? null;
	const pages = usePages(crawlId, page);

	if (summary.isPending) return <Loading label="Loading the pages…" />;
	if (summary.isError) return <ErrorState message={errorMessage(summary.error)} onRetry={() => void summary.refetch()} />;
	if (!crawlId) return NO_PAGES;
	if (pages.isPending) return <Loading label="Loading the pages…" />;
	if (pages.isError) return <ErrorState message={errorMessage(pages.error)} onRetry={() => void pages.refetch()} />;
	if (pages.data.items.length === 0) return NO_PAGES;

	return (
		<div className="grid gap-6">
			<div className="flex flex-col gap-1">
				<h2 className="text-[15px] font-medium text-ink">Pages</h2>
				<p className="text-xs text-ink-muted">Every URL indexed in the latest crawl with status and render requirements</p>
			</div>
			<div className="grid gap-4">
				<Table caption="Pages from the latest crawl" columns={columns} rows={pages.data.items} rowKey={(row) => row.id} />
				<Pager meta={pages.data.meta} onPage={(next) => setParams(next > 1 ? { page: String(next) } : {})} />
			</div>
		</div>
	);
}
