import { Link } from "react-router-dom";
import { useKeywords } from "../api/hooks";
import { useSettings } from "../api/settingsHooks";
import type { Site } from "../api/types";

type Step = { label: string; done: boolean; to: string; note?: string; optional?: boolean };

/**
 * A short checklist for a new install: contact, site, keywords, an AI provider, and optional search data.
 * It goes away once the four required steps are done. If the settings cannot be read it draws nothing,
 * so the page behind it still works.
 */
export function FirstRun({ sites }: { sites: Site[] }) {
	const settings = useSettings();
	const first = sites[0];
	const keywords = useKeywords(first?.id ?? "", Boolean(first));
	if (!settings.data) return null;
	const config = settings.data;

	const steps: Step[] = [
		{ label: "Add a crawler contact", done: Boolean(config.crawler.contact), to: "/settings", note: "So site owners know who is visiting." },
		{ label: "Add your website", done: sites.length > 0, to: "/sites/new", note: "The address of your website." },
		{
			label: "Add keywords",
			done: (keywords.data?.length ?? 0) > 0,
			to: first ? `/sites/${first.id}/keywords` : "/sites/new",
			note: "The searches you want to be found for.",
		},
		{ label: "Connect an AI provider", done: config.providers.some((p) => p.key_set), to: "/settings", note: "Any one of Claude, ChatGPT, Grok, Gemini or Perplexity." },
		{
			label: "Connect search data",
			done: config.dataforseo.login_set && config.dataforseo.password_set,
			to: "/settings",
			note: "Optional and paid per use. Unlocks rank tracking, AI Overviews and search volume.",
			optional: true,
		},
	];
	const required = steps.filter((step) => !step.optional);
	if (required.every((step) => step.done)) return null;
	const doneCount = required.filter((step) => step.done).length;

	return (
		<section aria-label="Get set up" className="grid gap-3 rounded-md border border-line bg-raised p-4">
			<div className="flex flex-wrap items-baseline justify-between gap-2">
				<h2 className="text-[15px] font-medium text-ink">Get set up</h2>
				<p className="text-xs font-medium text-ink-soft">{`${doneCount} of ${required.length} steps done`}</p>
			</div>
			<ol className="grid gap-2">
				{steps.map((step) => (
					<li key={step.label} className="flex items-start gap-3 text-sm">
						<span
							aria-hidden
							className={`dot mt-2 ${step.done ? "dot-good" : "dot-info"}`}
						/>
						<span className="grid gap-0.5">
							{step.done ? (
								<span className="font-medium text-ink-muted line-through">{step.label}</span>
							) : (
								<Link to={step.to} className="font-medium text-ink hover:underline">
									{step.label}
								</Link>
							)}
							<span className="text-xs text-ink-muted">{step.done ? "Done" : "To do"}</span>
							{step.note && !step.done && <span className="text-xs text-ink-muted">{step.note}</span>}
						</span>
					</li>
				))}
			</ol>
		</section>
	);
}
