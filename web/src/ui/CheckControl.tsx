import { useState } from "react";
import { Link } from "react-router-dom";
import { ApiError, errorMessage } from "../api/client";
import type { CheckWatch } from "../api/checkWatch";
import type { RunState } from "../api/types";
import { Button } from "./Button";
import { mentionsSettings } from "./Missing";

type Props = {
	/** The button's words, for example "Check now". */
	label: string;
	/** What is being checked, for the progress line: "Checking your keywords". */
	working: string;
	/** Sends the request that starts the check. Rejects with the server's 409, 422 and 429 messages. */
	start: () => Promise<unknown>;
	watch: CheckWatch;
	/** The `checked_at` the screen shows now. */
	checkedAt: string | null | undefined;
	/** The newest run the screen shows, when the API sends one. A run that ends after the click ends the watch. */
	lastRun?: RunState | null;
};

/**
 * "Check now" for something that runs in the background. The API gives no progress figure, so the
 * screen shows that it is working until the data's check date changes. A 409 means a check is
 * already running, so it is followed too. A 422 links to Settings only when its message says to go there.
 */
export function CheckControl({ label, working, start, watch, checkedAt, lastRun }: Props) {
	const [starting, setStarting] = useState(false);
	const [problem, setProblem] = useState<{ message: string; status: number } | null>(null);

	async function go() {
		setProblem(null);
		setStarting(true);
		try {
			await start();
			watch.begin(checkedAt, lastRun);
		} catch (error) {
			const status = error instanceof ApiError ? error.status : 0;
			if (status === 409) watch.begin(checkedAt, lastRun);
			setProblem({ message: errorMessage(error), status });
		} finally {
			setStarting(false);
		}
	}

	return (
		<div className="grid gap-3">
			<div>
				<Button busy={starting} disabled={watch.watching} onClick={() => void go()}>
					{starting ? "Starting…" : watch.watching ? "Checking…" : label}
				</Button>
			</div>
			{problem && (
				<p role="alert" className="notice">
					{problem.message}
					{problem.status === 422 && mentionsSettings(problem.message) && (
						<>
							{" "}
							<Link to="/settings" className="font-medium text-ink">
								Settings
							</Link>
						</>
					)}
				</p>
			)}
			{watch.failure && (
				<p role="alert" className="notice">
					The last check reported: {watch.failure}
				</p>
			)}
			{watch.watching && (
				<div role="status" className="grid gap-2 rounded-md border border-line bg-raised p-4 text-sm text-ink">
					<p className="font-medium">{working}. This usually takes a minute or two.</p>
					<progress aria-label="Check in progress" className="h-2 w-full" />
				</div>
			)}
			{watch.timedOut && (
				<p className="notice">This is taking longer than usual. The check keeps running; come back in a few minutes and reload.</p>
			)}
		</div>
	);
}
