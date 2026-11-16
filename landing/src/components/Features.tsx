import { KeySwatch, NorthArrow, type KeySymbol } from "./symbols";
import { content } from "../content";

const features = content.features;

// The map symbol that stands for each feature, in the order of content.features.items.
const SYMBOLS: readonly (KeySymbol | "north")[] = ["contour", "course", "open", "north"];

/** The feature list, set as the map's legend: a symbol, then what it means. */
export function Features() {
	return (
		<section id="features" className="py-24 md:py-32">
			<div className="wrap grid gap-x-16 gap-y-12 xl:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
				<header className="xl:sticky xl:top-28 xl:self-start">
					<h2 className="text-course">{features.heading}</h2>
					<p className="lead mt-6 max-w-[28rem] text-ink-soft">{features.intro}</p>
				</header>

				<ul className="m-0 list-none border-b-2 border-ink p-0">
					{features.items.map((item, i) => {
						const symbol = SYMBOLS[i % SYMBOLS.length];
						return (
							<li
								key={item.title}
								className="grid grid-cols-[4.5rem_minmax(0,1fr)] items-start gap-x-6 border-t-2 border-ink py-7 sm:grid-cols-[6.5rem_minmax(0,1fr)] sm:gap-x-8"
							>
								{symbol === "north" ? (
									<NorthArrow weight={7} className="mx-auto h-[4.75rem] w-auto text-course" />
								) : (
									<KeySwatch symbol={symbol} className="mt-1 h-auto w-full" />
								)}
								<div>
									<h3>{item.title}</h3>
									<p className="mt-2.5 max-w-[38rem] text-ink-soft">{item.body}</p>
								</div>
							</li>
						);
					})}
				</ul>
			</div>
		</section>
	);
}
