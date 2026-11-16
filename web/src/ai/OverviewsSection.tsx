import { errorMessage } from "../api/client";
import { useAiOverviews } from "../api/researchHooks";
import { useSettings } from "../api/settingsHooks";
import type { AiOverviewRow } from "../api/types";
import { overviewWord } from "../rankings/words";
import { ButtonLink } from "../ui/Button";
import { CrawledLink } from "../ui/CrawledLink";
import { Panel } from "../ui/Panel";
import { shortDate } from "../ui/format";
import { ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";
import { Table, type Column } from "../ui/Table";

const NONE = "–";

const columns: Column<AiOverviewRow>[] = [
	{ header: "Keyword", cell: (row) => <span className="break-words font-medium text-ink">{row.keyword}</span> },
	{
		header: "AI Overview",
		cell: (row) => {
			const { word, tone } = overviewWord(row.status);
			return <StatusTag tone={tone}>{word}</StatusTag>;
		},
	},
	{
		header: "Domains it cites",
		cell: (row) => (row.cited_domains.length ? <span className="break-words">{row.cited_domains.join(", ")}</span> : <span className="text-ink-muted">{NONE}</span>),
	},
	{
		header: "Your pages it cites",
		cell: (row) =>
			row.your_urls.length ? (
				<ul className="grid gap-1">
					{row.your_urls.map((url) => (
						<li key={url}>
							<CrawledLink url={url} />
						</li>
					))}
				</ul>
			) : (
				<span className="text-ink-muted">{NONE}</span>
			),
	},
	{ header: "Checked", cell: (row) => (row.checked_at ? shortDate(row.checked_at) : NONE) },
];

function Body({ siteId }: { siteId: string }) {
	const overviews = useAiOverviews(siteId, true);
	if (overviews.isPending) return <Loading label="Loading the AI Overviews…" />;
	if (overviews.isError) return <ErrorState message={errorMessage(overviews.error)} onRetry={() => void overviews.refetch()} />;
	if (overviews.data.length === 0) {
		return (
			<div className="grid justify-items-start gap-3">
				<p className="text-sm text-ink-soft">Not checked yet. AI Overviews are looked up when your rankings are checked.</p>
				<ButtonLink to={`/sites/${siteId}/rankings`} variant="line" compact>
					Open Rankings
				</ButtonLink>
			</div>
		);
	}
	return <Table caption="AI Overviews by keyword" columns={columns} rows={overviews.data} rowKey={(row) => row.keyword_id} />;
}

/** Google's AI Overview for each tracked keyword: is there one, and does it cite the site? */
export function OverviewsSection({ siteId }: { siteId: string }) {
	const settings = useSettings();
	const ready = Boolean(settings.data?.dataforseo.login_set && settings.data.dataforseo.password_set);

	let body;
	if (settings.isPending) body = <Loading label="Loading the settings…" />;
	else if (settings.isError) body = <ErrorState message={errorMessage(settings.error)} onRetry={() => void settings.refetch()} />;
	else if (!ready) {
		body = (
			<div className="grid justify-items-start gap-3">
				<p className="text-sm text-ink-soft">
					Google's AI Overview is the AI answer at the top of some searches. To see whether it appears for your keywords and whether it cites
					you, add search data (DataForSEO) in Settings.
				</p>
				<ButtonLink to="/settings" variant="line" compact>
					Open Settings
				</ButtonLink>
			</div>
		);
	} else body = <Body siteId={siteId} />;

	return (
		<Panel title="AI Overviews">
			<p className="text-xs text-ink-muted">Whether Google's AI answer appears for each keyword you track, and which sites it cites.</p>
			{body}
		</Panel>
	);
}
