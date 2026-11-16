import type { ItemStatus, SuggestionItem } from "../api/types";
import { Button } from "../ui/Button";
import { CrawledLink } from "../ui/CrawledLink";
import { shortDate } from "../ui/format";
import { StatusTag, type Tone } from "../ui/StatusTag";
import { Table, type Column } from "../ui/Table";
import { evidenceLines } from "./evidence";

const STATUS_WORD: Record<ItemStatus, string> = { open: "Open", done: "Done", dismissed: "Dismissed" };
const STATUS_TONE: Record<ItemStatus, Tone> = { open: "open", done: "course", dismissed: "line" };

function Target({ item }: { item: SuggestionItem }) {
	return item.target_kind === "page" ? (
		<CrawledLink url={item.target} />
	) : (
		<span className="break-all mono text-ink-soft">{item.target}</span>
	);
}

function Evidence({ evidence }: { evidence: Record<string, unknown> }) {
	const lines = evidenceLines(evidence);
	if (lines.length === 0) return <span className="text-xs text-ink-muted">No details recorded.</span>;
	return (
		<ul className="grid gap-1 text-xs text-ink-soft">
			{lines.map((line, index) => (
				<li key={`${index}-${line}`} className="rounded-md bg-canvas px-2 py-0.5 mono text-[11px] border border-line">
					{line}
				</li>
			))}
		</ul>
	);
}

type Props = {
	items: SuggestionItem[];
	onChange: (status: ItemStatus, target: string) => void;
};

/** One row per affected page or site, each with Done, Dismiss and Reopen. */
export function ItemsTable({ items, onChange }: Props) {
	const actions: [ItemStatus, string][] = [
		["done", "Done"],
		["dismissed", "Dismiss"],
		["open", "Reopen"],
	];
	const columns: Column<SuggestionItem>[] = [
		{ header: "Target", cell: (item) => <Target item={item} /> },
		{
			header: "Status",
			cell: (item) => <StatusTag tone={STATUS_TONE[item.status]}>{STATUS_WORD[item.status]}</StatusTag>,
		},
		{
			header: "First seen",
			cell: (item) => <span className="text-xs text-ink-muted font-medium">{shortDate(item.first_seen)}</span>,
		},
		{ header: "Evidence", cell: (item) => <Evidence evidence={item.evidence} /> },
		{
			header: "Change",
			cell: (item) => (
				<div className="flex flex-wrap gap-1.5">
					{actions.map(([status, word]) => (
						<Button
							key={status}
							variant="line"
							compact
							disabled={item.status === status}
							aria-label={`${word}: ${item.target}`}
							onClick={() => onChange(status, item.target)}
						>
							{word}
						</Button>
					))}
				</div>
			),
		},
	];
	return <Table caption="Affected items" columns={columns} rows={items} rowKey={(item) => item.target} />;
}
