import { useState } from "react";
import { Link } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useDisconnectGoogle, useGoogle, useStartGoogle, useSyncGoogle } from "../api/hooks";
import type { GoogleStatus, Integrations } from "../api/types";
import { Button } from "../ui/Button";
import { shortDate } from "../ui/format";
import { ErrorState, Loading } from "../ui/States";
import { StatusTag } from "../ui/StatusTag";
import { safeHref } from "../ui/safeLink";
import { goTo } from "./navigate";
import { DisconnectButton, MissingSettings, Panel } from "./Panel";
import { PropertyChooser } from "./PropertyChooser";

/** Asks the server for Google's sign-in address, then sends the whole browser there. */
function ConnectButton({ siteId, ready }: { siteId: string; ready: boolean }) {
	const start = useStartGoogle(siteId);
	const [problem, setProblem] = useState<string | null>(null);

	async function connect() {
		setProblem(null);
		try {
			const { auth_url } = await start.mutateAsync();
			const href = safeHref(auth_url);
			if (!href) {
				setProblem("Margincite got an unexpected sign-in address from the server, so it did not open it. Try again.");
				return;
			}
			goTo(href);
		} catch (error) {
			setProblem(errorMessage(error));
		}
	}

	return (
		<div className="grid justify-items-start gap-2">
			<Button disabled={!ready} busy={start.isPending} onClick={() => void connect()}>
				Connect Google Search Console
			</Button>
			{problem && (
				<p role="alert" className="notice">
					{problem}
				</p>
			)}
		</div>
	);
}

function Connected({ siteId, status }: { siteId: string; status: GoogleStatus }) {
	const sync = useSyncGoogle(siteId);
	const disconnect = useDisconnectGoogle(siteId);
	const [changing, setChanging] = useState(false);

	return (
		<div className="grid gap-3">
			<p className="text-sm text-ink-soft">
				Property <span className="break-all font-medium">{status.property}</span>
			</p>
			<p className="text-xs text-ink-muted">
				{status.synced_at ? `Last synced ${shortDate(status.synced_at)}.` : "Not synced yet."} Numbers are for the last 28 days and are about three days late.
			</p>
			{changing ? (
				<PropertyChooser siteId={siteId} current={status.property} onDone={() => setChanging(false)} />
			) : (
				<div className="flex flex-wrap items-start gap-2">
					<Button compact busy={sync.isPending} onClick={() => sync.mutate()}>
						Sync now
					</Button>
					<Button variant="line" compact onClick={() => setChanging(true)}>
						Change property
					</Button>
					<DisconnectButton busy={disconnect.isPending} error={disconnect.error} onConfirm={() => disconnect.mutate()} />
				</div>
			)}
			{sync.isSuccess && <p role="status" className="text-xs font-medium text-ink-soft">The sync has started. New numbers appear in a minute or two.</p>}
			{sync.isError && (
				<p role="alert" className="notice">
					{errorMessage(sync.error)}
				</p>
			)}
		</div>
	);
}

/** Google Search Console: connect, choose the property, sync, disconnect. */
export function GooglePanel({ siteId, server }: { siteId: string; server: Integrations }) {
	const status = useGoogle(siteId);
	const ready = server.secret_key_set && server.google.configured;

	let body;
	if (status.isPending) body = <Loading label="Loading Google Search Console…" />;
	else if (status.isError) body = <ErrorState message={errorMessage(status.error)} onRetry={() => void status.refetch()} />;
	else if (status.data.status === "choose_property")
		body = <PropertyChooser siteId={siteId} current={status.data.property} onDone={() => undefined} />;
	else if (status.data.status === "connected") body = <Connected siteId={siteId} status={status.data} />;
	else
		body = (
			<div className="grid gap-3">
				{status.data.status === "needs_reconnecting" && (
					<p role="alert" className="notice">
						{status.data.error ?? "Google no longer accepts this connection. Connect Google Search Console again."}
					</p>
				)}
				<ConnectButton siteId={siteId} ready={ready} />
			</div>
		);


	return (
		<Panel title="Google Search Console">
			<div className="flex flex-wrap items-center gap-2">
				<StatusTag tone={status.data?.status === "connected" ? "course" : status.data?.status === "needs_reconnecting" ? "ink" : "line"}>
					{status.data ? STATUS_WORDS[status.data.status] : "Checking"}
				</StatusTag>
				<p className="text-xs text-ink-muted">Organic traffic, keywords and visibility, from Google.</p>
			</div>
			{!server.secret_key_set && <MissingSettings names={["MARGINCITE_SECRET_KEY"]} why="It protects the stored Google token." />}
			{!server.google.configured && (
				<p className="notice">
					Google sign-in is not set up yet. Add your Google client id and secret in{" "}
					<Link to="/settings" className="font-medium text-ink">
						Settings
					</Link>
					, then come back to connect.
				</p>
			)}
			{body}
		</Panel>
	);
}

const STATUS_WORDS: Record<GoogleStatus["status"], string> = {
	not_connected: "Not connected",
	choose_property: "Choose a property",
	connected: "Connected",
	needs_reconnecting: "Needs reconnecting",
};
