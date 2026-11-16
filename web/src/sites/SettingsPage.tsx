import { useState } from "react";
import type { FormEvent } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useDeleteSite, useSite } from "../api/hooks";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { ErrorState, Loading } from "../ui/States";
import { SiteOptionsForm } from "./SiteOptionsForm";

/** Deleting a site. The button wakes only when the host is typed exactly. */
function DeleteSite() {
	const { id = "" } = useParams();
	const navigate = useNavigate();
	const site = useSite(id);
	const remove = useDeleteSite(id);
	const [typed, setTyped] = useState("");

	if (site.isPending) return <Loading label="Loading the settings…" />;
	if (site.isError) return <ErrorState message={errorMessage(site.error)} onRetry={() => void site.refetch()} />;
	const host = site.data.host;

	async function submit(event: FormEvent) {
		event.preventDefault();
		if (typed !== host) return;
		try {
			await remove.mutateAsync();
			navigate("/");
		} catch {
			// The error is shown from the mutation's state below.
		}
	}

	return (
		<div className="rounded-md border border-line bg-raised p-4 grid gap-4">
				<div className="flex items-center gap-2.5 border-b border-line pb-3">
					<div>
						<h2 className="text-[15px] font-medium text-ink">Delete this site</h2>
						<p className="flex items-center gap-1.5 text-xs text-ink-soft"><span className="dot dot-bad" aria-hidden />Danger zone</p>
					</div>
				</div>
				<p className="text-sm text-ink-soft leading-relaxed">
					Deleting <strong className="font-medium text-ink">{host}</strong> removes its crawls, pages and suggestions from Margincite. It does not change your website.
					This cannot be undone.
				</p>
				<form onSubmit={submit} className="grid gap-4 mt-2" noValidate>
					<Field
						label={`Type ${host} to confirm`}
						type="text"
						autoComplete="off"
						autoCapitalize="off"
						spellCheck={false}
						value={typed}
						onChange={(event) => setTyped(event.target.value)}
					/>
					<div>
						<Button
							type="submit"
							busy={remove.isPending}
							disabled={typed !== host}
						>
							{remove.isPending ? "Deleting…" : "Delete site"}
						</Button>
					</div>
					{remove.isError && (
						<p role="alert" className="notice">
							{errorMessage(remove.error)}
						</p>
					)}
				</form>
		</div>
	);
}

/** Settings of one site: its options, and deleting it. */
export function SettingsPage() {
	const { id = "" } = useParams();
	return (
		<div className="mx-auto grid max-w-3xl gap-6">
			<SiteOptionsForm siteId={id} />
			<DeleteSite />
		</div>
	);
}
