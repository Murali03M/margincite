import { useParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useMetrics } from "../api/hooks";
import type { Metrics } from "../api/types";
import { describeChange } from "../metrics/tiles";
import { ButtonLink } from "../ui/Button";
import { formatNumber } from "../ui/format";
import { Empty, ErrorState, Loading } from "../ui/States";
import { SearchTables } from "./SearchTables";
import { TrafficChart } from "./TrafficChart";

function Totals({ organic }: { organic: Metrics["organic"] }) {
	const clicks = describeChange(organic.clicks ?? 0, organic.clicks_previous, "vs the previous 28 days");
	const keywords = describeChange(organic.keywords ?? 0, organic.keywords_previous, "vs the previous 28 days");
	const rows = [
		{ label: "Clicks", value: formatNumber(organic.clicks ?? 0), change: clicks },
		{ label: "Impressions", value: formatNumber(organic.impressions ?? 0), change: null },
		{ label: "Keywords", value: formatNumber(organic.keywords ?? 0), change: keywords },
		{ label: "Average position", value: organic.avg_position === null ? "–" : organic.avg_position.toFixed(1), change: null },
	];
	return (
		<ul aria-label="Search totals" className="grid grid-cols-2 gap-3 sm:grid-cols-4">
			{rows.map((row) => (
				<li key={row.label} className="flex flex-col gap-1 rounded-md border border-line bg-raised p-4">
					<span className="label font-medium">{row.label}</span>
					<span className="text-2xl font-medium text-ink">{row.value}</span>
					{row.change && (
						<span className={`text-xs font-medium ${row.change.direction === "up" ? "text-ink-soft" : row.change.direction === "down" ? "text-ink-soft" : "text-ink-muted"}`}>
							{row.change.text}
						</span>
					)}
				</li>
			))}
		</ul>
	);
}

/** Search Console numbers: totals, daily charts for up to 90 days, top queries and top pages. */
export function SearchPage() {
	const { id = "" } = useParams();
	const metrics = useMetrics(id);

	if (metrics.isPending) return <Loading label="Loading the search numbers…" />;
	if (metrics.isError) return <ErrorState message={errorMessage(metrics.error)} onRetry={() => void metrics.refetch()} />;
	const { organic } = metrics.data;

	if (organic.clicks === null) {
		return (
			<Empty title="No search numbers yet" action={<ButtonLink to={`/sites/${id}/connections`}>Open Connections</ButtonLink>}>
				{organic.reason ?? "Connect Google Search Console to see how your site does in Google."}
			</Empty>
		);
	}

	return (
		<div className="grid gap-6">
			<div>
				<h2 className="text-[15px] font-medium text-ink">Search</h2>
				<p className="mt-0.5 text-xs text-ink-muted">From Google Search Console. Totals cover the last 28 days; the charts show up to 90 days. Data is about three days late.</p>
			</div>
			<Totals organic={organic} />
			<div className="grid gap-4 lg:grid-cols-2">
				<TrafficChart title="Clicks" daily={organic.daily.map((d) => ({ date: d.date, value: d.clicks }))} color="#ececee" />
				<TrafficChart title="Impressions" daily={organic.daily.map((d) => ({ date: d.date, value: d.impressions }))} color="#a0a0a8" />
			</div>
			<SearchTables siteId={id} />
		</div>
	);
}
