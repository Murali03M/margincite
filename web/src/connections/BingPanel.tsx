import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useBing, useDisconnectBing, useSaveBing, useSite, useSyncBing } from "../api/hooks";
import type { BingStatus, Integrations } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { shortDate } from "../ui/format";
import { ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";
import { DisconnectButton, MissingSettings, Panel } from "./Panel";

/** The key is held in this component's state only while it is typed, and cleared once saved. */
function BingForm({ siteId, defaultUrl, ready }: { siteId: string; defaultUrl: string; ready: boolean }) {
	const save = useSaveBing(siteId);
	const [apiKey, setApiKey] = useState("");
	const [siteUrl, setSiteUrl] = useState(defaultUrl);
	const [problem, setProblem] = useState<string | null>(null);

	async function submit(event: FormEvent) {
		event.preventDefault();
		const key = apiKey.trim();
		const url = siteUrl.trim();
		if (!key || !url) {
			setProblem("Paste the API key and the site address first.");
			return;
		}
		setProblem(null);
		try {
			await save.mutateAsync({ api_key: key, site_url: url });
			setApiKey("");
		} catch (error) {
			setProblem(errorMessage(error));
		}
	}

	return (
		<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-4">
			<p className="text-xs leading-relaxed text-ink-muted">
				Find the key in Bing Webmaster Tools: Settings, then API access, then API key. The site address must be one of the sites in
				that Bing account.
			</p>
			<Field
				label="API key"
				type="password"
				autoComplete="off"
				spellCheck={false}
				value={apiKey}
				onChange={(event) => setApiKey(event.target.value)}
			/>
			<Field label="Site address" autoComplete="off" spellCheck={false} value={siteUrl} onChange={(event) => setSiteUrl(event.target.value)} />
			<div>
				<Button type="submit" disabled={!ready} busy={save.isPending}>
					{save.isPending ? "Checking with Bing…" : "Save"}
				</Button>
			</div>
			{problem && (
				<p role="alert" className="notice">
					{problem}
				</p>
			)}
		</form>
	);
}

function BingConnected({ siteId, status }: { siteId: string; status: BingStatus }) {
	const sync = useSyncBing(siteId);
	const disconnect = useDisconnectBing(siteId);
	return (
		<div className="grid gap-3">
			<p className="break-all text-sm font-medium text-ink-soft">{`Connected to ${status.site_url}`}</p>
			<p className="text-xs text-ink-muted">{status.synced_at ? `Last synced ${shortDate(status.synced_at)}.` : "Not synced yet."} Bing is checked once a week.</p>
			<div className="flex flex-wrap items-start gap-2">
				<Button compact busy={sync.isPending} onClick={() => sync.mutate()}>
					Sync now
				</Button>
				<DisconnectButton busy={disconnect.isPending} error={disconnect.error} onConfirm={() => disconnect.mutate()} />
			</div>
			{sync.isSuccess && <p role="status" className="text-xs font-medium text-ink-soft">The sync has started. New numbers appear in a minute or two.</p>}
			{sync.isError && (
				<p role="alert" className="notice">
					{errorMessage(sync.error)}
				</p>
			)}
		</div>
	);
}

/** Bing Webmaster Tools: paste an API key and the site address; sync and disconnect. */
export function BingPanel({ siteId, server }: { siteId: string; server: Integrations }) {
	const status = useBing(siteId);
	const site = useSite(siteId);

	let body;
	if (status.isPending || site.isPending) body = <Loading label="Loading Bing Webmaster Tools…" />;
	else if (status.isError) body = <ErrorState message={errorMessage(status.error)} onRetry={() => void status.refetch()} />;
	else
		body = (
			<>
				{status.data.status !== "not_connected" && status.data.site_url && <BingConnected siteId={siteId} status={status.data} />}
				{status.data.status === "needs_reconnecting" && (
					<p role="alert" className="notice">
						{status.data.error ?? "Bing no longer accepts this API key. Paste a new one."}
					</p>
				)}
				<BingForm siteId={siteId} defaultUrl={status.data.site_url ?? site.data?.root_url ?? ""} ready={server.secret_key_set} />
			</>
		);

	return (
		<Panel title="Bing Webmaster Tools">
			<div className="flex flex-wrap items-center gap-2">
				<StatusTag tone={status.data?.status === "connected" ? "course" : status.data?.status === "needs_reconnecting" ? "ink" : "line"}>
					{status.data ? BING_WORDS[status.data.status] : "Checking"}
				</StatusTag>
				<p className="text-xs text-ink-muted">Backlinks, from Bing.</p>
			</div>
			{!server.secret_key_set && <MissingSettings names={["MARGINCITE_SECRET_KEY"]} why="It protects the stored API key." />}
			{body}
		</Panel>
	);
}

const BING_WORDS: Record<BingStatus["status"], string> = {
	not_connected: "Not connected",
	connected: "Connected",
	needs_reconnecting: "Needs reconnecting",
};
