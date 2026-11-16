import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, ReactNode } from "react";
import { useLocation } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { useSites } from "../api/hooks";
import { clearToken } from "../auth/session";
import { Sidebar } from "./Sidebar";
import { pageName } from "./shellRoute";

const FOCUSABLE = "a[href], button:not([disabled])";

/** The title bar's path: the site's host, then the section; just the page name outside a site. */
function Breadcrumb({ host, name }: { host: string | null; name: string }) {
	return (
		<nav aria-label="Breadcrumb" className="min-w-0">
			<ol className="flex min-w-0 items-center gap-1.5 text-sm">
				{host && (
					<>
						<li className="mono min-w-0 truncate text-ink-soft">{host}</li>
						<li aria-hidden className="text-ink-muted">/</li>
					</>
				)}
				<li aria-current="page" className="min-w-0 truncate font-medium text-ink">
					{name}
				</li>
			</ol>
		</nav>
	);
}

/** The frame of every signed-in screen: sidebar (a drawer on small screens), a slim title bar, then the page. */
export function Shell({ children }: { children: ReactNode }) {
	const client = useQueryClient();
	const location = useLocation();
	const sites = useSites();
	const [open, setOpen] = useState(false);
	const menuButton = useRef<HTMLButtonElement>(null);
	const sidebar = useRef<HTMLElement>(null);

	const { siteId, name } = pageName(location.pathname, location.search);
	const host = siteId ? (sites.data?.find((site) => site.id === siteId)?.host ?? null) : null;

	const close = useCallback(() => {
		setOpen(false);
		menuButton.current?.focus();
	}, []);

	// Following a link closes the drawer. Skipped while it is shut, so focus is not pulled on every page change.
	const lastPath = useRef(location.pathname + location.search);
	useEffect(() => {
		const path = location.pathname + location.search;
		if (path === lastPath.current) return;
		lastPath.current = path;
		if (open) close();
	}, [location.pathname, location.search, open, close]);

	useEffect(() => {
		if (!open) return;
		sidebar.current?.querySelector<HTMLElement>(FOCUSABLE)?.focus();
		function onKey(event: KeyboardEvent) {
			if (event.key === "Escape") close();
		}
		document.addEventListener("keydown", onKey);
		return () => document.removeEventListener("keydown", onKey);
	}, [open, close]);

	/** Keeps Tab inside the open drawer, so it never walks behind the dimmed page. */
	function keepFocusInside(event: ReactKeyboardEvent) {
		if (!open || event.key !== "Tab" || !sidebar.current) return;
		const items = Array.from(sidebar.current.querySelectorAll<HTMLElement>(FOCUSABLE));
		const first = items[0];
		const last = items[items.length - 1];
		if (event.shiftKey && document.activeElement === first) {
			event.preventDefault();
			last?.focus();
		} else if (!event.shiftKey && document.activeElement === last) {
			event.preventDefault();
			first?.focus();
		}
	}

	function signOut() {
		clearToken();
		client.clear();
	}

	return (
		<div className="min-h-screen bg-canvas text-ink" onKeyDown={keepFocusInside}>
			<a href="#main" className="skip-link">
				Skip to the content
			</a>
			<Sidebar ref={sidebar} open={open} currentSiteId={siteId} onClose={close} onSignOut={signOut} />
			{open && <div data-testid="scrim" className="scrim" onClick={close} aria-hidden />}
			<div className="shell-main">
				<header className="sticky top-0 z-30 flex h-11 items-center gap-2 border-b border-line bg-bar px-3">
					<button
						ref={menuButton}
						type="button"
						aria-label="Open menu"
						aria-expanded={open}
						aria-controls="sidebar"
						onClick={() => setOpen(true)}
						className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-ink-soft hover:bg-hover hover:text-ink lg:hidden"
					>
						<svg viewBox="0 0 16 16" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden>
							<path d="M2.5 4.5h11M2.5 8h11M2.5 11.5h11" />
						</svg>
					</button>
					<Breadcrumb host={host} name={name} />
				</header>
				<main id="main" className="mx-auto w-full max-w-[64rem] px-4 py-6 sm:px-6">
					{children}
				</main>
			</div>
		</div>
	);
}
