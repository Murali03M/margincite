import { useEffect, useRef } from "react";
import { errorMessage } from "../api/client";
import { isCrawlActive, useCrawl } from "../api/hooks";
import { ErrorState, Loading } from "../ui/States";
import { endedSentence, pagesText } from "./crawlText";

type Props = {
	crawlId: string;
	/** Called once, when the crawl ends, so the screen can refresh its numbers. */
	onEnded?: () => void;
};

/** Progress of one crawl: polls while it runs, then says in one sentence how it ended. */
export function CrawlStatus({ crawlId, onEnded }: Props) {
	const crawl = useCrawl(crawlId);
	const data = crawl.data;
	const ended = Boolean(data) && !isCrawlActive(data);
	const told = useRef(false);

	useEffect(() => {
		if (ended && !told.current) {
			told.current = true;
			onEnded?.();
		}
	}, [ended, onEnded]);

	if (crawl.isPending) return <Loading label="Loading the crawl…" />;
	if (crawl.isError) return <ErrorState message={errorMessage(crawl.error)} onRetry={() => void crawl.refetch()} />;
	if (!data) return null;

	const isRunning = data.status === "running" || data.status === "queued";

	return (
		<div
			role="status"
			className="flex items-start gap-3 rounded-md border border-line bg-raised p-3 text-sm text-ink"
		>
			{isRunning ? (
				<svg className="mt-0.5 h-4 w-4 shrink-0 animate-spin text-ink-soft" fill="none" viewBox="0 0 24 24" aria-hidden>
					<circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
					<path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
				</svg>
			) : (
				<span className={`dot mt-2 ${data.error ? "dot-bad" : "dot-good"}`} aria-hidden />
			)}
			<div className="grid gap-1">
				{data.status === "queued" && <p className="font-medium">Waiting to start.</p>}
				{data.status === "running" && (
					<p className="font-medium">{pagesText(data.pages_crawled)} crawled so far.</p>
				)}
				{ended && (
					<>
						<p className="font-medium">{endedSentence(data)}</p>
						<p className="text-xs text-ink-soft">{pagesText(data.pages_crawled)} crawled.</p>
						{data.error && <p className="text-xs text-ink-soft mt-1">{data.error}</p>}
					</>
				)}
			</div>
		</div>
	);
}
