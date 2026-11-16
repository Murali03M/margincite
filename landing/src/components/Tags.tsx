import type { Cost, Serves } from "../content";
import { content } from "../content";

/** Which kind of search a feature or fix serves: SEO, AIO or GEO, as small printed tags. */
export function ServesTags({ serves }: { serves: readonly Serves[] }) {
	return (
		<span className="inline-flex flex-wrap gap-1.5">
			{serves.map((area) => (
				<span key={area} className="rounded-[2px] border-[1.5px] border-ink px-1.5 py-px font-[family-name:var(--font-mono)] text-[0.75rem] font-bold leading-[1.35]">
					{area}
				</span>
			))}
		</span>
	);
}

/** What a feature costs, as a rubber stamp. Paid is stamped in seal red. */
export function CostStamp({ cost }: { cost: Cost }) {
	return <span className={"stamp label text-[0.6875rem] " + (cost === "paid" ? "text-seal" : "text-ink")}>{content.features.costs[cost]}</span>;
}
