import { Link, useLocation, useParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useAllSuggestions, useChecks } from "../api/hooks";
import type { Category, CheckRow, CheckStatus } from "../api/types";
import { categoryLabel } from "../ui/labels";
import { Empty, ErrorState, Loading } from "../ui/States";
import { StatusTag, type Tone } from "../ui/StatusTag";
import { SpeedPanel } from "../speed/SpeedPanel";

const CATEGORY_ORDER: Category[] = ["technical", "ai_readiness", "content"];

const STATUS_WORD: Record<CheckStatus, string> = { passed: "Passed", failed: "Failed", not_run: "Not run" };
const STATUS_TONE: Record<CheckStatus, Tone> = { passed: "course", failed: "ink", not_run: "line" };

function Totals({ checks }: { checks: CheckRow[] }) {
	const count = (status: CheckStatus) => checks.filter((check) => check.status === status).length;
	return (
		<ul aria-label="Check totals" className="grid grid-cols-3 gap-3">
			{(["passed", "failed", "not_run"] as const).map((status) => (
				<li key={status} className="rounded-md border border-line bg-raised p-4">
					<div className="flex flex-col">
						<span className="label font-medium">{STATUS_WORD[status]}</span>
						<span className="mt-1 text-2xl font-medium text-ink">{count(status)}</span>
					</div>
				</li>
			))}
		</ul>
	);
}

function CheckItem({ check, siteId, suggestionId }: { check: CheckRow; siteId: string; suggestionId: string | undefined }) {
	return (
		<li className="grid gap-1 py-3 first:pt-0 last:pb-0">
			<div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
				<span className="min-w-0 break-words text-sm font-medium text-ink">{check.title}</span>
				<span className="flex items-center gap-2">
					{check.status === "failed" &&
						(suggestionId ? (
							<Link to={`/suggestions/${suggestionId}?site=${siteId}`} className="text-xs font-medium text-ink hover:underline">
								{check.open} open
							</Link>
						) : (
							<span className="text-xs font-medium text-ink-soft">{check.open} open</span>
						))}
					<StatusTag tone={STATUS_TONE[check.status]}>{STATUS_WORD[check.status]}</StatusTag>
				</span>
			</div>
			{check.status === "not_run" && check.reason && <p className="text-xs leading-relaxed text-ink-muted">{check.reason}</p>}
		</li>
	);
}

function ChecksList() {
	const { id = "" } = useParams();
	const checks = useChecks(id);
	const suggestions = useAllSuggestions(id);

	if (checks.isPending) return <Loading label="Loading the checks…" />;
	if (checks.isError) return <ErrorState message={errorMessage(checks.error)} onRetry={() => void checks.refetch()} />;

	const { crawl_id, checks: rows } = checks.data;
	if (rows.length === 0) {
		return <Empty title="No checks to show">The server listed no checks for this site. Try again in a moment.</Empty>;
	}
	const suggestionFor = new Map((suggestions.data ?? []).map((s) => [s.check_id, s.id]));

	return (
		<div className="grid gap-6">
			<div>
				<h2 className="text-[15px] font-medium text-ink">Checks</h2>
				<p className="mt-0.5 text-xs text-ink-muted">Everything Margincite checks, from the latest crawl</p>
			</div>
			{crawl_id === null && (
				<p className="notice">Results appear once a crawl has finished. Until then, checks are listed as not run.</p>
			)}
			<Totals checks={rows} />
			{CATEGORY_ORDER.map((category) => {
				const inCategory = rows.filter((check) => check.category === category);
				if (inCategory.length === 0) return null;
				return (
					<section key={category} className="grid gap-3 rounded-md border border-line bg-raised p-6">
						<h3 className="border-b border-line pb-3 text-sm font-medium text-ink">{categoryLabel(category)}</h3>
						<ul aria-label={`${categoryLabel(category)} checks`} className="divide-y divide-line">
							{inCategory.map((check) => (
								<CheckItem key={check.check_id} check={check} siteId={id} suggestionId={suggestionFor.get(check.check_id)} />
							))}
						</ul>
					</section>
				);
			})}
		</div>
	);
}

/** The address a link uses to open this screen with page speed on top. */
const SPEED_FIRST = "#page-speed";

/**
 * Every check, grouped by category: passed, failed (linking to its suggestion) and not run (with why), then page speed.
 * Opened at `#page-speed`, page speed comes first, so a link that says "Open Page speed" lands on it.
 */
export function ChecksPage() {
	const { id = "" } = useParams();
	const speedFirst = useLocation().hash === SPEED_FIRST;
	return (
		<div className="grid gap-6">
			{speedFirst && <SpeedPanel siteId={id} />}
			<ChecksList />
			{!speedFirst && <SpeedPanel siteId={id} />}
		</div>
	);
}
