import { Link } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useMetrics } from "../api/hooks";
import { Button } from "../ui/Button";
import { Loading } from "../ui/States";
import { buildTiles, type Tile } from "./tiles";

const CHANGE_CLASS = {
	up: "text-ink-soft",
	down: "text-ink-soft",
	flat: "text-ink-muted",
} as const;

function TileView({ tile }: { tile: Tile }) {
	const missing = tile.value === null;
	return (
		<li title={tile.source} className="flex flex-col gap-1 rounded-md border border-line bg-canvas p-4">
			<span data-tile-label className="label font-medium">
				{tile.label}
			</span>
			{missing ? (
				<>
					<span className="text-2xl font-medium text-ink-muted">n/a</span>
					{tile.reason && <p className="text-xs leading-relaxed text-ink-muted">{tile.reason}</p>}
					{tile.fix && (
						<Link to={tile.fix.to} className="text-xs font-medium text-ink hover:underline">
							{tile.fix.label} →
						</Link>
					)}
				</>
			) : (
				<>
					<span className="text-2xl font-medium text-ink">{tile.value}</span>
					{tile.change && (
						<span className={`text-xs font-medium ${CHANGE_CLASS[tile.change.direction]}`}>{tile.change.text}</span>
					)}
					<span className="text-xs leading-relaxed text-ink-muted">{tile.caption}</span>
				</>
			)}
		</li>
	);
}

/**
 * The project card: nine numbers for one site. Each says where it comes from; a number that
 * cannot be shown says n/a, why, and where to fix it.
 */
export function MetricCards({ siteId, label = "Project card" }: { siteId: string; label?: string }) {
	const metrics = useMetrics(siteId);

	if (metrics.isPending) return <Loading label="Loading the numbers…" />;
	if (metrics.isError) {
		return (
			<div role="alert" className="flex flex-wrap items-center gap-3 rounded-md border border-line bg-raised p-4">
				<p className="text-sm font-medium text-ink">{errorMessage(metrics.error)}</p>
				<Button variant="line" compact onClick={() => void metrics.refetch()}>
					Retry
				</Button>
			</div>
		);
	}
	return (
		<ul aria-label={label} className="grid grid-cols-2 gap-3 sm:grid-cols-3">
			{buildTiles(siteId, metrics.data).map((tile) => (
				<TileView key={tile.label} tile={tile} />
			))}
		</ul>
	);
}
