import { errorMessage } from "../api/client";
import { useAiDomains, useAiRuns, useRunAnswers } from "../api/hooks";
import type { AiAnswer, AiDomain, EngineId } from "../api/types";
import { Link } from "react-router-dom";
import { Empty, ErrorState, Loading } from "../ui/States";
import { StatusTag, type Tone } from "../ui/StatusTag";
import { Table, type Column } from "../ui/Table";
import { ENGINES, engineName } from "./engines";

type Row = { promptId: string; prompt: string; byEngine: Map<EngineId, AiAnswer> };

function verdict(answer: AiAnswer): { word: string; tone: Tone } {
	if (answer.error) return { word: "Error", tone: "ink" };
	if (answer.cited) return { word: "Cited", tone: "course" };
	if (answer.mentioned) return { word: "Mentioned", tone: "open" };
	return { word: "Not mentioned", tone: "line" };
}

function rowsOf(answers: AiAnswer[]): Row[] {
	const rows = new Map<string, Row>();
	for (const answer of answers) {
		const row = rows.get(answer.prompt_id) ?? { promptId: answer.prompt_id, prompt: answer.prompt, byEngine: new Map() };
		row.byEngine.set(answer.engine, answer);
		rows.set(answer.prompt_id, row);
	}
	return [...rows.values()];
}

function columnsOf(siteId: string, engines: EngineId[]): Column<Row>[] {
	return [
		{ header: "Question", cell: (row) => <span className="break-words font-medium text-ink">{row.prompt}</span> },
		...engines.map((engine) => ({
			header: engineName(engine),
			cell: (row: Row) => {
				const answer = row.byEngine.get(engine);
				if (!answer) return <span className="text-ink-muted">–</span>;
				const { word, tone } = verdict(answer);
				return (
					<Link to={`/sites/${siteId}/ai/answers/${answer.id}`} aria-label={`${engineName(engine)} answer: ${word}`}>
						<StatusTag tone={tone}>{word}</StatusTag>
					</Link>
				);
			},
		})),
	];
}

function Grid({ siteId, runId, runEngines }: { siteId: string; runId: string; runEngines: EngineId[] }) {
	const answers = useRunAnswers(runId);
	if (answers.isPending) return <Loading label="Loading the answers…" />;
	if (answers.isError) return <ErrorState message={errorMessage(answers.error)} onRetry={() => void answers.refetch()} />;
	const engines = ENGINES.map((e) => e.id).filter((id) => runEngines.includes(id) || answers.data.some((a) => a.engine === id));
	return (
		<Table
			caption="Answers by question and engine"
			columns={columnsOf(siteId, engines)}
			rows={rowsOf(answers.data)}
			rowKey={(row) => row.promptId}
		/>
	);
}

const domainColumns: Column<AiDomain>[] = [
	{ header: "Domain", cell: (row) => <span className="break-all font-medium text-ink">{row.domain}</span> },
	{ header: "Citations", numeric: true, cell: (row) => String(row.citations) },
	{ header: "Answers", numeric: true, cell: (row) => String(row.answers) },
	{
		header: "Who",
		cell: (row) =>
			row.is_you ? <StatusTag tone="course">You</StatusTag> : row.is_competitor ? <StatusTag tone="open">Competitor</StatusTag> : null,
	},
];

function Domains({ siteId }: { siteId: string }) {
	const domains = useAiDomains(siteId);
	if (domains.isPending) return <Loading label="Loading the cited domains…" />;
	if (domains.isError) return <ErrorState message={errorMessage(domains.error)} onRetry={() => void domains.refetch()} />;
	if (domains.data.length === 0) return <p className="text-sm text-ink-muted">No sources were cited in the latest run.</p>;
	return <Table caption="Most cited domains" columns={domainColumns} rows={domains.data} rowKey={(row) => row.domain} />;
}

/** The latest completed run: a grid of questions by engines, and the most cited domains. */
export function AiResults({ siteId }: { siteId: string }) {
	const runs = useAiRuns(siteId);
	if (runs.isPending) return <Loading label="Loading the results…" />;
	if (runs.isError) return <ErrorState message={errorMessage(runs.error)} onRetry={() => void runs.refetch()} />;
	const done = runs.data.items.find((run) => run.status === "completed");
	if (!done) {
		return <Empty title="No results yet">Add some questions, then press Run now to ask the AI engines and see whether they mention or cite your site.</Empty>;
	}
	return (
		<div className="grid gap-6">
			<Grid siteId={siteId} runId={done.id} runEngines={done.engines} />
			<div className="grid gap-3">
				<h3 className="text-sm font-medium text-ink">Most cited domains</h3>
				<Domains siteId={siteId} />
			</div>
		</div>
	);
}
