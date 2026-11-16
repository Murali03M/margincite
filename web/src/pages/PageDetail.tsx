import { Link, useParams } from "react-router-dom";
import { ApiError, errorMessage } from "../api/client";
import { useCrawl, usePage } from "../api/hooks";
import { NotFound } from "../ui/NotFound";
import { ErrorState, Loading } from "../ui/States";
import { PageFacts } from "./PageFacts";
import { PageFindings } from "./PageFindings";

function BackLink({ siteId, settled }: { siteId: string | null; settled: boolean }) {
	if (siteId) {
		return (
			<Link
				to={`/sites/${siteId}/pages`}
				className="inline-flex items-center gap-1.5 rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-ink-soft border border-line hover:bg-canvas hover:underline transition-colors max-w-fit"
			>
				<svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4" aria-hidden>
					<path fillRule="evenodd" d="M17 10a.75.75 0 0 1-.75.75H5.612l4.158 3.96a.75.75 0 1 1-1.04 1.08l-5.5-5.25a.75.75 0 0 1 0-1.08l5.5-5.25a.75.75 0 1 1 1.04 1.08L5.612 9.25H16.25A.75.75 0 0 1 17 10Z" clipRule="evenodd" />
				</svg>
				Back to pages
			</Link>
		);
	}
	if (!settled) return null;
	return (
		<Link
			to="/"
			className="inline-flex items-center gap-1.5 rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-ink-soft border border-line hover:bg-canvas hover:underline transition-colors max-w-fit"
		>
			<svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4" aria-hidden>
				<path fillRule="evenodd" d="M17 10a.75.75 0 0 1-.75.75H5.612l4.158 3.96a.75.75 0 1 1-1.04 1.08l-5.5-5.25a.75.75 0 0 1 0-1.08l5.5-5.25a.75.75 0 1 1 1.04 1.08L5.612 9.25H16.25A.75.75 0 0 1 17 10Z" clipRule="evenodd" />
			</svg>
			Back to your sites
		</Link>
	);
}

/** One crawled page: its facts, the findings against it, and the text that was read from it. */
export function PageDetail() {
	const { id = "" } = useParams();
	const page = usePage(id);
	const crawl = useCrawl(page.data?.crawl_id, { poll: false });

	if (page.isPending) return <Loading label="Loading the page…" />;
	if (page.isError) {
		if (page.error instanceof ApiError && page.error.status === 404) return <NotFound what="page" />;
		return <ErrorState message={errorMessage(page.error)} onRetry={() => void page.refetch()} />;
	}
	const data = page.data;
	const siteId = crawl.data?.site_id ?? null;

	return (
		<div className="grid gap-6">
			<BackLink siteId={siteId} settled={!crawl.isPending} />
			<div className="rounded-md border border-line bg-raised p-6">
				<h1 className="break-words text-lg text-lg font-medium text-ink ">
					{data.facts?.title || "Untitled page"}
				</h1>
				<p className="mono text-ink-muted break-all mt-1">{data.url}</p>
			</div>
			<div className="grid gap-6 lg:grid-cols-2">
				<section className="rounded-md border border-line bg-raised p-6 grid gap-3 content-start">
					<h2 className="text-[15px] font-medium text-ink border-b border-line pb-2.5">Facts</h2>
					<PageFacts page={data} />
					{!data.facts && <p className="text-xs text-ink-muted">No details were read from this page.</p>}
				</section>
				<section className="rounded-md border border-line bg-raised p-6 grid gap-3 content-start">
					<h2 className="text-[15px] font-medium text-ink border-b border-line pb-2.5">Findings</h2>
					<PageFindings pageId={id} siteId={siteId} />
				</section>
			</div>
			<section className="rounded-md border border-line bg-raised p-6 grid gap-3">
				<h2 className="text-[15px] font-medium text-ink border-b border-line pb-2.5">Text read from this page</h2>
				{data.text ? (
					<div
						role="region"
						aria-label="Text read from this page"
						tabIndex={0}
						className="max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-md bg-canvas p-4 mono text-ink-soft border border-line leading-relaxed"
					>
						{data.text}
					</div>
				) : (
					<p className="text-xs text-ink-muted">No text was stored for this page.</p>
				)}
			</section>
		</div>
	);
}
