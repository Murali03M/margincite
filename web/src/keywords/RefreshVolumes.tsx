import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { ApiError } from "../api/client";
import { useCheckWatch } from "../api/checkWatch";
import { keys } from "../api/hooks";
import { CHECK_POLL_MS, useRefreshVolumes, useVolumeStatus, volumesBusy } from "../api/researchHooks";
import { Button } from "../ui/Button";
import { shortDate } from "../ui/format";
import { Refusal } from "../ui/Missing";

/**
 * Asks for fresh search volumes and follows them: progress while the server works, the table refreshed
 * when it is done, and the server's reason when it failed.
 */
export function RefreshVolumes({ siteId, pollMs = CHECK_POLL_MS }: { siteId: string; pollMs?: number }) {
	const client = useQueryClient();
	const refresh = useRefreshVolumes(siteId);
	const watch = useCheckWatch();
	const { watching, timedOut, begin, end } = watch;
	const status = useVolumeStatus(siteId, !timedOut, pollMs);
	const data = status.data;
	const busy = volumesBusy(data);
	// After the time limit the server may still be working, but this screen stops following it.
	const working = busy && !timedOut;

	// A refresh already in progress, from a click or from before this screen opened, is followed too.
	useEffect(() => {
		if (busy && !watching && !timedOut) begin(null);
	}, [busy, watching, timedOut, begin]);

	useEffect(() => {
		if (watching && data && !busy) {
			end(data.status === "failed" ? data.error : null);
			void client.invalidateQueries({ queryKey: keys.keywords(siteId) });
		}
	}, [watching, data, busy, end, client, siteId]);

	async function go() {
		try {
			await refresh.mutateAsync();
		} catch (error) {
			// 409: one is already running, so follow it. Anything else is shown below.
			if (!(error instanceof ApiError && error.status === 409)) return;
		}
		const fresh = await status.refetch();
		if (fresh.data && !volumesBusy(fresh.data)) void client.invalidateQueries({ queryKey: keys.keywords(siteId) });
	}

	const failure = !busy && data?.status === "failed" ? data.error : null;
	return (
		<div className="grid justify-items-start gap-2">
			<Button variant="line" compact busy={refresh.isPending} disabled={working} onClick={() => void go()}>
				Refresh volumes
			</Button>
			{working && (
				<div role="status" className="grid w-64 gap-2 text-xs font-medium text-ink-soft">
					<p>Refreshing the search numbers. This usually takes a minute or two.</p>
					<progress aria-label="Check in progress" className="h-2 w-full" />
				</div>
			)}
			{timedOut && (
				<p className="notice">This is taking longer than usual. The refresh keeps running; the numbers appear in the table when it is done.</p>
			)}
			{failure && (
				<p role="alert" className="notice">
					The last refresh failed: {failure}
				</p>
			)}
			{!working && !failure && data?.checked_at && <p className="text-xs text-ink-muted">Refreshed {shortDate(data.checked_at)}.</p>}
			<Refusal error={refresh.error} />
		</div>
	);
}
