import { useState } from "react";
import { Logo } from "./Logo";
import { ThemeToggle } from "./ThemeToggle";
import { VisitCounter } from "./VisitCounter";
import { CloseIcon, GitHubMark, MenuIcon } from "./marks";
import { content } from "../content";

const LINKS = content.nav.links;
const CTA = content.nav.cta;

export function Nav({ load }: { load?: () => Promise<number | null> }) {
	const [open, setOpen] = useState(false);

	return (
		<header className="sticky top-0 z-50 border-b border-ink bg-paper">
			<nav className="wrap flex h-16 items-center justify-between gap-4" aria-label="Main">
				<a href="#top" aria-label={`${content.brand} home`} className="shrink-0 no-underline">
					<Logo />
				</a>

				<ul className="m-0 hidden list-none items-center gap-8 p-0 lg:flex">
					{LINKS.map((link) => (
						<li key={link.href}>
							<a href={link.href} className="label nav-link text-[0.8125rem]">
								{link.label}
							</a>
						</li>
					))}
				</ul>

				<div className="flex items-center gap-2 sm:gap-3">
					<VisitCounter load={load} />
					<ThemeToggle className="hidden sm:inline-flex" />
					<a href={CTA.href} className="btn btn-compact hidden sm:inline-flex">
						<GitHubMark />
						{CTA.label}
					</a>
					<a href={CTA.href} aria-label={CTA.label} className="btn btn-compact px-2.5 sm:hidden">
						<GitHubMark />
					</a>
					<button
						type="button"
						aria-label={open ? "Close menu" : "Open menu"}
						aria-expanded={open}
						aria-controls="mobile-menu"
						onClick={() => setOpen((value) => !value)}
						className="btn btn-line btn-compact px-2 lg:hidden"
					>
						{open ? <CloseIcon /> : <MenuIcon />}
					</button>
				</div>
			</nav>

			{open && (
				<div id="mobile-menu" className="border-t border-ink bg-paper lg:hidden">
					<ul className="wrap my-0 flex list-none flex-col p-0 py-2">
						{LINKS.map((link) => (
							<li key={link.href} className="border-b border-rule">
								<a
									href={link.href}
									onClick={() => setOpen(false)}
									className="block py-4 font-[family-name:var(--font-display)] text-[1.75rem] font-[850] uppercase leading-none no-underline [font-stretch:108%]"
								>
									{link.label}
								</a>
							</li>
						))}
						<li className="pt-5 sm:hidden">
							<ThemeToggle withLabel className="w-full" />
						</li>
						<li className="pb-3 pt-3 sm:hidden">
							<a href={CTA.href} onClick={() => setOpen(false)} className="btn w-full">
								<GitHubMark />
								{CTA.label}
							</a>
						</li>
					</ul>
				</div>
			)}
		</header>
	);
}
