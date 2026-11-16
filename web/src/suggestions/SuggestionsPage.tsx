import { Link, useParams, useSearchParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useSuggestions } from "../api/hooks";
import type { SuggestionOut } from "../api/types";
import { SelectField } from "../ui/Field";
import { Button } from "../ui/Button";
import { categoryLabel, severityLabel } from "../ui/labels";
import { Pager } from "../ui/Pager";
import { Empty, ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";
import { Table, type Column } from "../ui/Table";

const STATUSES = [
	{ value: "open", label: "Open" },
	{ value: "done", label: "Done" },
	{ value: "dismissed", label: "Dismissed" },
	{ value: "all", label: "All" },
] as const;

const CATEGORIES = [
	{ value: "", label: "All categories" },
	{ value: "technical", label: "Technical" },
	{ value: "ai_readiness", label: "AI readiness" },
	{ value: "content", label: "Content" },
] as const;

const pick = (value: string | null, options: readonly { value: string }[], fallback: string) =>
	options.some((option) => option.value === value) ? (value as string) : fallback;

/** The address carries the filters, so a filtered view can be reloaded and shared. */
function useFilters() {
	const [params, setParams] = useSearchParams();
	const status = pick(params.get("status"), STATUSES, "open");
	const category = pick(params.get("category"), CATEGORIES, "");
	const page = Math.max(1, Number.parseInt(params.get("page") ?? "1", 10) || 1);

	function show(next: { status?: string; category?: string; page?: number }) {
		const merged = { status, category, page, ...next };
		const search = new URLSearchParams();
		if (merged.status !== "open") search.set("status", merged.status);
		if (merged.category) search.set("category", merged.category);
		if (merged.page > 1) search.set("page", String(merged.page));
		setParams(search);
	}
	return { status, category, page, show };
}

function columnsFor(siteId: string, firstRank: number, rows: SuggestionOut[]): Column<SuggestionOut>[] {
	return [
		{
			header: "Rank",
			numeric: true,
			cell: (row) => (
				<span className="inline-flex h-6 w-6 items-center justify-center rounded-md bg-hover text-xs font-medium text-ink-soft">
					{firstRank + rows.indexOf(row)}
				</span>
			),
		},
		{
			header: "Suggestion",
			cell: (row) => (
				<Link to={`/suggestions/${row.id}?site=${siteId}`} className="font-medium text-ink hover:underline transition-colors">
					{row.title}
				</Link>
			),
		},
		{ header: "Category", cell: (row) => categoryLabel(row.category) },
		{
			header: "Severity",
			cell: (row) => (
				<StatusTag tone={row.severity >= 3 ? "ink" : row.severity === 2 ? "open" : "line"}>
					{severityLabel(row.severity)}
				</StatusTag>
			),
		},
		{
			header: "Open",
			numeric: true,
			cell: (row) => (
				<span className="inline-flex items-center rounded-md bg-hover px-2 py-0.5 text-xs font-medium text-ink-soft">
					{row.open}
				</span>
			),
		},
	];
}

/** The ranked suggestions of one site, with filters kept in the address. */
export function SuggestionsPage() {
	const { id = "" } = useParams();
	const filters = useFilters();
	const result = useSuggestions(id, filters);

	return (
		<div className="grid gap-6">
			<div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
				<div>
					<h2 className="text-[15px] font-medium text-ink">Suggestions</h2>
					<p className="text-xs text-ink-muted mt-0.5">Ranked actionable fixes across technical SEO, AI readiness, and content</p>
				</div>
				<div className="flex flex-wrap items-center gap-3">
					<SelectField label="Status" value={filters.status} options={STATUSES} onChange={(status) => filters.show({ status, page: 1 })} />
					<SelectField label="Category" value={filters.category} options={CATEGORIES} onChange={(category) => filters.show({ category, page: 1 })} />
				</div>
			</div>
			{result.isPending && <Loading label="Loading the suggestions…" />}
			{result.isError && <ErrorState message={errorMessage(result.error)} onRetry={() => void result.refetch()} />}
			{result.data && result.data.items.length === 0 && (
				<NoSuggestions filtered={filters.status !== "open" || filters.category !== ""} onClear={() => filters.show({ status: "open", category: "", page: 1 })} />
			)}
			{result.data && result.data.items.length > 0 && (
				<div className="grid gap-4">
					<Table
						caption="Suggestions"
						columns={columnsFor(id, (result.data.meta.page - 1) * result.data.meta.limit + 1, result.data.items)}
						rows={result.data.items}
						rowKey={(row) => row.id}
					/>
					<Pager meta={result.data.meta} onPage={(page) => filters.show({ page })} />
				</div>
			)}
		</div>
	);
}

function NoSuggestions({ filtered, onClear }: { filtered: boolean; onClear: () => void }) {
	if (!filtered) {
		return (
			<Empty title="Nothing open">
				Nothing needs fixing right now. Crawl again after you change the site to check it.
			</Empty>
		);
	}
	return (
		<Empty title="No suggestions match" action={<Button onClick={onClear}>Show open suggestions</Button>}>
			No suggestions match these filters.
		</Empty>
	);
}
