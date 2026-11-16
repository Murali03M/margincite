import { content } from "../content";
import { Marker, PlannedMarker } from "./marks";
import { ServesTags } from "./Tags";

const product = content.product;
const sample = product.sample;

function ExampleBadge() {
	return <span className="label rounded-[3px] bg-ink px-2 py-1 text-marker">{product.tag}</span>;
}

function SampleList() {
	return (
		<div className="paper mt-12 rounded-[3px] border-[1.5px] border-ink">
			<div className="flex flex-wrap items-center justify-between gap-3 border-b-2 border-ink px-4 py-3 sm:px-6">
				<span className="font-[family-name:var(--font-mono)] text-[0.875rem] font-bold">{sample.caption}</span>
				<ExampleBadge />
			</div>
			<div aria-hidden className="label hidden grid-cols-[3.5rem_1fr_8rem_7.5rem] gap-4 border-b border-rule px-6 py-2.5 text-ink-faint md:grid">
				<span>{sample.columns.rank}</span>
				<span>{sample.columns.fix}</span>
				<span>{sample.columns.serves}</span>
				<span>{sample.columns.reach}</span>
			</div>
			<ol aria-label={sample.caption} className="m-0 list-none p-0">
				{sample.rows.map((row, i) => (
					<li
						key={row.title}
						className="grid grid-cols-[2.75rem_1fr] items-center gap-x-3 gap-y-2 border-b border-rule px-4 py-3 last:border-b-0 sm:px-6 md:grid-cols-[3.5rem_1fr_8rem_7.5rem] md:gap-4"
					>
						<Marker n={i + 1} className="row-span-2 h-9 w-10 md:row-span-1" />
						<span className="font-semibold leading-snug">{row.title}</span>
						<span className="flex flex-wrap items-center gap-3 md:contents">
							<ServesTags serves={row.serves} />
							<span className="font-[family-name:var(--font-mono)] text-[0.8125rem] text-ink-soft">{row.reach}</span>
						</span>
					</li>
				))}
			</ol>
		</div>
	);
}

function FindingSheet() {
	const note = product.note;
	return (
		<article className="paper rounded-[3px] border-2 border-ink">
			<div className="flex items-center gap-4 border-b-2 border-ink px-4 py-4 sm:px-6">
				<Marker n={1} className="h-11 w-12 shrink-0" />
				<h3 className="title min-w-0 flex-1">{note.title}</h3>
				<span className="hidden sm:inline">
					<ExampleBadge />
				</span>
			</div>
			<dl className="m-0">
				{note.parts.map((part) => (
					<div key={part.label} className="grid gap-1 border-b border-rule px-4 py-4 last:border-b-0 sm:grid-cols-[10rem_1fr] sm:gap-6 sm:px-6">
						<dt className="label pt-1 text-ink-soft">{part.label}</dt>
						<dd className="m-0">{part.body}</dd>
					</div>
				))}
			</dl>
		</article>
	);
}

export function ProductDetail() {
	return (
		<section id="product" className="py-20 lg:py-28">
			<div className="wrap">
				<h2 className="heading">{product.heading}</h2>
				<p className="lead measure mt-5 text-ink-soft">{product.intro}</p>
				<SampleList />
				<div className="mt-10 grid gap-x-14 gap-y-12 lg:grid-cols-12">
					<div className="lg:col-span-7">
						<FindingSheet />
					</div>
					<div className="lg:col-span-5">
						<p className="font-[family-name:var(--font-display)] text-[1.5rem] font-[800] leading-tight [font-stretch:104%]">{product.honesty}</p>
						<h3 id="coming-later" className="label mt-10 text-ink-soft">
							{product.coming.heading}
						</h3>
						<ul aria-labelledby="coming-later" className="m-0 mt-4 list-none space-y-3 p-0">
							{product.coming.items.map((item) => (
								<li key={item} className="flex items-start gap-3">
									<PlannedMarker className="mt-0.5 h-6 w-7 shrink-0 text-ink-soft" />
									<span>{item}</span>
								</li>
							))}
						</ul>
					</div>
				</div>
			</div>
		</section>
	);
}
