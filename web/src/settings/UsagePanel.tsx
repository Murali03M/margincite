import { errorMessage } from "../api/client";
import { useUsage } from "../api/settingsHooks";
import type { UsageRow } from "../api/types";
import { ENGINES, engineName } from "../ai/engines";
import { Panel } from "../ui/Panel";
import { formatMoney, formatNumber } from "../ui/format";
import { ErrorState, Loading } from "../ui/States";
import { Table, type Column } from "../ui/Table";

type Total = { key: string; source: string; kind: string; calls: number; cost: number | null };

const SOURCES: Record<string, string> = { dataforseo: "DataForSEO", pagespeed: "Google PageSpeed" };

export function sourceName(source: string): string {
	if (SOURCES[source]) return SOURCES[source];
	const engine = ENGINES.find((item) => item.id === source);
	return engine ? engineName(engine.id) : source;
}

const KINDS: Record<string, string> = {
	page_test: "Page speed test",
	serp: "Search result check",
	keyword_volume: "Search volume lookup",
	keyword_ideas: "Keyword ideas search",
};

/** What a kind of call was for, in plain words. A kind this list does not know is shown as the server sent it. */
export function kindName(kind: string): string {
	return KINDS[kind] ?? kind;
}

/** Adds the days up by source and kind. A group with no estimate on any day stays "no estimate", never zero. */
export function totals(rows: UsageRow[]): Total[] {
	const groups = new Map<string, Total>();
	for (const row of rows) {
		const key = `${row.source}/${row.kind}`;
		const before = groups.get(key) ?? { key, source: row.source, kind: row.kind, calls: 0, cost: null };
		const cost = row.estimated_cost_usd === null ? before.cost : (before.cost ?? 0) + row.estimated_cost_usd;
		groups.set(key, { ...before, calls: before.calls + row.calls, cost });
	}
	return [...groups.values()].sort((a, b) => a.source.localeCompare(b.source) || a.kind.localeCompare(b.kind));
}

const columns: Column<Total>[] = [
	{ header: "Source", cell: (row) => <span className="font-medium text-ink">{sourceName(row.source)}</span> },
	{ header: "Used for", cell: (row) => <span className="break-words">{kindName(row.kind)}</span> },
	{ header: "Calls", numeric: true, cell: (row) => formatNumber(row.calls) },
	{ header: "Estimated cost", numeric: true, cell: (row) => (row.cost === null ? <span className="text-ink-muted">No estimate</span> : formatMoney(row.cost)) },
];

/** What the paid and metered sources were used for in the last 30 days. */
export function UsagePanel() {
	const usage = useUsage(30);
	let body;
	if (usage.isPending) body = <Loading label="Loading the usage…" />;
	else if (usage.isError) body = <ErrorState message={errorMessage(usage.error)} onRetry={() => void usage.refetch()} />;
	else if (usage.data.length === 0) body = <p className="text-sm text-ink-muted">No paid calls in the last 30 days.</p>;
	else
		body = (
			<>
				<Table caption="Calls and estimated cost by source" columns={columns} rows={totals(usage.data)} rowKey={(row) => row.key} />
				<p className="text-xs text-ink-muted">
					Costs are estimates worked out from each source's published prices. Your provider's invoice is the real figure.
				</p>
			</>
		);
	return (
		<Panel title="Usage">
			<p className="text-sm text-ink-soft">Calls Margincite made in the last 30 days, and roughly what they cost.</p>
			{body}
		</Panel>
	);
}
