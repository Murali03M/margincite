import type { CSSProperties } from "react";
import { content } from "../content";
import { Marker } from "./marks";

const how = content.how;

export function HowItWorks() {
	return (
		<section id="how" className="bg-ink py-20 text-on-ink lg:py-28">
			<div className="wrap">
				<h2 className="heading">{how.heading}</h2>
				<ol className="relative mt-14 grid list-none gap-12 p-0 md:grid-cols-3 md:gap-10">
					<span
						aria-hidden
						className="absolute left-[2.75rem] right-[calc(33%-1rem)] top-[1.4rem] hidden border-t-2 border-dashed border-on-ink-soft/60 md:block"
					/>
					{how.steps.map((step, i) => (
						<li key={step.title} className="relative" style={{ "--step": i } as CSSProperties}>
							<Marker n={i + 1} className="relative h-12 w-[3.3rem] bg-ink" />
							<h3 className="title mt-6 text-[1.5rem]">{step.title}</h3>
							<p className="mt-3 max-w-[24rem] text-on-ink-soft">{step.body}</p>
							{"code" in step && (
								<code className="mt-5 inline-block rounded-[3px] border border-on-ink-soft/50 px-2.5 py-1.5 text-[0.875rem] text-on-ink">
									<span className="select-none text-marker">$ </span>
									{step.code}
								</code>
							)}
						</li>
					))}
				</ol>
			</div>
		</section>
	);
}
