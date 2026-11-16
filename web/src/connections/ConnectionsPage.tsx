import { useParams } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useIntegrations } from "../api/hooks";
import { ErrorState, Loading } from "../ui/States";
import { BingPanel } from "./BingPanel";
import { EnginesPanel } from "./EnginesPanel";
import { GooglePanel } from "./GooglePanel";
import { useGoogleReturn } from "./returnNotice";

/** Google Search Console, Bing Webmaster Tools and the AI engines, and what the server is missing. */
export function ConnectionsPage() {
	const { id = "" } = useParams();
	const server = useIntegrations();
	const returned = useGoogleReturn();

	return (
		<div className="mx-auto grid max-w-3xl gap-6">
			<div>
				<h2 className="text-[15px] font-medium text-ink">Connections</h2>
				<p className="mt-0.5 text-xs text-ink-muted">Where the numbers on the project card come from.</p>
			</div>
			{returned &&
				(returned.kind === "connected" ? (
					<p role="status" className="rounded-md border border-line bg-raised px-4 py-2.5 text-sm font-medium text-ink">
						{returned.message}
					</p>
				) : (
					<p role="alert" className="notice">
						{returned.message}
					</p>
				))}
			{server.isPending && <Loading label="Loading the connections…" />}
			{server.isError && <ErrorState message={errorMessage(server.error)} onRetry={() => void server.refetch()} />}
			{server.data && (
				<>
					<GooglePanel siteId={id} server={server.data} />
					<BingPanel siteId={id} server={server.data} />
					<EnginesPanel server={server.data} />
				</>
			)}
		</div>
	);
}
