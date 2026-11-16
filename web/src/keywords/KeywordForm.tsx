import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useAddKeyword } from "../api/hooks";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";

/** Adds a target keyword, with an optional page it should be judged on. */
export function KeywordForm({ siteId }: { siteId: string }) {
	const add = useAddKeyword(siteId);
	const [keyword, setKeyword] = useState("");
	const [target, setTarget] = useState("");
	const [problem, setProblem] = useState<string | null>(null);

	async function submit(event: FormEvent) {
		event.preventDefault();
		const word = keyword.trim();
		if (!word) {
			setProblem("Type the keyword first.");
			return;
		}
		setProblem(null);
		const page = target.trim();
		try {
			await add.mutateAsync(page ? { keyword: word, target_url: page } : { keyword: word });
			setKeyword("");
			setTarget("");
		} catch (error) {
			setProblem(errorMessage(error));
		}
	}

	return (
		<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-4 rounded-md border border-line bg-raised p-5 sm:grid-cols-[1fr_1fr_auto] sm:items-start">
			<Field label="Keyword" value={keyword} maxLength={100} autoComplete="off" onChange={(event) => setKeyword(event.target.value)} />
			<Field
				label="Target page (optional)"
				value={target}
				autoComplete="off"
				placeholder="/pricing"
				onChange={(event) => setTarget(event.target.value)}
			/>
			<div className="sm:pt-[1.6rem]">
				<Button type="submit" busy={add.isPending}>
					{add.isPending ? "Adding…" : "Add keyword"}
				</Button>
			</div>
			{problem && (
				<p role="alert" className="notice sm:col-span-3">
					{problem}
				</p>
			)}
		</form>
	);
}
