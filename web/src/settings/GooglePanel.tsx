import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useRemoveGoogle, useSaveGoogle } from "../api/settingsHooks";
import type { Settings } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Panel } from "../ui/Panel";
import { EXTERNAL_REL } from "../ui/safeLink";
import { ConfirmRemove, KeyState, Problem } from "./parts";

/**
 * The address Google sends the browser back to, as the server gives it. An older backend does not send it,
 * so then it is worked out from this page's address, which is right only when the app and API share one.
 */
export function redirectAddress(google: Settings["google"]): string {
	return google.redirect_uri || `${window.location.origin}/api/v1/oauth/google/callback`;
}

function CopyAddress({ address }: { address: string }) {
	const [state, setState] = useState<"idle" | "copied" | "failed">("idle");

	async function copy() {
		try {
			await navigator.clipboard.writeText(address);
			setState("copied");
		} catch {
			setState("failed");
		}
	}

	return (
		<div className="grid gap-2">
			<div className="flex flex-wrap items-center gap-2">
				<code className="break-all rounded-md border border-line bg-canvas px-3 py-2 text-sm font-medium text-ink">{address}</code>
				<Button variant="line" compact aria-label="Copy the redirect address" onClick={() => void copy()}>
					Copy
				</Button>
			</div>
			{state === "copied" && (
				<p role="status" className="text-xs font-medium text-ink-soft">
					Copied
				</p>
			)}
			{state === "failed" && (
				<p role="alert" className="notice">
					Your browser would not copy it. Select the address above and copy it by hand.
				</p>
			)}
		</div>
	);
}

const STEPS = [
	"In the Google Cloud console, create a project (or pick one) and turn on the Search Console API for it.",
	"Open APIs and Services, then Credentials, and create an OAuth client ID of the type Web application.",
	"Under Authorized redirect URIs, add the redirect address shown above, exactly as written.",
	"Copy the client id and client secret Google shows you into the fields below and press Save.",
	"Open a site, go to its Connections tab and press Connect Google Search Console.",
];

/** The Google client id and secret that let a site connect Search Console. Write-only. */
export function GooglePanel({ google, locked }: { google: Settings["google"]; locked: boolean }) {
	const save = useSaveGoogle();
	const remove = useRemoveGoogle();
	const [id, setId] = useState("");
	const [secret, setSecret] = useState("");
	const [problem, setProblem] = useState<string | null>(null);
	const set = google.client_id_set && google.client_secret_set;

	async function submit(event: FormEvent) {
		event.preventDefault();
		try {
			await save.mutateAsync({ client_id: id.trim(), client_secret: secret.trim() });
			setProblem(null);
			setId("");
			setSecret("");
		} catch (error) {
			setProblem(errorMessage(error));
		} finally {
			save.reset();
		}
	}

	return (
		<Panel title="Google Search Console sign-in">
			<p className="text-sm leading-relaxed text-ink-soft">
				This lets each site connect to Google Search Console, which unlocks organic traffic, search keywords and the Search page.
				You create the sign-in once in your own Google Cloud account.
			</p>
			<div className="grid gap-2">
				<h3 className="text-sm font-medium text-ink">Redirect address to register</h3>
				<CopyAddress address={redirectAddress(google)} />
			</div>
			<ol aria-label="Steps" className="grid list-decimal gap-1.5 pl-5 text-sm leading-relaxed text-ink-soft">
				{STEPS.map((step) => (
					<li key={step}>{step}</li>
				))}
			</ol>
			<p className="text-xs text-ink-muted">
				<a href="https://console.cloud.google.com/apis/credentials" target="_blank" rel={EXTERNAL_REL} className="font-medium text-ink">
					Open Google Cloud credentials
				</a>
			</p>
			<p className="text-xs text-ink-soft">
				Client id and secret: <KeyState set={set} source={google.source} />
			</p>
			<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-3 sm:grid-cols-2 sm:items-start">
				<Field label="Google client id" autoComplete="off" spellCheck={false} value={id} disabled={locked} onChange={(event) => setId(event.target.value)} />
				<Field
					label="Google client secret"
					type="password"
					autoComplete="off"
					spellCheck={false}
					value={secret}
					disabled={locked}
					onChange={(event) => setSecret(event.target.value)}
				/>
				<div className="sm:col-span-2">
					<Button type="submit" compact busy={save.isPending} disabled={!id.trim() || !secret.trim()} aria-label="Save Google client">
						Save
					</Button>
				</div>
			</form>
			<Problem error={problem} />
			{google.source === "workspace" && (
				<ConfirmRemove what="Google client" busy={remove.isPending} error={remove.error} onConfirm={() => remove.mutate()} />
			)}
		</Panel>
	);
}
