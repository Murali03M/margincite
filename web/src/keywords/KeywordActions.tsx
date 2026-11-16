import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useDeleteKeyword, useSetKeywordTarget } from "../api/hooks";
import type { Keyword } from "../api/types";
import { Button } from "../ui/Button";

/** Set, change or clear the page a keyword is judged on. */
function TargetEditor({ siteId, keyword }: { siteId: string; keyword: Keyword }) {
	const save = useSetKeywordTarget(siteId);
	const [editing, setEditing] = useState(false);
	const [value, setValue] = useState(keyword.target_url ?? "");

	async function submit(event: FormEvent) {
		event.preventDefault();
		const page = value.trim();
		try {
			await save.mutateAsync({ id: keyword.id, target_url: page ? page : null });
			setEditing(false);
		} catch {
			// The refusal is shown from the mutation's state below.
		}
	}

	if (!editing) {
		const verb = keyword.target_url ? "Change" : "Set";
		return (
			<Button
				variant="line"
				compact
				aria-label={`${verb} target page for ${keyword.keyword}`}
				onClick={() => {
					setValue(keyword.target_url ?? "");
					save.reset();
					setEditing(true);
				}}
			>
				{verb} page
			</Button>
		);
	}
	return (
		<form onSubmit={(event) => void submit(event)} noValidate className="grid min-w-[14rem] gap-2">
			<input
				aria-label={`Target page for ${keyword.keyword}`}
				className="field"
				value={value}
				placeholder="/pricing"
				autoComplete="off"
				onChange={(event) => setValue(event.target.value)}
			/>
			<div className="flex gap-2">
				<Button type="submit" compact busy={save.isPending}>
					Save
				</Button>
				<Button variant="line" compact onClick={() => setEditing(false)}>
					Cancel
				</Button>
			</div>
			{save.isError && (
				<p role="alert" className="notice">
					{errorMessage(save.error)}
				</p>
			)}
		</form>
	);
}

/** The buttons at the end of a keyword's row. */
export function KeywordActions({ siteId, keyword }: { siteId: string; keyword: Keyword }) {
	const remove = useDeleteKeyword(siteId);
	return (
		<div className="grid gap-2">
			<div className="flex flex-wrap gap-2">
				<TargetEditor siteId={siteId} keyword={keyword} />
				<Button variant="line" compact busy={remove.isPending} aria-label={`Remove ${keyword.keyword}`} onClick={() => remove.mutate(keyword.id)}>
					Remove
				</Button>
			</div>
			{remove.isError && (
				<p role="alert" className="notice">
					{errorMessage(remove.error)}
				</p>
			)}
		</div>
	);
}
