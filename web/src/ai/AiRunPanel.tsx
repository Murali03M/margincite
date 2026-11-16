import { useEffect, useRef, useState } from "react";
import { errorMessage } from "../api/client";
import { isRunActive, useAiRun, useAiRuns, useRefreshAfterRun, useStartRun } from "../api/hooks";
import type { AiRun } from "../api/types";
import { Button } from "../ui/Button";
import { shortDate } from "../ui/format";
import { engineName } from "./engines";

function Progress({ run }: { run: AiRun }) {
	const total = Math.max(1, run.answers_total);
	const asking = run.engines.map(engineName).join(", ");
	return (
		<div role="status" className="grid gap-2 rounded-md border border-line bg-raised p-4 text-sm text-ink">
			{run.status === "queued" ? (
				<p className="font-medium">Waiting to start.</p>
			) : (
				<p className="font-medium">
					{run.answers_done} of {run.answers_total} answers
					{asking ? ` from ${asking}` : ""}.
				</p>
			)}
			<progress aria-label="Run progress" value={run.answers_done} max={total} className="h-2 w-full" />
		</div>
	);
}

/** "Run now", the progress of a run while it goes, and a line about the last run. */
export function AiRunPanel({ siteId }: { siteId: string }) {
	const runs = useAiRuns(siteId);
	const start = useStartRun(siteId);
	const refresh = useRefreshAfterRun(siteId);
	const [startedId, setStartedId] = useState<string | null>(null);

	const latest = runs.data?.items[0];
	const activeId = startedId ?? (isRunActive(latest) ? latest?.id ?? null : null);
	const run = useAiRun(activeId);
	const active = activeId !== null && (run.data ? isRunActive(run.data) : !run.isError);

	const ended = Boolean(run.data) && !isRunActive(run.data);
	const told = useRef<string | null>(null);
	useEffect(() => {
		if (ended && activeId && told.current !== activeId) {
			told.current = activeId;
			refresh();
		}
	});

	async function go() {
		try {
			setStartedId((await start.mutateAsync()).id);
		} catch {
			void runs.refetch();
		}
	}

	const last = latest && !isRunActive(latest) ? latest : undefined;
	return (
		<div className="grid gap-4">
			<div className="flex flex-wrap items-center gap-3">
				<Button busy={start.isPending} disabled={active} onClick={() => void go()}>
					{start.isPending ? "Starting…" : active ? "Running…" : "Run now"}
				</Button>
				{last?.status === "completed" && last.finished_at && (
					<p className="text-xs text-ink-muted">Last run finished {shortDate(last.finished_at)}.</p>
				)}
			</div>
			{start.isError && (
				<p role="alert" className="notice">
					{errorMessage(start.error)}
				</p>
			)}
			{active && run.data && <Progress run={run.data} />}
			{last?.status === "failed" && (
				<p role="alert" className="notice">
					The last run failed. <span>{last.error ?? "No engine could answer."}</span>
				</p>
			)}
		</div>
	);
}
