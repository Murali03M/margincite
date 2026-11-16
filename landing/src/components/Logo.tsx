import { content } from "../content";
import { Marker } from "./marks";

/** An evidence marker standing beside the name. */
export function Logo({ className, onInk = false }: { className?: string; onInk?: boolean }) {
	return (
		<span className={"inline-flex items-center gap-2 " + (onInk ? "text-on-plate " : "text-ink ") + (className ?? "")}>
			<Marker n="M" className="h-8 w-[2.2rem] shrink-0" />
			<span className="font-[family-name:var(--font-display)] text-[1rem] font-[850] uppercase max-[374px]:sr-only min-[440px]:text-[1.35rem] leading-none tracking-[0.01em] [font-stretch:118%]">
				{content.brand}
			</span>
		</span>
	);
}
