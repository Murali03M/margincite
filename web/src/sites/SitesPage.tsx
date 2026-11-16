import { Link } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useSites, useSummary } from "../api/hooks";
import type { Site } from "../api/types";
import { ButtonLink } from "../ui/Button";
import { Empty, ErrorState, Loading } from "../ui/States";
import { useGoogleReturn } from "../connections/returnNotice";
import { MetricCards } from "../metrics/MetricCards";
import { crawlWord } from "./crawlText";
import { FirstRun } from "./FirstRun";

/** The site's name, last crawl and open suggestions, from its summary. */
function CardFacts({ siteId }: { siteId: string }) {
	const summary = useSummary(siteId);
	if (summary.isPending) return <span className="text-xs text-ink-muted">…</span>;
	if (summary.isError) return <span className="text-xs text-ink-muted">Not loaded</span>;
	return (
		<span className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-ink-soft">
			<span>{crawlWord(summary.data.latest_crawl)}</span>
			<span className="inline-flex items-center rounded-md bg-raised px-2 py-0.5 font-medium text-ink-soft">
				{summary.data.counts.open} open suggestions
			</span>
		</span>
	);
}

/** One site: its name and the project card. */
function SiteCard({ site }: { site: Site }) {
	return (
		<article className="grid gap-4 rounded-md border border-line bg-raised p-5">
			<div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
				<Link to={`/sites/${site.id}`} className="break-all text-lg font-medium text-ink transition-colors hover:underline">
					{site.host}
				</Link>
				<CardFacts siteId={site.id} />
			</div>
			<MetricCards siteId={site.id} label={`Project card, ${site.host}`} />
		</article>
	);
}

/** The home screen: every site as a ruled row. */
export function SitesPage() {
	const sites = useSites();
	const returned = useGoogleReturn();
	const notice = returned?.kind === "error" && (
		<p role="alert" className="notice">
			{returned.message}
		</p>
	);

	if (sites.isPending) return <Loading label="Loading your sites…" />;
	if (sites.isError) return <ErrorState message={errorMessage(sites.error)} onRetry={() => void sites.refetch()} />;

	if (sites.data.length === 0) {
		return (
			<div className="grid gap-6">
				{notice}
				<FirstRun sites={sites.data} />
				<Empty title="Add your first site" action={<ButtonLink to="/sites/new">Add a site</ButtonLink>}>
					Margincite reads your site, then lists what to fix, in order. Add the address of your website to start
					the first crawl.
				</Empty>
			</div>
		);
	}

	return (
		<div className="grid gap-6">
			{notice}
			<FirstRun sites={sites.data} />
			<div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
				<div>
					<h1 className="text-lg font-medium text-ink">Your sites</h1>
					<p className="text-xs text-ink-muted mt-1">Websites monitored for classic SEO and AI answer engine (GEO) readiness</p>
				</div>
				<ButtonLink to="/sites/new">
					<svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4" aria-hidden>
						<path d="M10.75 4.75a.75.75 0 0 0-1.5 0v4.5h-4.5a.75.75 0 0 0 0 1.5h4.5v4.5a.75.75 0 0 0 1.5 0v-4.5h4.5a.75.75 0 0 0 0-1.5h-4.5v-4.5Z" />
					</svg>
					Add a site
				</ButtonLink>
			</div>
			<div className="grid gap-5">
				{sites.data.map((site) => (
					<SiteCard key={site.id} site={site} />
				))}
			</div>
		</div>
	);
}
