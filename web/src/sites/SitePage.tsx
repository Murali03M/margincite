import { Outlet, useParams } from "react-router-dom";
import { ApiError, errorMessage } from "../api/client";
import { useSite } from "../api/hooks";
import { NotFound } from "../ui/NotFound";
import { ErrorState, Loading } from "../ui/States";

/**
 * The frame of one site: its name and a link to it, then the section's screen in the Outlet.
 * The sections themselves are listed in the sidebar.
 */
export function SitePage() {
	const { id = "" } = useParams();
	const site = useSite(id);

	if (site.isPending) return <Loading label="Loading the site…" />;
	if (site.isError) {
		if (site.error instanceof ApiError && site.error.status === 404) return <NotFound what="site" />;
		return <ErrorState message={errorMessage(site.error)} onRetry={() => void site.refetch()} />;
	}

	const host = site.data.host;
	const externalUrl = site.data.root_url || `https://${host}`;

	return (
		<div className="grid grid-cols-[minmax(0,1fr)] gap-6">
			<div className="flex items-center gap-2 border-b border-line pb-4">
				<h1 className="mono break-all text-lg font-medium text-ink">{host}</h1>
				<a
					href={externalUrl}
					target="_blank"
					rel="noreferrer noopener"
					className="inline-flex items-center justify-center rounded-md p-1 text-ink-muted hover:bg-hover hover:text-ink"
					title={`Visit ${host}`}
					aria-label={`Visit ${host} in a new tab`}
				>
					<svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4" aria-hidden>
						<path fillRule="evenodd" d="M4.25 5.5a.75.75 0 0 0-.75.75v8.5c0 .414.336.75.75.75h8.5a.75.75 0 0 0 .75-.75v-4a.75.75 0 0 1 1.5 0v4A2.25 2.25 0 0 1 12.75 17h-8.5A2.25 2.25 0 0 1 2 14.75v-8.5A2.25 2.25 0 0 1 4.25 4h4a.75.75 0 0 1 0 1.5h-4Z" clipRule="evenodd" />
						<path fillRule="evenodd" d="M6.194 12.753a.75.75 0 0 0 1.06 1.06L14.5 6.56v3.19a.75.75 0 0 0 1.5 0v-5a.75.75 0 0 0-.75-.75h-5a.75.75 0 0 0 0 1.5h3.19l-7.246 7.253Z" clipRule="evenodd" />
					</svg>
				</a>
			</div>
			<Outlet />
		</div>
	);
}
