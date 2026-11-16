import { useEffect, useRef, useState } from "react";
import type { RunState } from "./types";

/** How long a started check is watched before the screen says it is still working. */
export const WATCH_LIMIT_MS = 3 * 60 * 1000;

export type CheckWatch = {
	/** True from the moment a check was accepted until its results arrive or the time limit passes. */
	watching: boolean;
	/** True when the time limit passed first. The check itself keeps running on the server. */
	timedOut: boolean;
	/** Starts watching. Pass the `checked_at` the screen shows now and, when the API has one, its `last_run`. */
	begin: (checkedAt: string | null | undefined, lastRun?: RunState | null) => void;
	/** Stops watching. `failure` is the reason the run gave when it ended with an error. */
	end: (failure?: string | null) => void;
	/** The `checked_at` that was showing when watching began. */
	from: string | null;
	/** The `last_run` that was showing when watching began. */
	fromRun: RunState | null;
	/** Why the run that just ended failed, when it said. Cleared when the next check begins. */
	failure: string | null;
};

type Started = { at: string | null; run: RunState | null };

/**
 * Follows a check that runs in the background. The API has no progress figure, so a check counts as
 * finished when the data's `last_run` has ended, or, when the API sends none, when its `checked_at`
 * changes: pair this with `useWatchEnd`.
 */
export function useCheckWatch(): CheckWatch {
	const [from, setFrom] = useState<Started | null>(null);
	const [failure, setFailure] = useState<string | null>(null);
	const [timedOut, setTimedOut] = useState(false);
	const watching = from !== null;

	useEffect(() => {
		if (!watching) return;
		const timer = setTimeout(() => {
			setFrom(null);
			setTimedOut(true);
		}, WATCH_LIMIT_MS);
		return () => clearTimeout(timer);
	}, [watching]);

	return {
		watching,
		timedOut,
		from: from?.at ?? null,
		fromRun: from?.run ?? null,
		failure,
		begin: (checkedAt, lastRun) => {
			setTimedOut(false);
			setFailure(null);
			setFrom({ at: checkedAt ?? null, run: lastRun ?? null });
		},
		end: (reason) => {
			setFailure(reason ?? null);
			setFrom(null);
		},
	};
}

/** A run that has ended, and is not the run (in the state it was in) that the screen showed when the check began. */
function endedSince(run: RunState | null | undefined, before: RunState | null): run is RunState {
	if (!run || (run.status !== "completed" && run.status !== "failed")) return false;
	return run.id !== before?.id || run.finished_at !== before.finished_at;
}

/**
 * Ends the watch and calls `onDone` once the check is over: the screen's `last_run` has ended since the
 * watch began, or, without a `last_run`, its `checked_at` differs from the one it began with.
 */
export function useWatchEnd(watch: CheckWatch, checkedAt: string | null | undefined, onDone: () => void, lastRun?: RunState | null) {
	const done = useRef(onDone);
	done.current = onDone;
	const { watching, from, fromRun, end } = watch;
	const runEnded = watching && endedSince(lastRun, fromRun);
	const dateChanged = watching && checkedAt !== undefined && checkedAt !== from;
	const changed = runEnded || dateChanged;
	const failure = runEnded ? (lastRun?.error ?? null) : null;
	useEffect(() => {
		if (changed) {
			end(failure);
			done.current();
		}
	}, [changed, failure, end]);
}
