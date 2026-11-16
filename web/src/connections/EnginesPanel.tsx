import { Link } from "react-router-dom";
import type { Integrations } from "../api/types";
import { ENGINES } from "../ai/engines";
import { StatusTag } from "../ui/StatusTag";
import { Panel } from "./Panel";

/** Which AI engines have a key, and where to add the missing ones. */
export function EnginesPanel({ server }: { server: Integrations }) {
	return (
		<Panel title="AI engines">
			<p className="text-xs text-ink-muted">
				An engine without a key is simply off. Add or replace keys in{" "}
				<Link to="/settings" className="font-medium text-ink">
					Settings
				</Link>
				.
			</p>
			<ul aria-label="AI engines" className="divide-y divide-line">
				{ENGINES.map((engine) => {
					const row = server.engines.find((item) => item.engine === engine.id);
					const on = row?.configured ?? false;
					return (
						<li key={engine.id} className="flex flex-wrap items-center justify-between gap-2 py-3 first:pt-0 last:pb-0">
							<div className="grid gap-0.5">
								<span className="text-sm font-medium text-ink">{engine.name}</span>
								{on ? (
									<span className="text-xs text-ink-muted">Model {row?.model}</span>
								) : (
									<span className="text-xs text-ink-muted">Add a key in Settings to turn it on.</span>
								)}
							</div>
							<StatusTag tone={on ? "course" : "line"}>{on ? "Set up" : "Not set up"}</StatusTag>
						</li>
					);
				})}
			</ul>
		</Panel>
	);
}
