import { Link, useParams } from "react-router-dom";
import { useIntegrations } from "../api/hooks";
import { Panel } from "../ui/Panel";
import { AiResults } from "./AiResults";
import { AiRunPanel } from "./AiRunPanel";
import { AiSettingsPanel } from "./AiSettingsPanel";
import { OverviewsSection } from "./OverviewsSection";
import { PromptsPanel } from "./PromptsPanel";
import { ENGINES } from "./engines";

function EnginesOff() {
	const server = useIntegrations();
	if (!server.data) return null;
	const off = ENGINES.filter((engine) => !server.data.engines.find((row) => row.engine === engine.id)?.configured);
	if (off.length === 0) return null;
	return (
		<div className="grid gap-2 rounded-md border border-line bg-canvas p-4">
			<p className="text-xs font-medium text-ink-soft">Not set up, so these engines are not asked:</p>
			<ul aria-label="Engines not set up" className="grid gap-1 text-xs text-ink-soft">
				{off.map((engine) => (
					<li key={engine.id}>
						<span className="font-medium">{engine.name}</span>: add a key in{" "}
						<Link to="/settings" className="font-medium text-ink">
							Settings
						</Link>
						.
					</li>
				))}
			</ul>
		</div>
	);
}

/** AI Visibility: do AI assistants mention or cite this site when asked the questions customers ask? */
export function AiPage() {
	const { id = "" } = useParams();
	return (
		<div className="grid gap-6">
			<div>
				<h2 className="text-[15px] font-medium text-ink">AI Visibility</h2>
				<p className="mt-0.5 text-xs text-ink-muted">
					Margincite asks AI assistants your questions and notes whether they mention or cite your site. Up to 3 manual runs a day.
				</p>
			</div>
			<AiRunPanel siteId={id} />
			<EnginesOff />
			<Panel title="Latest results">
				<AiResults siteId={id} />
			</Panel>
			<OverviewsSection siteId={id} />
			<div className="grid gap-6 lg:grid-cols-2 lg:items-start">
				<PromptsPanel siteId={id} />
				<AiSettingsPanel siteId={id} />
			</div>
		</div>
	);
}
