import { errorMessage } from "../api/client";
import { useSettings } from "../api/settingsHooks";
import { ErrorState, Loading } from "../ui/States";
import { CrawlerPanel } from "./CrawlerPanel";
import { DataPanel } from "./DataPanel";
import { GooglePanel } from "./GooglePanel";
import { PageSpeedPanel } from "./PageSpeedPanel";
import { ProvidersPanel } from "./ProvidersPanel";
import { UsagePanel } from "./UsagePanel";

/** Settings: every key and option Margincite needs, set here instead of in a file on the server. */
export function SettingsPage() {
	const settings = useSettings();

	if (settings.isPending) return <Loading label="Loading the settings…" />;
	if (settings.isError) return <ErrorState message={errorMessage(settings.error)} onRetry={() => void settings.refetch()} />;
	const data = settings.data;
	const locked = !data.secret_key_ready;

	return (
		<div className="mx-auto grid max-w-3xl gap-6">
			<div>
				<h1 className="text-lg font-medium text-ink">Settings</h1>
				<p className="mt-1 text-xs text-ink-muted">
					Connect your own providers here. Keys are stored encrypted and are never shown again after you save them.
				</p>
			</div>
			{locked && (
				<p role="alert" className="notice">
					Keys cannot be saved yet. The server needs a <code className="font-medium">MARGINCITE_SECRET_KEY</code> to encrypt them.
					Set it in the server's environment file and restart the backend, then come back.
				</p>
			)}
			<CrawlerPanel crawler={data.crawler} />
			<ProvidersPanel providers={data.providers} locked={locked} />
			<GooglePanel google={data.google} locked={locked} />
			<DataPanel data={data.dataforseo} locked={locked} />
			<PageSpeedPanel pagespeed={data.pagespeed} locked={locked} />
			<UsagePanel />
		</div>
	);
}
