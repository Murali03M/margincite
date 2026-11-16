import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useAddPrompt, useAiPrompts, useDeletePrompt, useSuggestPrompts } from "../api/hooks";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Panel } from "../ui/Panel";
import { ErrorState, Loading } from "../ui/States";

const LIMIT = 25;

function Suggestions({ siteId, shown, onShown }: { siteId: string; shown: string[]; onShown: (next: string[]) => void }) {
	const add = useAddPrompt(siteId);
	const [problem, setProblem] = useState<string | null>(null);

	async function take(text: string) {
		setProblem(null);
		try {
			await add.mutateAsync(text);
			onShown(shown.filter((item) => item !== text));
		} catch (error) {
			setProblem(errorMessage(error));
		}
	}

	return (
		<div className="grid gap-2 rounded-md border border-line bg-raised p-4">
			<p className="text-xs font-medium text-ink">Suggested by Claude. Nothing is saved until you add it.</p>
			<ul aria-label="Suggested questions" className="grid gap-2">
				{shown.map((text) => (
					<li key={text} className="flex flex-wrap items-center justify-between gap-2 text-sm">
						<span className="min-w-0 break-words">{text}</span>
						<Button variant="line" compact aria-label={`Add: ${text}`} onClick={() => void take(text)}>
							Add
						</Button>
					</li>
				))}
			</ul>
			{problem && (
				<p role="alert" className="notice">
					{problem}
				</p>
			)}
		</div>
	);
}

/** The questions put to the AI engines: add, remove, and ask Claude for ideas. */
export function PromptsPanel({ siteId }: { siteId: string }) {
	const prompts = useAiPrompts(siteId);
	const add = useAddPrompt(siteId);
	const remove = useDeletePrompt(siteId);
	const suggest = useSuggestPrompts(siteId);
	const [text, setText] = useState("");
	const [problem, setProblem] = useState<string | null>(null);
	const [ideas, setIdeas] = useState<string[]>([]);

	async function submit(event: FormEvent) {
		event.preventDefault();
		const question = text.trim();
		if (!question) {
			setProblem("Type the question first.");
			return;
		}
		setProblem(null);
		try {
			await add.mutateAsync(question);
			setText("");
		} catch (error) {
			setProblem(errorMessage(error));
		}
	}

	async function ask() {
		try {
			setIdeas(await suggest.mutateAsync());
		} catch {
			setIdeas([]);
		}
	}

	return (
		<Panel title="Questions to ask">
			<p className="text-xs text-ink-muted">The questions your customers ask AI assistants. Each engine is asked all of them on every run.</p>
			{prompts.isPending && <Loading label="Loading the questions…" />}
			{prompts.isError && <ErrorState message={errorMessage(prompts.error)} onRetry={() => void prompts.refetch()} />}
			{prompts.data &&
				(prompts.data.length === 0 ? (
					<p className="text-sm text-ink-soft">No questions yet. Add one below, or ask Claude for ideas.</p>
				) : (
					<div className="grid gap-2">
						<p className="text-xs font-medium text-ink-muted">
							{prompts.data.length} of {LIMIT} questions
						</p>
						<ul aria-label="Questions" className="divide-y divide-line">
							{prompts.data.map((prompt) => (
								<li key={prompt.id} className="flex flex-wrap items-center justify-between gap-2 py-2.5 text-sm first:pt-0 last:pb-0">
									<span className="min-w-0 break-words">{prompt.text}</span>
									<Button variant="line" compact aria-label={`Remove question: ${prompt.text}`} onClick={() => remove.mutate(prompt.id)}>
										Remove
									</Button>
								</li>
							))}
						</ul>
						{remove.isError && (
							<p role="alert" className="notice">
								{errorMessage(remove.error)}
							</p>
						)}
					</div>
				))}
			<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-start">
				<Field label="New question" value={text} maxLength={300} autoComplete="off" onChange={(event) => setText(event.target.value)} />
				<div className="sm:pt-[1.6rem]">
					<Button type="submit" busy={add.isPending}>
						Add question
					</Button>
				</div>
				{problem && (
					<p role="alert" className="notice sm:col-span-2">
						{problem}
					</p>
				)}
			</form>
			<div className="grid gap-3">
				<div>
					<Button variant="line" busy={suggest.isPending} onClick={() => void ask()}>
						{suggest.isPending ? "Asking Claude…" : "Suggest questions"}
					</Button>
				</div>
				{suggest.isError && (
					<p role="alert" className="notice">
						{errorMessage(suggest.error)}
					</p>
				)}
				{ideas.length > 0 && <Suggestions siteId={siteId} shown={ideas} onShown={setIdeas} />}
			</div>
		</Panel>
	);
}
