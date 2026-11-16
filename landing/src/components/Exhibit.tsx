import type { CSSProperties } from "react";
import { content } from "../content";
import { Marker } from "./marks";

const exhibit = content.hero.exhibit;

function Document() {
	return (
		<div className="paper min-w-0 rounded-[3px]">
			<div className="flex items-center justify-between gap-3 border-b border-rule px-4 py-2.5">
				<span className="truncate font-[family-name:var(--font-mono)] text-[0.8125rem] text-ink-soft">{exhibit.file}</span>
				<span className="label shrink-0 text-ink-faint">Evidence</span>
			</div>
			<ol className="m-0 list-none p-0 py-2 font-[family-name:var(--font-mono)] text-[0.8125rem] sm:text-[0.9375rem]">
				{exhibit.lines.map((line, i) => (
					<li key={i} className="flex min-h-[2.15rem] items-center gap-3 px-3 sm:gap-4 sm:px-4">
						<span className="w-5 shrink-0 select-none text-right text-ink-faint">{i + 1}</span>
						<span className={"whitespace-pre " + ("marker" in line ? "font-bold text-seal" : "")}>{line.text || " "}</span>
						{"marker" in line && (
							<span
								data-marker={line.marker}
								className="set-marker ml-auto -my-1 shrink-0"
								style={{ "--step": line.marker - 1 } as CSSProperties}
							>
								<Marker n={line.marker} className="h-9 w-10 sm:h-10 sm:w-11" />
							</span>
						)}
					</li>
				))}
			</ol>
		</div>
	);
}

function Tag() {
	return (
		<div className="paper place-tag overflow-hidden rounded-[3px] border-[1.5px] border-ink">
			<div className="label bg-ink px-3 py-2 text-marker">{exhibit.tag.title}</div>
			<dl className="m-0">
				{exhibit.tag.rows.map((row) => (
					<div key={row.label} className="grid grid-cols-[4.25rem_1fr] gap-3 border-t border-rule px-3 py-2 first:border-t-0">
						<dt className="label pt-[0.2rem] text-ink-faint">{row.label}</dt>
						<dd className="m-0 font-[family-name:var(--font-mono)] text-[0.8125rem] font-bold leading-snug">{row.value}</dd>
					</div>
				))}
			</dl>
		</div>
	);
}

/** The example finding, laid on the bench: the file, three markers, a scale, and its label. */
export function Exhibit() {
	return (
		<figure aria-label={exhibit.label} className="m-0">
			<div className="mb-3 flex flex-wrap items-center gap-x-3 gap-y-1">
				<span className="label rounded-[3px] bg-ink px-2 py-1 text-marker">{exhibit.badge}</span>
				<span className="text-[0.875rem] text-ink-soft">{exhibit.note}</span>
			</div>
			<div className="relative rounded-[4px] bg-bench-deep p-3 sm:p-6 lg:p-8 lg:pb-14" aria-hidden>
				<div className="grid grid-cols-[1.5rem_minmax(0,1fr)] grid-rows-[1fr_1.5rem] gap-2">
					<div className="ruler ruler-v lay-ruler" />
					<Document />
					<div />
					<div className="ruler lay-ruler" />
				</div>
				<div className="mt-4 lg:absolute lg:-bottom-8 lg:right-6 lg:mt-0 lg:w-[19rem]">
					<Tag />
				</div>
			</div>
		</figure>
	);
}
