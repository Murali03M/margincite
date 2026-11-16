import { forwardRef } from "react";
import { Link, NavLink } from "react-router-dom";
import { useMe, useSites, useSummary } from "../api/hooks";
import type { Site } from "../api/types";
import { SECTIONS } from "../sites/sections";
import { Logo } from "./Logo";

/** Small mark for a site's health: only meaning, only a dot. */
function healthDot(score: number | null): string {
	if (score === null) return "dot-info";
	if (score >= 85) return "dot-good";
	if (score >= 70) return "dot-warn";
	return "dot-bad";
}

function SiteFacts({ siteId }: { siteId: string }) {
	const summary = useSummary(siteId);
	if (summary.isPending) return <span className="text-xs text-ink-muted">Loading…</span>;
	if (summary.isError) return <span className="text-xs text-ink-muted">Not loaded</span>;
	const { score, counts } = summary.data;
	return (
		<span className="flex items-center gap-1.5 text-xs text-ink-muted">
			<span className={`dot ${healthDot(score)}`} aria-hidden />
			<span>
				{score === null ? "No score" : `Score ${score}`} · {counts.open} open
			</span>
		</span>
	);
}

function SiteSections({ siteId }: { siteId: string }) {
	return (
		<nav aria-label="Site sections" className="mt-1 ml-3 border-l border-line pl-2">
			<ul className="grid gap-px">
				{SECTIONS.map((section) => (
					<li key={section.label}>
						<NavLink
							to={section.to ? `/sites/${siteId}/${section.to}` : `/sites/${siteId}`}
							end={section.to === ""}
							className="side-link"
						>
							{section.label}
						</NavLink>
					</li>
				))}
			</ul>
		</nav>
	);
}

function SiteRow({ site, current }: { site: Site; current: boolean }) {
	return (
		<li>
			<Link
				to={`/sites/${site.id}`}
				aria-current={current ? "true" : undefined}
				className="side-link grid gap-0.5 py-1.5"
			>
				<span className="break-all text-ink">{site.host}</span>
				<SiteFacts siteId={site.id} />
			</Link>
			{current && <SiteSections siteId={site.id} />}
		</li>
	);
}

type SidebarProps = {
	open: boolean;
	currentSiteId: string | null;
	onClose: () => void;
	onSignOut: () => void;
};

/** The fixed left column: product name, the sites (the current one opens its sections) and the account. */
export const Sidebar = forwardRef<HTMLElement, SidebarProps>(function Sidebar(
	{ open, currentSiteId, onClose, onSignOut },
	ref,
) {
	const sites = useSites();
	const me = useMe();
	return (
		<aside ref={ref} aria-label="Sidebar" data-open={open} id="sidebar" className="sidebar">
			<div className="flex h-11 shrink-0 items-center justify-between border-b border-line px-3">
				<Link to="/" aria-label="Margincite, home" className="rounded-md hover:no-underline">
					<Logo />
				</Link>
				<button
					type="button"
					aria-label="Close menu"
					onClick={onClose}
					className="flex h-7 w-7 items-center justify-center rounded-md text-ink-soft hover:bg-hover hover:text-ink lg:hidden"
				>
					<svg viewBox="0 0 16 16" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden>
						<path d="M4 4l8 8M12 4l-8 8" />
					</svg>
				</button>
			</div>
			<div className="min-h-0 flex-1 overflow-y-auto p-2">
				<nav aria-label="Sites">
					<p className="label px-2 pb-1 pt-1">Sites</p>
					<ul className="grid gap-px">
						{sites.data?.map((site) => (
							<SiteRow key={site.id} site={site} current={site.id === currentSiteId} />
						))}
						<li>
							<Link to="/sites/new" className="side-link text-ink-soft">
								Add site
							</Link>
						</li>
					</ul>
					{sites.isPending && <p className="px-2 pt-1 text-xs text-ink-muted">Loading sites…</p>}
					{sites.isError && <p className="px-2 pt-1 text-xs text-ink-muted">The sites did not load.</p>}
				</nav>
			</div>
			<div className="grid shrink-0 gap-1 border-t border-line p-2">
				<NavLink to="/settings" className="side-link">
					Settings
				</NavLink>
				<div className="flex items-center justify-between gap-2 px-2">
					<span className="min-w-0 break-all text-xs text-ink-muted">{me.data?.email}</span>
					<button
						type="button"
						onClick={onSignOut}
						className="shrink-0 rounded-md px-2 py-1 text-xs text-ink-soft hover:bg-hover hover:text-ink"
					>
						Sign out
					</button>
				</div>
			</div>
		</aside>
	);
});
