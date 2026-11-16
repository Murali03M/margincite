import { CourseMap } from "./CourseMap";
import { ArrowIcon, ControlSymbol, KeySwatch } from "./symbols";
import { content } from "../content";

const hero = content.hero;

/** The strip a printed map keeps in its margin: what sheet this is and how to read it. */
function MapMargin() {
	return (
		<div className="border-t-2 border-course bg-sheet xl:absolute xl:inset-x-0 xl:bottom-0">
			<div className="hero-pad flex flex-col gap-x-10 gap-y-3 py-3.5 xl:flex-row xl:items-center">
				<p className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
					<span className="label text-course-deep">{hero.sketchCaption}</span>
					<span className="label rounded-[4px] bg-course px-2 py-0.5 text-white">{hero.badge}</span>
					<span className="text-[0.9375rem] text-ink-soft">{hero.mapNote}</span>
				</p>
				<ul aria-label="Map key" className="flex flex-wrap gap-x-6 gap-y-2 xl:ml-auto">
					{hero.key.map((entry) => (
						<li key={entry.label} className="flex items-center gap-2 text-[0.9375rem] leading-tight">
							<KeySwatch symbol={entry.symbol} className="h-[1.15rem] w-auto shrink-0" />
							{entry.label}
						</li>
					))}
				</ul>
			</div>
		</div>
	);
}

/** The words and buttons. On a wide screen they sit in the map's title block. */
function TitleBlock() {
	return (
		<div className="title-block grid gap-x-12 gap-y-7 md:grid-cols-2 md:items-end xl:block">
			<h1>
				{hero.titleLead} <span className="text-ink">{hero.titleAccent}</span> {hero.titleTail}
			</h1>
			<div>
				<p className="lead xl:mt-6">{hero.body}</p>
				<div className="mt-7 flex flex-wrap gap-3 xl:flex-col">
					<a href={hero.primary.href} className="btn">
						{hero.primary.label}
						<ArrowIcon />
					</a>
					<a href={hero.secondary.href} className="btn btn-line">
						{hero.secondary.label}
					</a>
				</div>
				<p className="mt-6 flex items-center gap-2.5 text-ink-soft">
					<span aria-hidden className="h-2.5 w-2.5 shrink-0 rounded-full bg-course" />
					{hero.status}
				</p>
			</div>
		</div>
	);
}

export function Hero() {
	return (
		<section id="top" className="hero relative mx-auto max-w-[100rem]">
			<CourseMap viewBox="0 0 1600 900" tags className="absolute inset-0 hidden h-full w-full xl:block" />

			<div className="hero-pad relative z-10 py-12 xl:self-center xl:pb-24 xl:pt-8">
				<TitleBlock />
			</div>

			<figure className="border-t-2 border-course xl:hidden">
				<CourseMap viewBox="590 70 1010 740" labelSize={32} className="block aspect-[1010/740] w-full" />
			</figure>

			<ol
				aria-label={hero.sketchCaption}
				className="hero-pad grid gap-3 border-t-2 border-course py-6 sm:grid-cols-3 xl:hidden"
			>
				{hero.sketchNotes.map((note, i) => (
					<li key={note} className="flex items-center gap-3 font-medium leading-snug">
						<ControlSymbol label={String(i + 1)} className="h-11 w-11 shrink-0 text-course" />
						{note}
					</li>
				))}
			</ol>

			<MapMargin />
		</section>
	);
}
