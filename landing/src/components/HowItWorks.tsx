import { ControlSymbol, FinishSymbol, StartSymbol } from "./symbols";
import { content } from "../content";
import { asset } from "../asset";

const how = content.how;

// A course in three marks: the start triangle, a control, and the finish.
const MARKS = [StartSymbol, ControlSymbol, FinishSymbol] as const;

export function HowItWorks() {
	return (
		<section
			id="how"
			className="contours border-y-2 border-course py-24 md:py-32"
			style={{
				["--contour-image" as string]: `url(${asset("map/contours-brown.svg")})`,
				["--contour-strength" as string]: 0.16,
				["--contour-shift" as string]: "84% 12%",
			}}
		>
			<div className="wrap">
				<h2 className="text-course">{how.heading}</h2>

				<ol className="m-0 mt-14 grid list-none gap-y-12 p-0 md:grid-cols-3 md:gap-x-10">
					{how.steps.map((step, i) => {
						const Mark = MARKS[i % MARKS.length];
						return (
							<li key={step.title} className="relative">
								{i < how.steps.length - 1 && (
									<span
										aria-hidden
										className="absolute left-[5.25rem] right-[-2rem] top-[2.1rem] hidden h-[5px] rounded-full bg-course md:block"
									/>
								)}
								<Mark className="h-[4.5rem] w-[4.5rem] text-course" />
								<h3 className="mt-6">{step.title}</h3>
								<p className="mt-2.5 max-w-[24rem] text-ink-soft">{step.body}</p>
							</li>
						);
					})}
				</ol>
			</div>
		</section>
	);
}
