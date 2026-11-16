import { content } from "../content";

const faq = content.faq;

export function Faq() {
	return (
		<section id="faq" className="border-t border-ink bg-paper py-20 lg:py-28">
			<div className="wrap grid gap-x-14 gap-y-10 lg:grid-cols-12">
				<h2 className="heading lg:col-span-4">{faq.heading}</h2>
				<div className="border-t-2 border-ink lg:col-span-8">
					{faq.items.map((item) => (
						<details key={item.question} className="group border-b border-rule">
							<summary className="flex cursor-pointer list-none items-center justify-between gap-6 py-5 [&::-webkit-details-marker]:hidden">
								<span className="title">{item.question}</span>
								<svg viewBox="0 0 20 20" className="h-5 w-5 shrink-0 transition-transform duration-200 group-open:rotate-45" aria-hidden>
									<path d="M10 3v14M3 10h14" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" />
								</svg>
							</summary>
							<p className="measure pb-6 text-ink-soft">{item.answer}</p>
						</details>
					))}
				</div>
			</div>
		</section>
	);
}
