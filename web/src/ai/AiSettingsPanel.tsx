import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useAiSettings, useSaveAiSettings } from "../api/hooks";
import type { AiSettings } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Panel } from "../ui/Panel";
import { ErrorState, Loading } from "../ui/States";

const list = (text: string): string[] =>
	text
		.split(/[,\n]/)
		.map((part) => part.trim())
		.filter(Boolean);

function SettingsForm({ siteId, initial }: { siteId: string; initial: AiSettings }) {
	const save = useSaveAiSettings(siteId);
	const [brand, setBrand] = useState(initial.brand_name);
	const [aliases, setAliases] = useState(initial.aliases.join(", "));
	const [competitors, setCompetitors] = useState(initial.competitors.join(", "));

	async function submit(event: FormEvent) {
		event.preventDefault();
		try {
			const saved = await save.mutateAsync({ brand_name: brand.trim(), aliases: list(aliases), competitors: list(competitors) });
			setBrand(saved.brand_name);
			setAliases(saved.aliases.join(", "));
			setCompetitors(saved.competitors.join(", "));
		} catch {
			// The refusal is shown from the mutation's state below.
		}
	}

	return (
		<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-4">
			<Field label="Brand name" value={brand} maxLength={100} onChange={(event) => setBrand(event.target.value)} />
			<Field
				label="Other names (separated by commas)"
				hint="Up to 10. An answer that uses any of them counts as a mention."
				value={aliases}
				onChange={(event) => setAliases(event.target.value)}
			/>
			<Field
				label="Competitor domains (separated by commas)"
				hint="Up to 10, such as rival.com. Margincite notes when they are mentioned or cited."
				value={competitors}
				onChange={(event) => setCompetitors(event.target.value)}
			/>
			<div className="flex flex-wrap items-center gap-3">
				<Button type="submit" busy={save.isPending}>
					Save settings
				</Button>
				{save.isSuccess && <p role="status" className="text-xs font-medium text-ink-soft">Saved.</p>}
			</div>
			{save.isError && (
				<p role="alert" className="notice">
					{errorMessage(save.error)}
				</p>
			)}
		</form>
	);
}

/** The brand, its other names and the competitors the AI answers are read for. */
export function AiSettingsPanel({ siteId }: { siteId: string }) {
	const settings = useAiSettings(siteId);
	return (
		<Panel title="Brand and competitors">
			{settings.isPending && <Loading label="Loading the settings…" />}
			{settings.isError && <ErrorState message={errorMessage(settings.error)} onRetry={() => void settings.refetch()} />}
			{settings.data && <SettingsForm siteId={siteId} initial={settings.data} />}
		</Panel>
	);
}
