import type { ReactNode } from "react";

/** A framed block with a heading. The title also names the region for screen readers. */
export function Panel({ title, children }: { title: string; children: ReactNode }) {
	return (
		<section aria-label={title} className="grid gap-3 rounded-md border border-line bg-raised p-4">
			<h2 className="border-b border-line pb-2 text-sm font-medium text-ink">{title}</h2>
			{children}
		</section>
	);
}
