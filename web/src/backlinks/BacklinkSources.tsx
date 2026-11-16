import { errorMessage } from "../api/client";
import { useBacklinkSources } from "../api/hooks";
import { Button } from "../ui/Button";
import { CrawledLink } from "../ui/CrawledLink";
import { ErrorState, Loading } from "../ui/States";

/** The pages that link to one target, as Bing lists them. Text only; links go through CrawledLink. */
export function BacklinkSources({ siteId, target, onClose }: { siteId: string; target: string; onClose: () => void }) {
	const sources = useBacklinkSources(siteId, target);
	return (
		<section className="grid gap-3 rounded-md border border-line bg-raised p-5">
			<div className="flex flex-wrap items-center justify-between gap-3 border-b border-line pb-3">
				<h3 className="min-w-0 break-all text-sm font-medium text-ink">Pages linking to {target}</h3>
				<Button variant="line" compact onClick={onClose}>
					Close sources
				</Button>
			</div>
			{sources.isPending && <Loading label="Loading the sources…" />}
			{sources.isError && <ErrorState message={errorMessage(sources.error)} onRetry={() => void sources.refetch()} />}
			{sources.data && sources.data.length === 0 && (
				<p className="text-sm text-ink-soft">
					No sources are stored for this page. Only the 10 most linked pages have them, and only up to 100 links each.
				</p>
			)}
			{sources.data && sources.data.length > 0 && (
				<ul aria-label={`Pages linking to ${target}`} className="divide-y divide-line">
					{sources.data.map((source) => (
						<li key={`${source.source_url}|${source.anchor_text ?? ""}`} className="grid gap-0.5 py-2.5 first:pt-0 last:pb-0">
							<CrawledLink url={source.source_url} />
							{source.anchor_text && <span className="break-words text-xs text-ink-muted">{source.anchor_text}</span>}
						</li>
					))}
				</ul>
			)}
		</section>
	);
}
