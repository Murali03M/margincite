import { content } from "../content";
import { Exhibit } from "./Exhibit";
import { ArrowIcon, CheckIcon, GitHubMark } from "./marks";

const hero = content.hero;

export function Hero() {
	return (
		<section id="top" className="pb-14 pt-10 sm:pt-14 lg:pb-16 lg:pt-16">
			<div className="wrap grid gap-x-14 gap-y-8 lg:grid-cols-12 lg:grid-rows-[1fr_auto_auto_1fr] lg:gap-y-0">
				<h1 className="display lg:col-span-5 lg:row-start-2">
					{hero.titleLead}{" "}
					<span className="bg-marker box-decoration-clone px-[0.08em]">{hero.titleAccent}</span>
				</h1>
				<div className="lg:col-span-7 lg:col-start-6 lg:row-span-4 lg:row-start-1 lg:self-center">
					<Exhibit />
				</div>
				<div className="lg:col-span-5 lg:row-start-3">
					<p className="lead measure mt-2 text-ink-soft lg:mt-7">{hero.body}</p>
					<div className="mt-9 flex flex-wrap gap-3">
						<a href={hero.primary.href} className="btn">
							<GitHubMark />
							{hero.primary.label}
							<ArrowIcon />
						</a>
						<a href={hero.secondary.href} className="btn btn-line">
							{hero.secondary.label}
						</a>
					</div>
					<p className="mt-6 text-[0.9375rem] text-ink-soft">{hero.status}</p>
				</div>
			</div>
			<ul
				aria-label={hero.strip.label}
				className="wrap mb-0 mt-20 grid list-none gap-x-8 gap-y-4 border-y border-ink p-0 py-5 sm:grid-cols-2 lg:mt-24 lg:grid-cols-4"
			>
				{hero.strip.items.map((item) => (
					<li key={item} className="label flex items-center gap-2.5 text-[0.8125rem]">
						<CheckIcon className="h-4 w-4 shrink-0 text-ink" />
						{item}
					</li>
				))}
			</ul>
		</section>
	);
}
