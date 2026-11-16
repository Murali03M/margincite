import { Logo } from "./Logo";
import { NorthArrow, ScaleBar } from "./symbols";
import { content } from "../content";

/** The foot of the sheet, where a map keeps its north arrow and scale. */
export function Footer() {
	return (
		<footer className="bg-sheet py-14">
			<div className="wrap flex flex-col gap-10 md:flex-row md:items-end md:justify-between">
				<div>
					<Logo />
					<p className="mt-4 max-w-xs text-ink-soft">{content.footer.blurb}</p>
				</div>

				<nav aria-label="Footer">
					<ul className="m-0 flex list-none flex-col gap-3 p-0 md:flex-row md:gap-8">
						{content.footer.links.map((link) => (
							<li key={link.href}>
								<a href={link.href} className="label nav-link">
									{link.label}
								</a>
							</li>
						))}
					</ul>
				</nav>

				<div className="flex items-end gap-6 text-course">
					<ScaleBar className="h-5 w-40" />
					<NorthArrow className="h-16 w-auto" />
				</div>
			</div>
			<p className="wrap mt-10 border-t-2 border-course pt-5 text-[0.9375rem] text-ink-soft">
				© {content.brand}
			</p>
		</footer>
	);
}
