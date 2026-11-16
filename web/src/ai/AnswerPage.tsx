import { Link, useParams } from "react-router-dom";
import { ApiError, errorMessage } from "../api/client";
import { useAiAnswer } from "../api/hooks";
import type { AiAnswerDetail } from "../api/types";
import { NotFound } from "../ui/NotFound";
import { ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";
import { EXTERNAL_REL, safeHref } from "../ui/safeLink";
import { engineName } from "./engines";

/** A citation: a link only when the address is a safe http or https address. */
function Citation({ citation }: { citation: AiAnswerDetail["citations"][number] }) {
	const label = citation.title || citation.url;
	const href = safeHref(citation.url);
	return (
		<li className="grid gap-0.5 py-2.5 first:pt-0 last:pb-0">
			{href ? (
				<a href={href} rel={EXTERNAL_REL} target="_blank" className="break-all font-medium text-course-deep">
					{label}
				</a>
			) : (
				<span className="break-all font-medium text-ink-soft">{label}</span>
			)}
			{citation.domain && <span className="break-all text-xs text-ink-muted">{citation.domain}</span>}
		</li>
	);
}

/** One answer from one engine, as plain text, with what was cited and what went wrong. */
export function AnswerPage() {
	const { id = "", answerId = "" } = useParams();
	const answer = useAiAnswer(answerId);

	if (answer.isPending) return <Loading label="Loading the answer…" />;
	if (answer.isError) {
		if (answer.error instanceof ApiError && answer.error.status === 404) return <NotFound what="answer" />;
		return <ErrorState message={errorMessage(answer.error)} onRetry={() => void answer.refetch()} />;
	}
	const data = answer.data;

	return (
		<div className="mx-auto grid max-w-3xl gap-5">
			<Link to={`/sites/${id}/ai`} className="text-xs font-medium text-ink">
				← Back to AI Visibility
			</Link>
			<div className="grid gap-2">
				<h2 className="break-words text-[15px] font-medium text-ink">{data.prompt}</h2>
				<div className="flex flex-wrap items-center gap-2 text-xs text-ink-muted">
					<span className="font-medium text-ink-soft">{engineName(data.engine)}</span>
					<span>{data.model}</span>
					{data.cited && <StatusTag tone="course">{data.citation_rank ? `Cited, position ${data.citation_rank}` : "Cited"}</StatusTag>}
					{data.mentioned && <StatusTag tone="open">Mentioned</StatusTag>}
					{!data.cited && !data.mentioned && !data.error && <StatusTag>Not mentioned</StatusTag>}
				</div>
			</div>
			{data.error && (
				<p role="alert" className="notice">
					{data.error}
				</p>
			)}
			{data.notes.length > 0 && (
				<ul aria-label="Notes" className="grid gap-1 text-xs text-ink-soft">
					{data.notes.map((note) => (
						<li key={note}>{note}</li>
					))}
				</ul>
			)}
			{data.answer_text && (
				<section className="grid gap-2 rounded-md border border-line bg-raised p-5">
					<h3 className="text-sm font-medium text-ink">Answer</h3>
					<pre className="whitespace-pre-wrap break-words font-sans text-sm leading-relaxed text-ink">{data.answer_text}</pre>
				</section>
			)}
			{data.competitor_mentions.length > 0 && (
				<p className="text-sm text-ink-soft">Competitors named: {data.competitor_mentions.join(", ")}</p>
			)}
			<section className="grid gap-2 rounded-md border border-line bg-raised p-5">
				<h3 className="text-sm font-medium text-ink">Citations</h3>
				{data.citations.length === 0 ? (
					<p className="text-sm text-ink-muted">No citations.</p>
				) : (
					<ul aria-label="Citations" className="divide-y divide-line">
						{data.citations.map((citation) => (
							<Citation key={`${citation.url}|${citation.title ?? ""}`} citation={citation} />
						))}
					</ul>
				)}
			</section>
		</div>
	);
}
