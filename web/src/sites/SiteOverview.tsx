import { useCallback, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "../api/client";
import { isCrawlActive, keys, useCrawl, useStartCrawl, useSummary } from "../api/hooks";
import type { SiteSummary } from "../api/types";
import { Button } from "../ui/Button";
import { categoryLabel, effortLabel, severityLabel } from "../ui/labels";
import { ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";
import { Table, type Column } from "../ui/Table";
import { MetricCards } from "../metrics/MetricCards";
import { CrawlStatus } from "./CrawlStatus";
import { endedSentence } from "./crawlText";

const SCORE_EXPLANATION = "100 minus the share of pages affected by each open check, weighted by how serious it is.";

function healthLabel(score: number): { text: string; dot: string } {
	if (score >= 85) return { text: "Good health", dot: "dot-good" };
	if (score >= 70) return { text: "Fair health", dot: "dot-warn" };
	return { text: "Needs attention", dot: "dot-bad" };
}

function ScoreBlock({ summary, crawling }: { summary: SiteSummary; crawling: boolean }) {
	const { score, latest_crawl: latest } = summary;
	if (score !== null) {
		const health = healthLabel(score);
		return (
			<div className="grid gap-3">
				<div className="flex flex-wrap items-center gap-3">
					<p className="flex items-baseline gap-2.5">
						<span className="text-5xl font-medium text-ink">{score}</span>
						<span className="text-sm font-medium text-ink-muted">out of 100</span>
					</p>
					<span className="tag">
						<span className={`dot ${health.dot}`} aria-hidden />
						{health.text}
					</span>
				</div>
				<p className="max-w-[28rem] text-xs text-ink-muted leading-relaxed">{SCORE_EXPLANATION}</p>
			</div>
		);
	}
	if (!latest) {
		return (
			<div className="rounded-md border border-line bg-canvas p-4">
				<p className="font-medium text-ink">No score yet. Start a crawl to get one.</p>
				<p className="text-xs text-ink-muted mt-1">Margincite will analyze all pages and generate a health score.</p>
			</div>
		);
	}
	if (crawling || isCrawlActive(latest)) {
		return (
			<div className="flex items-center gap-3 rounded-md border border-line bg-raised p-4 text-ink">
				<svg className="h-4 w-4 animate-spin text-ink-soft shrink-0" aria-hidden fill="none" viewBox="0 0 24 24">
					<circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
					<path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
				</svg>
				<p className="font-medium text-sm">No score yet. It appears when the crawl finishes.</p>
			</div>
		);
	}
	return (
		<div className="rounded-md border border-line bg-raised p-4 grid gap-2">
			<p className="flex items-center gap-2 text-sm font-medium text-ink"><span className="dot dot-bad" aria-hidden />No score: no pages could be read.</p>
			<p className="text-sm text-ink-soft">{endedSentence(latest)}</p>
			{latest.error && <p className="text-xs text-ink-soft">{latest.error}</p>}
		</div>
	);
}

function CrawlAgain({ siteId, busyWithCrawl, onStarted }: { siteId: string; busyWithCrawl: boolean; onStarted: (id: string) => void }) {
	const start = useStartCrawl(siteId);
	const [message, setMessage] = useState<string | null>(null);

	async function run() {
		setMessage(null);
		try {
			onStarted((await start.mutateAsync()).id);
		} catch (error) {
			setMessage(errorMessage(error));
		}
	}

	return (
		<div className="grid justify-items-start gap-3">
			<Button onClick={() => void run()} busy={start.isPending} disabled={busyWithCrawl}>
				<svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4 shrink-0" aria-hidden>
					<path fillRule="evenodd" d="M15.312 11.424a5.5 5.5 0 0 1-9.201 2.466l-.312-.311h2.433a.75.75 0 0 0 0-1.5H3.75a.75.75 0 0 0-.75.75v4.482a.75.75 0 0 0 1.5 0v-2.296l.487.487a7 7 0 1 0 .97-9.585.75.75 0 1 0 1.05 1.07 5.5 5.5 0 0 1 8.305 4.407Z" clipRule="evenodd" />
				</svg>
				{start.isPending ? "Starting…" : "Crawl again"}
			</Button>
			{message && (
				<p role="alert" className="notice">
					{message}
				</p>
			)}
		</div>
	);
}

function Facts({ info }: { info: SiteSummary["info"] }) {
	const found = (yes: boolean) => (yes ? "Found" : "Not found");
	return (
		<ul aria-label="Crawl facts" className="divide-y divide-line">
			<Fact label="Pages crawled" value={String(info.pages_crawled)} />
			<Fact label="Sitemap" value={found(info.sitemap_found)} isTag={true} isSuccess={info.sitemap_found} />
			<Fact
				label="llms.txt"
				value={found(info.llms_txt_found)}
				note="For information only. Margincite does not count this as a problem."
				isTag={true}
				isSuccess={info.llms_txt_found}
			/>
		</ul>
	);
}

function Fact({ label, value, note, isTag, isSuccess }: { label: string; value: string; note?: string; isTag?: boolean; isSuccess?: boolean }) {
	return (
		<li className="py-3">
			<div className="grid gap-x-4 gap-y-1 sm:grid-cols-[10rem_1fr] items-baseline">
				<span className="label text-ink-muted font-medium">{label}</span>
				<span className="font-medium text-ink text-sm">
					{isTag ? (
						<span className="tag">
							<span className={`dot ${isSuccess ? "dot-good" : "dot-info"}`} aria-hidden />
							{value}
						</span>
					) : (
						value
					)}
				</span>
				{note && <p className="text-xs text-ink-muted sm:col-start-2 mt-0.5 leading-relaxed">{note}</p>}
			</div>
		</li>
	);
}

function Counts({ counts }: { counts: SiteSummary["counts"] }) {
	const rows = [
		["Open", counts.open],
		["Done", counts.done],
		["Dismissed", counts.dismissed],
	] as const;

	return (
		<ul aria-label="Suggestion counts" className="grid grid-cols-3 gap-3">
			{rows.map(([label, count]) => (
				<li key={label} className="rounded-md border border-line bg-canvas p-3">
					<div className="flex flex-col">
						<span className="label">{label}</span>
						<span className="text-2xl font-medium text-ink mt-1">{count}</span>
					</div>
				</li>
			))}
		</ul>
	);
}

const categoryColumns: Column<SiteSummary["by_category"][number]>[] = [
	{
		header: "Category",
		cell: (row) => (
			<span className="font-medium text-ink">
				{categoryLabel(row.category)}
			</span>
		),
	},
	{ header: "Open", numeric: true, cell: (row) => String(row.open) },
];

function TopSuggestions({ siteId, summary }: { siteId: string; summary: SiteSummary }) {
	if (summary.top.length === 0) {
		return (
			<div className="rounded-md border border-line bg-canvas p-6 text-center">
				<p className="text-sm font-medium text-ink-soft">
					{summary.score === null
						? "Suggestions appear once a crawl has read some pages."
						: "Nothing is open right now. Crawl again after you change the site to check it."}
				</p>
			</div>
		);
	}
	return (
		<ol aria-label="Top suggestions" className="divide-y divide-line">
			{summary.top.map((item, index) => (
				<li key={item.id} className="group py-3.5 first:pt-0 last:pb-0">
					<div className="flex items-start gap-3">
						<span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-hover text-xs font-medium text-ink-muted group-hover:bg-hover hover:underline transition-colors">
							{index + 1}
						</span>
						<div className="grid flex-1 gap-1.5">
							<Link
								to={`/suggestions/${item.id}?site=${siteId}`}
								className="text-sm font-medium text-ink hover:underline transition-colors leading-snug"
							>
								{item.title}
							</Link>
							<div className="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-xs text-ink-muted">
								<StatusTag tone={item.severity >= 3 ? "ink" : item.severity === 2 ? "open" : "line"}>
									{severityLabel(item.severity)}
								</StatusTag>
								<span className="inline-flex items-center gap-1 text-ink-soft font-medium">
									<svg viewBox="0 0 20 20" fill="currentColor" className="h-3.5 w-3.5 text-ink-muted" aria-hidden>
										<path fillRule="evenodd" d="M10 18a8 8 0 1 0 0-16 8 8 0 0 0 0 16Zm.75-13a.75.75 0 0 0-1.5 0v5c0 .414.336.75.75.75h4a.75.75 0 0 0 0-1.5h-3.25V5Z" clipRule="evenodd" />
									</svg>
									{effortLabel(item.effort)}
								</span>
								<span className="rounded-md bg-hover px-2 py-0.5 font-medium text-ink-soft">
									{item.open} open
								</span>
							</div>
						</div>
					</div>
				</li>
			))}
		</ol>
	);
}

/** The overview tab: score, counts, the top five suggestions, crawl facts and Crawl again. */
export function SiteOverview() {
	const { id = "" } = useParams();
	const client = useQueryClient();
	const summary = useSummary(id);
	const [startedId, setStartedId] = useState<string | null>(null);

	const latest = summary.data?.latest_crawl ?? null;
	const shownId = startedId ?? (latest && isCrawlActive(latest) ? latest.id : null);
	const shown = useCrawl(shownId, { poll: false });
	const crawling = shownId !== null && !shown.isError && (shown.data ? isCrawlActive(shown.data) : true);

	const refresh = useCallback(() => void client.invalidateQueries({ queryKey: keys.summary(id) }), [client, id]);

	if (summary.isPending) return <Loading label="Loading the overview…" />;
	if (summary.isError) return <ErrorState message={errorMessage(summary.error)} onRetry={() => void summary.refetch()} />;
	const data = summary.data;

	return (
		<div className="grid gap-6">
			<MetricCards siteId={id} />
			<div className="grid items-start gap-6 lg:grid-cols-[5fr_7fr] lg:gap-8">
			{/* Left Column: Health Score & Crawl Details */}
			<div className="grid content-start gap-6">
				<div className="rounded-md border border-line bg-raised p-4 flex flex-col gap-6">
					<div>
						<h2 className="text-xs font-medium text-ink-muted mb-3">Overall Health Score</h2>
						<ScoreBlock summary={data} crawling={crawling} />
					</div>
					<div className="pt-2 border-t border-line">
						<CrawlAgain siteId={id} busyWithCrawl={crawling} onStarted={setStartedId} />
					</div>
				</div>

				{shownId && (
					<div className="rounded-md border border-line bg-raised p-4">
						<CrawlStatus crawlId={shownId} onEnded={refresh} />
					</div>
				)}

				<div className="rounded-md border border-line bg-raised p-4 grid gap-4">
					<div className="flex items-center justify-between border-b border-line pb-3">
						<h2 className="text-[15px] font-medium text-ink">Crawl facts</h2>
						<span className="text-xs font-medium text-ink-muted">Latest crawl</span>
					</div>
					<Facts info={data.info} />
				</div>
			</div>

			{/* Right Column: Suggestions, Breakdown, Fix First */}
			<div className="grid content-start gap-6">
				<div className="rounded-md border border-line bg-raised p-4 grid gap-4">
					<div className="flex items-center justify-between border-b border-line pb-3">
						<h2 className="text-[15px] font-medium text-ink">Suggestions</h2>
						<Link to={`/sites/${id}/suggestions`} className="text-xs font-medium text-ink hover:underline transition-colors">
							View all →
						</Link>
					</div>
					<Counts counts={data.counts} />
				</div>

				<div className="rounded-md border border-line bg-raised p-4 grid gap-4">
					<div className="flex items-center justify-between border-b border-line pb-3">
						<h2 className="text-[15px] font-medium text-ink">Open by category</h2>
						<span className="text-xs text-ink-muted">Total issues</span>
					</div>
					<Table
						caption="Open suggestions by category"
						columns={categoryColumns}
						rows={data.by_category}
						rowKey={(row) => row.category}
					/>
				</div>

				<div className="rounded-md border border-line bg-raised p-4 grid gap-4">
					<div className="flex items-center justify-between border-b border-line pb-3">
						<div className="flex items-center gap-2">
							<h2 className="text-[15px] font-medium text-ink">Fix these first</h2>
							<span className="tag">
								Highest priority
							</span>
						</div>
						<Link to={`/sites/${id}/suggestions`} className="text-xs font-medium text-ink hover:underline transition-colors">
							Full list →
						</Link>
					</div>
					<TopSuggestions siteId={id} summary={data} />
				</div>
			</div>
			</div>
		</div>
	);
}
