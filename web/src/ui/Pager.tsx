import type { Meta } from "../api/types";
import { Button } from "./Button";

/** Previous and Next for a paged list. Draws nothing when everything fits on one page. */
export function Pager({ meta, onPage }: { meta: Meta; onPage: (page: number) => void }) {
	const pages = Math.max(1, Math.ceil(meta.total / Math.max(1, meta.limit)));
	if (pages <= 1) return null;
	return (
		<nav aria-label="Pages of results" className="flex flex-wrap items-center gap-4">
			<Button variant="line" compact disabled={meta.page <= 1} onClick={() => onPage(meta.page - 1)} aria-label="Previous page">
				Previous
			</Button>
			<span>
				Page {meta.page} of {pages}
			</span>
			<Button variant="line" compact disabled={meta.page >= pages} onClick={() => onPage(meta.page + 1)} aria-label="Next page">
				Next
			</Button>
		</nav>
	);
}
