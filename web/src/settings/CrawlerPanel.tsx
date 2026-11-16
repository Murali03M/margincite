import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useRemoveCrawler, useSaveCrawler } from "../api/settingsHooks";
import type { Settings } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Panel } from "../ui/Panel";
import { ConfirmRemove } from "./parts";

/** The contact address the crawler gives site owners. Not a secret, so it is shown and can be edited. */
export function CrawlerPanel({ crawler }: { crawler: Settings["crawler"] }) {
	const save = useSaveCrawler();
	const remove = useRemoveCrawler();
	const [contact, setContact] = useState(crawler.contact ?? "");
	const [problem, setProblem] = useState<string | null>(null);
	const [saved, setSaved] = useState(false);

	async function submit(event: FormEvent) {
		event.preventDefault();
		setSaved(false);
		try {
			await save.mutateAsync({ contact: contact.trim() });
			setProblem(null);
			setSaved(true);
		} catch (error) {
			setProblem(errorMessage(error));
		}
	}

	return (
		<Panel title="Crawler">
			<p className="text-sm leading-relaxed text-ink-soft">
				Crawling needs a contact address, an email or a web address. The crawler shows it to site owners in its user agent, so they know
				who is visiting and can reach you if it causes a problem.
			</p>
			<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
				<Field
					label="Contact address"
					autoComplete="off"
					placeholder="you@example.com"
					value={contact}
					error={problem}
					onChange={(event) => setContact(event.target.value)}
				/>
				<Button type="submit" compact busy={save.isPending} disabled={!contact.trim()} aria-label="Save contact">
					Save
				</Button>
			</form>
			{saved && (
				<p role="status" className="text-xs font-medium text-ink-soft">
					Saved
				</p>
			)}
			{crawler.contact_source === "workspace" && (
				<ConfirmRemove
					what="crawler contact"
					busy={remove.isPending}
					error={remove.error}
					onConfirm={() =>
						remove.mutate(undefined, {
							onSuccess: () => {
								setContact("");
								setSaved(false);
							},
						})
					}
				/>
			)}
			{crawler.contact && crawler.contact_source === "environment" && (
				<p className="text-xs text-ink-muted">The current address comes from the server file. Saving here replaces it.</p>
			)}
		</Panel>
	);
}
