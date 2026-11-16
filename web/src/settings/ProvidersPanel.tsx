import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useRemoveProvider, useSaveProvider, useTestProvider } from "../api/settingsHooks";
import type { ProviderOut } from "../api/types";
import { engineName } from "../ai/engines";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Panel } from "../ui/Panel";
import { ConfirmRemove, Group, KeyState, Problem, TestResult } from "./parts";

/** One provider: its state, a write-only key field, a model field, and Save, Test and Remove. */
function ProviderRow({ provider, locked }: { provider: ProviderOut; locked: boolean }) {
	const name = engineName(provider.provider);
	const save = useSaveProvider(provider.provider);
	const test = useTestProvider(provider.provider);
	const remove = useRemoveProvider(provider.provider);
	const [key, setKey] = useState("");
	const [model, setModel] = useState(provider.model);
	const [problem, setProblem] = useState<string | null>(null);

	const modelChanged = model.trim() !== "" && model.trim() !== provider.model;
	const usesDefault = provider.model === provider.default_model;
	const canSave = key.trim() !== "" || modelChanged;

	async function submit(event: FormEvent) {
		event.preventDefault();
		const input = { ...(key.trim() ? { api_key: key.trim() } : {}), ...(modelChanged ? { model: model.trim() } : {}) };
		try {
			const saved = await save.mutateAsync(input);
			setProblem(null);
			setModel(saved.model);
			setKey("");
		} catch (error) {
			setProblem(errorMessage(error));
		} finally {
			// The mutation keeps the request it sent; drop it so the key is not held in memory.
			save.reset();
		}
	}

	/** An empty model tells the server to go back to its default. */
	async function useDefault() {
		try {
			const saved = await save.mutateAsync({ model: "" });
			setProblem(null);
			setModel(saved.model);
		} catch (error) {
			setProblem(errorMessage(error));
		} finally {
			save.reset();
		}
	}

	return (
		<Group title={name}>
			<TestResult status={provider.status} set={provider.key_set} error={provider.error} checkedAt={provider.checked_at} />
			<p className="text-xs text-ink-soft">
				API key: <KeyState set={provider.key_set} source={provider.source} />
			</p>
			<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-3 sm:grid-cols-2 sm:items-start">
				<Field
					label={`${name} API key`}
					type="password"
					autoComplete="off"
					spellCheck={false}
					value={key}
					disabled={locked}
					hint={provider.key_set ? "Enter a new key to replace the saved one. It is never shown." : "Paste the key from your provider's dashboard."}
					onChange={(event) => setKey(event.target.value)}
				/>
				<Field
					label={`${name} model`}
					autoComplete="off"
					value={model}
					hint={`Default: ${provider.default_model}`}
					onChange={(event) => setModel(event.target.value)}
				/>
				<div className="flex flex-wrap items-start gap-2 sm:col-span-2">
					<Button type="submit" compact busy={save.isPending} disabled={!canSave} aria-label={`Save ${name}`}>
						Save
					</Button>
					<Button variant="line" compact busy={test.isPending} disabled={!provider.key_set} aria-label={`Test ${name}`} onClick={() => test.mutate()}>
						Test
					</Button>
					{!usesDefault && (
						<Button variant="line" compact busy={save.isPending} aria-label={`Use default model for ${name}`} onClick={() => void useDefault()}>
							Use default
						</Button>
					)}
				</div>
			</form>
			<Problem error={problem ?? test.error} />
			{provider.source === "workspace" && (
				<ConfirmRemove what={`${name} key`} busy={remove.isPending} error={remove.error} onConfirm={() => remove.mutate()} />
			)}
		</Group>
	);
}

/** Claude, ChatGPT, Grok, Gemini and Perplexity: the engines AI Visibility asks about your site. */
export function ProvidersPanel({ providers, locked }: { providers: ProviderOut[]; locked: boolean }) {
	return (
		<Panel title="AI providers">
			<p className="text-sm leading-relaxed text-ink-soft">
				AI Visibility asks these assistants the questions your customers ask and notes whether they mention or cite your site. Add a key
				for any one of them to start; each extra key adds another engine. Saving tests the key with one small request, which your provider
				may bill you for.
			</p>
			<div className="grid gap-4">
				{providers.map((provider) => (
					<ProviderRow key={provider.provider} provider={provider} locked={locked} />
				))}
			</div>
		</Panel>
	);
}
