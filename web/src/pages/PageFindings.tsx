import { Link } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useAllSuggestions, usePageFindings } from "../api/hooks";
import { evidenceLines } from "../suggestions/evidence";
import { categoryLabel, severityLabel } from "../ui/labels";
import { ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";

function evidenceOf(evidence: unknown): string[] {
	return evidence && typeof evidence === "object" && !Array.isArray(evidence)
		? evidenceLines(evidence as Record<string, unknown>)
		: [];
}

/** What the checks found on one page, each with a link to its suggestion when there is one. */
export function PageFindings({ pageId, siteId }: { pageId: string; siteId: string | null }) {
	const findings = usePageFindings(pageId);
	const suggestions = useAllSuggestions(siteId);

	if (findings.isPending) return <Loading label="Loading the findings…" />;
	if (findings.isError) return <ErrorState message={errorMessage(findings.error)} onRetry={() => void findings.refetch()} />;
	if (findings.data.length === 0) return <p className="text-sm text-ink-muted">No findings against this page.</p>;

	const idByCheck = new Map((suggestions.data ?? []).map((item) => [item.check_id, item.id]));
	return (
		<ul aria-label="Findings on this page" className="divide-y divide-line">
			{findings.data.map((finding, index) => {
				const suggestionId = idByCheck.get(finding.check_id);
				return (
					<li key={`${finding.check_id}-${index}`} className="grid gap-2 py-3.5 first:pt-0 last:pb-0">
						<p className="font-medium text-sm text-ink">{finding.title}</p>
						<p className="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-xs text-ink-muted">
							<StatusTag tone={finding.severity >= 3 ? "ink" : finding.severity === 2 ? "open" : "line"}>
								{severityLabel(finding.severity)}
							</StatusTag>
							<span className="rounded-md bg-hover px-2 py-0.5 font-medium text-ink-soft">
								{categoryLabel(finding.category)}
							</span>
						</p>
						{evidenceOf(finding.evidence).length > 0 && (
							<ul className="grid gap-1 mt-1 text-xs">
								{evidenceOf(finding.evidence).map((line, lineIndex) => (
									<li key={`${lineIndex}-${line}`} className="rounded-md bg-canvas px-2 py-1 mono text-[11px] text-ink-soft border border-line">
										{line}
									</li>
								))}
							</ul>
						)}
						{suggestionId && (
							<Link
								to={`/suggestions/${suggestionId}${siteId ? `?site=${siteId}` : ""}`}
								className="text-xs font-medium text-ink hover:underline transition-colors inline-flex items-center gap-1 mt-0.5"
							>
								See the suggestion →
							</Link>
						)}
					</li>
				);
			})}
		</ul>
	);
}
