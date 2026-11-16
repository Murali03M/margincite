import { ControlSymbol, PlannedSymbol } from "./symbols";
import { content } from "../content";
import { asset } from "../asset";

const product = content.product;

/**
 * One suggestion, set like the control description sheet a runner carries:
 * a ruled grid that says exactly what is at this control.
 */
export function ProductDetail() {
	return (
		<section
			id="product"
			className="contours bg-open py-24 md:py-32"
			style={{
				["--contour-image" as string]: `url(${asset("map/contours-brown.svg")})`,
				["--contour-strength" as string]: 0.3,
				["--contour-shift" as string]: "18% 40%",
			}}
		>
			<div className="wrap grid items-start gap-x-16 gap-y-14 lg:grid-cols-[minmax(0,5fr)_minmax(0,6fr)]">
				<div>
					<h2>{product.heading}</h2>
					<p className="lead mt-6 max-w-[30rem]">{product.intro}</p>
					<p className="mt-8 max-w-[30rem] text-[1.5rem] font-semibold leading-[1.25] text-course-deep">
						{product.honesty}
					</p>

					<h3 className="mt-14">{product.coming.heading}</h3>
					<ul aria-label={product.coming.heading} className="m-0 mt-4 list-none space-y-3 p-0">
						{product.coming.items.map((item) => (
							<li key={item} className="flex items-start gap-3">
								<PlannedSymbol className="mt-0.5 h-7 w-7 shrink-0 text-course-deep" />
								{item}
							</li>
						))}
					</ul>
				</div>

				<article className="rounded-[4px] border-[3px] border-ink bg-sheet">
					<header className="flex items-center gap-4 p-5 sm:p-6">
						<ControlSymbol label="1" className="h-14 w-14 shrink-0 text-course" />
						<div className="flex flex-1 flex-col items-start gap-2.5 sm:flex-row sm:items-start sm:gap-4">
							<h3 className="flex-1">{product.note.title}</h3>
							<span className="label shrink-0 rounded-[4px] bg-course px-2 py-0.5 text-white">
								{product.tag}
							</span>
						</div>
					</header>
					<dl className="m-0">
						{product.note.parts.map((part) => (
							<div
								key={part.label}
								className="grid border-t-2 border-ink sm:grid-cols-[10.5rem_minmax(0,1fr)]"
							>
								<dt className="label px-5 pt-5 text-course-deep sm:border-r-2 sm:border-ink sm:px-6 sm:py-5">
									{part.label}
								</dt>
								<dd className="m-0 px-5 pb-5 pt-1.5 sm:px-6 sm:py-5">{part.body}</dd>
							</div>
						))}
					</dl>
				</article>
			</div>
		</section>
	);
}
