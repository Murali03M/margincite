import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useRemovePageSpeed, useSavePageSpeed } from "../api/settingsHooks";
import type { Settings } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Panel } from "../ui/Panel";
import { ConfirmRemove, KeyState, Problem } from "./parts";

/** The Google PageSpeed key. Without one, checks rely on Google's shared allowance, which is usually used up. */
export function PageSpeedPanel({ pagespeed, locked }: { pagespeed: Settings["pagespeed"]; locked: boolean }) {
	const save = useSavePageSpeed();
	const remove = useRemovePageSpeed();
	const [key, setKey] = useState("");
	const [problem, setProblem] = useState<string | null>(null);

	async function submit(event: FormEvent) {
		event.preventDefault();
		try {
			await save.mutateAsync({ api_key: key.trim() });
			setProblem(null);
			setKey("");
		} catch (error) {
			setProblem(errorMessage(error));
		} finally {
			save.reset();
		}
	}

	return (
		<Panel title="Page speed">
			<p className="text-sm leading-relaxed text-ink-soft">
				Page speed checks use Google PageSpeed Insights. Add a key: Google shares one small allowance between everyone who has none, and
				it is usually used up, so checks without a key tend to fail. The key is free from Google Cloud.
			</p>
			<p className="text-xs text-ink-soft">
				API key: <KeyState set={pagespeed.key_set} source={pagespeed.source} />
			</p>
			<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
				<Field
					label="PageSpeed API key"
					type="password"
					autoComplete="off"
					spellCheck={false}
					value={key}
					disabled={locked}
					onChange={(event) => setKey(event.target.value)}
				/>
				<Button type="submit" compact busy={save.isPending} disabled={!key.trim()} aria-label="Save PageSpeed key">
					Save
				</Button>
			</form>
			<Problem error={problem} />
			{pagespeed.source === "workspace" && (
				<ConfirmRemove what="PageSpeed key" busy={remove.isPending} error={remove.error} onConfirm={() => remove.mutate()} />
			)}
		</Panel>
	);
}
