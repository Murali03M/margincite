import type { CSSProperties } from "react";
import { content } from "../content";
import { Marker } from "./marks";

const how = content.how;

export function HowItWorks() {
	return (
		<section id="how" className="border-y border-rule bg-plate py-20 text-on-plate lg:py-28">
			<div className="wrap">
				<h2 className="heading">{how.heading}</h2>
				<ol className="relative mt-14 grid list-none gap-12 p-0 md:grid-cols-3 md:gap-10">
					<span
						aria-hidden
						className="absolute left-[2.75rem] right-[calc(33%-1rem)] top-[1.4rem] hidden border-t-2 border-dashed border-plate-rule md:block"
					/>
					{how.steps.map((step, i) => (
						<li key={step.title} className="relative" style={{ "--step": i } as CSSProperties}>
							<Marker n={i + 1} className="relative h-12 w-[3.3rem] bg-plate" />
							<h3 className="title mt-6 text-[1.5rem]">{step.title}</h3>
							<p className="mt-3 max-w-[24rem] text-on-plate-soft">{step.body}</p>
							{"code" in step && (
								<code className="mt-5 inline-block rounded-[3px] border border-plate-rule px-2.5 py-1.5 text-[0.875rem] text-on-plate">
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
