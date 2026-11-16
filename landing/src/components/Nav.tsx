import { useState } from "react";
import { Logo } from "./Logo";
import { CloseIcon, MenuIcon } from "./symbols";
import { content } from "../content";

const LINKS = content.nav.links;
const CTA = content.nav.cta;

export function Nav() {
	const [open, setOpen] = useState(false);

	return (
		<header className="sticky top-0 z-50 border-b-2 border-course bg-sheet">
			<nav className="wrap flex h-[4.5rem] items-center justify-between gap-6" aria-label="Main">
				<a href="#top" aria-label={`${content.brand} home`} className="shrink-0 no-underline">
					<Logo />
				</a>

				<ul className="m-0 hidden list-none items-center gap-9 p-0 lg:flex">
					{LINKS.map((link) => (
						<li key={link.href}>
							<a href={link.href} className="label nav-link">
								{link.label}
							</a>
						</li>
					))}
				</ul>

				<a href={CTA.href} className="btn hidden min-h-[2.75rem] sm:inline-flex">
					{CTA.label}
				</a>

				<button
					type="button"
					aria-label={open ? "Close menu" : "Open menu"}
					aria-expanded={open}
					aria-controls="mobile-menu"
					onClick={() => setOpen((value) => !value)}
					className="btn btn-line min-h-[2.75rem] px-2.5 lg:hidden"
				>
					{open ? <CloseIcon /> : <MenuIcon />}
				</button>
			</nav>

			{open && (
				<div id="mobile-menu" className="border-t-2 border-course bg-sheet lg:hidden">
					<ul className="wrap flex flex-col py-3">
						{LINKS.map((link) => (
							<li key={link.href} className="border-b border-ink/15">
								<a
									href={link.href}
									onClick={() => setOpen(false)}
									className="block py-4 font-[family-name:var(--font-display)] text-4xl uppercase leading-none text-course no-underline"
								>
									{link.label}
								</a>
							</li>
						))}
						<li className="pt-5 pb-2 sm:hidden">
							<a href={CTA.href} onClick={() => setOpen(false)} className="btn w-full">
								{CTA.label}
							</a>
						</li>
					</ul>
				</div>
			)}
		</header>
	);
}
