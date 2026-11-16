import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useAddKeyword } from "../api/hooks";
import { useKeywordIdeas } from "../api/researchHooks";
import type { Keyword, KeywordIdea } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Refusal } from "../ui/Missing";
import { Panel } from "../ui/Panel";
import { formatMoney, formatNumber, titleCase } from "../ui/format";
import { Table, type Column } from "../ui/Table";

const NONE = "–";
/** The longest seed the server accepts. */
const SEED_MAX = 80;

function IdeaTable({ ideas, taken, siteId }: { ideas: KeywordIdea[]; taken: Set<string>; siteId: string }) {
	const add = useAddKeyword(siteId);
	const [added, setAdded] = useState<Set<string>>(new Set());
	const [problem, setProblem] = useState<string | null>(null);
	const has = (word: string) => taken.has(word.toLowerCase()) || added.has(word.toLowerCase());

	async function addIdea(word: string) {
		setProblem(null);
		try {
			await add.mutateAsync({ keyword: word });
			setAdded((now) => new Set(now).add(word.toLowerCase()));
		} catch (error) {
			setProblem(errorMessage(error));
		}
	}

	const columns: Column<KeywordIdea>[] = [
		{ header: "Idea", cell: (row) => <span className="break-words font-medium text-ink">{row.keyword}</span> },
		{ header: "Searches a month (estimate)", numeric: true, cell: (row) => (row.search_volume === null ? NONE : formatNumber(row.search_volume)) },
		{ header: "Competition", cell: (row) => (row.competition ? titleCase(row.competition) : NONE) },
		{ header: "Cost per click (estimate)", numeric: true, cell: (row) => (row.cpc === null ? NONE : formatMoney(row.cpc)) },
		{
			header: "Add",
			cell: (row) =>
				has(row.keyword) ? (
					<span data-added className="text-xs font-medium text-ink-soft">
						Added
					</span>
				) : (
					<Button variant="line" compact aria-label={`Add ${row.keyword}`} busy={add.isPending && add.variables?.keyword === row.keyword} onClick={() => void addIdea(row.keyword)}>
						Add
					</Button>
				),
		},
	];

	return (
		<div className="grid gap-3">
			<Table caption="Keyword ideas" columns={columns} rows={ideas} rowKey={(row) => row.keyword} />
			{problem && (
				<p role="alert" className="notice">
					{problem}
				</p>
			)}
		</div>
	);
}

/** Related searches for a seed word. Nothing is saved until the person adds one. */
export function IdeasPanel({ siteId, keywords }: { siteId: string; keywords: Keyword[] }) {
	const find = useKeywordIdeas(siteId);
	const [seed, setSeed] = useState("");
	const [problem, setProblem] = useState<string | null>(null);

	async function submit(event: FormEvent) {
		event.preventDefault();
		const word = seed.trim();
		if (!word) {
			setProblem("Type a word or phrase to start from.");
			return;
		}
		setProblem(null);
		try {
			await find.mutateAsync(word);
		} catch {
			// The refusal is shown from the mutation's state below.
		}
	}

	return (
		<Panel title="Find keyword ideas">
			<p className="text-xs leading-relaxed text-ink-muted">
				Type a word or phrase and see related searches with their estimated monthly volume. It uses paid search data (up to 20 searches a
				day), and nothing is saved until you press Add.
			</p>
			<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
				<Field label="Seed keyword" value={seed} maxLength={SEED_MAX} autoComplete="off" error={problem} onChange={(event) => setSeed(event.target.value)} />
				<Button type="submit" busy={find.isPending}>
					{find.isPending ? "Finding…" : "Find ideas"}
				</Button>
			</form>
			<Refusal error={find.error} />
			{find.data && find.data.length === 0 && <p className="text-sm text-ink-muted">No ideas for that seed. Try a broader word.</p>}
			{find.data && find.data.length > 0 && (
				<IdeaTable ideas={find.data} taken={new Set(keywords.map((k) => k.keyword.toLowerCase()))} siteId={siteId} />
			)}
		</Panel>
	);
}
