import { useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import { ApiError, errorMessage } from "../api/client";
import { useChangeStatus, useSuggestion } from "../api/hooks";
import type { ItemStatus, SuggestionDetail } from "../api/types";
import { Button } from "../ui/Button";
import { categoryLabel, effortLabel, severityLabel } from "../ui/labels";
import { NotFound } from "../ui/NotFound";
import { Pager } from "../ui/Pager";
import { Empty, ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";
import { ItemsTable } from "./ItemsTable";

const SITE_ID = /^[A-Za-z0-9_-]+$/;

/** Back to the site's suggestions when the address names the site, otherwise to the sites list. */
function BackLink({ siteId }: { siteId: string | null }) {
	return (
		<Link
			to={siteId ? `/sites/${siteId}/suggestions` : "/"}
			className="inline-flex items-center gap-1.5 rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-ink-soft border border-line hover:bg-canvas hover:underline transition-colors max-w-fit"
		>
			<svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4" aria-hidden>
				<path fillRule="evenodd" d="M17 10a.75.75 0 0 1-.75.75H5.612l4.158 3.96a.75.75 0 1 1-1.04 1.08l-5.5-5.25a.75.75 0 0 1 0-1.08l5.5-5.25a.75.75 0 1 1 1.04 1.08L5.612 9.25H16.25A.75.75 0 0 1 17 10Z" clipRule="evenodd" />
			</svg>
			{siteId ? "Back to suggestions" : "Back to your sites"}
		</Link>
	);
}

function Counts({ detail }: { detail: SuggestionDetail }) {
	const { open, done, dismissed } = detail.suggestion;
	return (
		<p className="flex flex-wrap items-center gap-x-3 gap-y-2 text-xs text-ink-soft">
			<StatusTag tone={detail.suggestion.severity >= 3 ? "ink" : detail.suggestion.severity === 2 ? "open" : "line"}>
				{severityLabel(detail.suggestion.severity)}
			</StatusTag>
			<span className="rounded-md bg-hover px-2.5 py-0.5 font-medium text-ink-soft">
				{categoryLabel(detail.suggestion.category)}
			</span>
			<span className="rounded-md bg-hover px-2.5 py-0.5 font-medium text-ink-soft">
				{effortLabel(detail.suggestion.effort)}
			</span>
			<span className="font-medium text-ink">
				{open} open, {done} done, {dismissed} dismissed
			</span>
		</p>
	);
}

type WholeProps = { count: number; onChange: (status: ItemStatus) => void };

/** Done, Dismiss and Reopen for every item at once. Dismissing everything asks first. */
function WholeSuggestion({ count, onChange }: WholeProps) {
	const [asking, setAsking] = useState(false);
	if (asking) {
		return (
			<div role="group" aria-label="Confirm dismissing all" className="grid justify-items-start gap-3 rounded-md border border-line bg-raised p-4">
				<div className="flex items-center gap-2">
					<svg viewBox="0 0 20 20" fill="currentColor" className="h-5 w-5 text-ink-soft" aria-hidden>
						<path fillRule="evenodd" d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495ZM10 5a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5A.75.75 0 0 1 10 5Zm0 9a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z" clipRule="evenodd" />
					</svg>
					<p className="font-medium text-ink text-sm">Dismiss all {count} items? They stay dismissed on later crawls.</p>
				</div>
				<div className="flex flex-wrap gap-2.5">
					<Button
						onClick={() => {
							setAsking(false);
							onChange("dismissed");
						}}
					>
						Yes, dismiss all
					</Button>
					<Button variant="line" onClick={() => setAsking(false)}>
						Keep them
					</Button>
				</div>
			</div>
		);
	}
	return (
		<div className="flex flex-wrap gap-2.5">
			<Button onClick={() => onChange("done")}>
				<svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4" aria-hidden>
					<path fillRule="evenodd" d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z" clipRule="evenodd" />
				</svg>
				Mark all done
			</Button>
			<Button variant="line" onClick={() => setAsking(true)}>
				Dismiss all
			</Button>
			<Button variant="line" onClick={() => onChange("open")}>
				Reopen all
			</Button>
		</div>
	);
}

/** One suggestion: why it matters, how to fix it, and each affected item. */
export function SuggestionPage() {
	const { id = "" } = useParams();
	const [params, setParams] = useSearchParams();
	const siteParam = params.get("site");
	const siteId = siteParam && SITE_ID.test(siteParam) ? siteParam : null;
	const page = Math.max(1, Number.parseInt(params.get("page") ?? "1", 10) || 1);
	const result = useSuggestion(id, page);
	const change = useChangeStatus(id);

	if (result.isPending) return <Loading label="Loading the suggestion…" />;
	if (result.isError) {
		if (result.error instanceof ApiError && result.error.status === 404) return <NotFound what="suggestion" />;
		return <ErrorState message={errorMessage(result.error)} onRetry={() => void result.refetch()} />;
	}
	const { detail, meta } = result.data;

	function goToPage(next: number) {
		const search = new URLSearchParams(params);
		search.set("page", String(next));
		setParams(search);
	}

	return (
		<div className="grid gap-6">
			<BackLink siteId={siteId} />
			<div className="rounded-md border border-line bg-raised p-6 grid gap-3">
				<h1 className="text-lg text-lg font-medium text-ink ">
					{detail.suggestion.title}
				</h1>
				<Counts detail={detail} />
			</div>
			<div className="grid gap-6 lg:grid-cols-2">
				<section className="rounded-md border border-line bg-raised p-6 grid content-start gap-2.5">
					<div className="flex items-center gap-2 border-b border-line pb-2.5">
						<svg viewBox="0 0 20 20" fill="currentColor" className="h-5 w-5 text-ink" aria-hidden>
							<path fillRule="evenodd" d="M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0Zm-7-4a1 1 0 1 1-2 0 1 1 0 0 1 2 0ZM9 9a.75.75 0 0 0 0 1.5h.253a.25.25 0 0 1 .244.304l-.459 2.066A1.75 1.75 0 0 0 10.747 15H11a.75.75 0 0 0 0-1.5h-.253a.25.25 0 0 1-.244-.304l.459-2.066A1.75 1.75 0 0 0 9.253 9H9Z" clipRule="evenodd" />
						</svg>
						<h2 className="text-[15px] font-medium text-ink">Why it matters</h2>
					</div>
					<p className="text-sm text-ink-soft leading-relaxed">{detail.why}</p>
				</section>
				<section className="rounded-md border border-line bg-raised p-6 grid content-start gap-2.5">
					<div className="flex items-center gap-2 border-b border-line pb-2.5">
						<svg viewBox="0 0 20 20" fill="currentColor" className="h-5 w-5 text-ink-soft" aria-hidden>
							<path fillRule="evenodd" d="M10 18a8 8 0 1 0 0-16 8 8 0 0 0 0 16Zm3.857-9.809a.75.75 0 0 0-1.214-.882l-3.483 4.79-1.88-1.88a.75.75 0 1 0-1.06 1.061l2.5 2.5a.75.75 0 0 0 1.137-.089l4-5.5Z" clipRule="evenodd" />
						</svg>
						<h2 className="text-[15px] font-medium text-ink">How to fix it</h2>
					</div>
					<p className="text-sm text-ink-soft leading-relaxed">{detail.fix}</p>
				</section>
			</div>
			<section className="rounded-md border border-line bg-raised p-6 grid gap-4">
				<div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between border-b border-line pb-3">
					<div className="flex items-center gap-2">
						<h2 className="text-[15px] font-medium text-ink">Affected items</h2>
						<span className="rounded-md bg-hover px-2 py-0.5 text-xs font-medium text-ink-soft">
							{meta.total}
						</span>
					</div>
					{detail.items.length > 0 && <WholeSuggestion count={meta.total} onChange={(status) => change.mutate({ status })} />}
				</div>
				{change.isError && (
					<p role="alert" className="notice">
						That change was not saved, and the list is back as it was. {errorMessage(change.error)}
					</p>
				)}
				{detail.items.length === 0 ? (
					<Empty title="Nothing here">There are no items to show for this suggestion.</Empty>
				) : (
					<ItemsTable items={detail.items} onChange={(status, target) => change.mutate({ status, targets: [target] })} />
				)}
				<Pager meta={meta} onPage={goToPage} />
			</section>
		</div>
	);
}
