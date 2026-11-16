import { useState } from "react";
import { content } from "../content";
import { ArrowIcon, CheckIcon, CopyIcon, GitHubMark } from "./marks";

const getIt = content.getIt;

type CopyState = "idle" | "done" | "failed";

function CopyButton() {
	const [state, setState] = useState<CopyState>("idle");

	async function copy() {
		try {
			await navigator.clipboard.writeText(getIt.commands.join("\n"));
			setState("done");
			setTimeout(() => setState("idle"), 2400);
		} catch {
			setState("failed");
		}
	}

	return (
		<>
			<button type="button" onClick={copy} className="btn btn-line">
				{state === "done" ? <CheckIcon /> : <CopyIcon />}
				{state === "done" ? getIt.copy.done : getIt.copy.idle}
			</button>
			<p role="status" className="basis-full text-[0.9375rem] font-semibold">
				{state === "failed" ? getIt.copy.failed : ""}
			</p>
		</>
	);
}

export function GetIt() {
	return (
		<section id="get-it" className="on-marker bg-marker py-20 lg:py-28">
			<div className="wrap grid items-start gap-x-14 gap-y-12 lg:grid-cols-12">
				<div className="min-w-0 lg:col-span-5">
					<h2 className="heading">{getIt.heading}</h2>
					<p className="lead mt-5">{getIt.body}</p>
					<div className="mt-8 flex flex-wrap gap-3">
						<a href={getIt.primary.href} className="btn">
							<GitHubMark />
							{getIt.primary.label}
							<ArrowIcon />
						</a>
						<CopyButton />
					</div>
					<p className="measure mt-4 text-[0.9375rem]">{getIt.costs}</p>
				</div>
				<div className="min-w-0 lg:col-span-7">
					<div role="region" aria-label={getIt.commandsLabel} className="paper rounded-[3px] border-2 border-ink">
						<div className="label border-b-2 border-ink px-4 py-2.5 text-ink-soft">Terminal</div>
						<pre className="m-0 overflow-x-auto px-4 py-4 text-[0.8125rem] leading-[1.9] max-sm:whitespace-pre-wrap max-sm:break-all sm:text-[0.9375rem]">
							{getIt.commands.map((line) => (
								<span key={line} className="block">
									<span className="select-none text-ink-faint">$ </span>
									{line}
								</span>
							))}
						</pre>
						<p className="border-t border-rule px-4 py-3 text-[0.9375rem]">{getIt.after}</p>
					</div>
				</div>
			</div>
		</section>
	);
}
