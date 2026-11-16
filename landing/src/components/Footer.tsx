import { content } from "../content";
import { Logo } from "./Logo";

const footer = content.footer;

export function Footer() {
	return (
		<footer className="bg-ink text-on-ink">
			<div aria-hidden className="ruler border-x-0 [--unit:1.5rem]" />
			<div className="wrap grid gap-10 py-14 md:grid-cols-12">
				<div className="md:col-span-5">
					<Logo onInk />
					<p className="mt-4 max-w-[22rem] text-on-ink-soft">{footer.blurb}</p>
				</div>
				<nav aria-label="Footer" className="md:col-span-3">
					<ul className="m-0 list-none space-y-2.5 p-0">
						{footer.links.map((link) => (
							<li key={link.href}>
								<a href={link.href} className="label nav-link text-[0.8125rem]">
									{link.label}
								</a>
							</li>
						))}
					</ul>
				</nav>
				<div className="space-y-2.5 text-[0.9375rem] text-on-ink-soft md:col-span-4">
					<p>{footer.licence}</p>
					<p>
						<a href={footer.source.href}>{footer.source.label}</a>
					</p>
					<p>
						<a href={footer.counter.href}>{footer.counter.label}</a>
					</p>
				</div>
			</div>
		</footer>
	);
}
