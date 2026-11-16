import { useEffect, useState } from "react";
import { content } from "../content";
import { digitsOf, formatCount, loadVisits } from "../lib/visits";

type State = { phase: "off" } | { phase: "loading" } | { phase: "ready"; count: number };

const TEN = Array.from({ length: 10 }, (_, i) => i);

function sessionStore() {
	try {
		return window.sessionStorage;
	} catch {
		return undefined;
	}
}

const defaultLoad = () => loadVisits({ storage: sessionStore() });

/**
 * The visit count, printed as a numbering stamp: six fixed places, each a wheel
 * that rolls to its digit once the count arrives. The server renders nothing, so
 * no number appears that the counting service did not give.
 */
export function VisitCounter({ load = defaultLoad }: { load?: () => Promise<number | null> }) {
	const [state, setState] = useState<State>({ phase: "off" });
	const [rolled, setRolled] = useState(false);

	useEffect(() => {
		let live = true;
		setState({ phase: "loading" });
		load().then((count) => {
			if (live) setState(count === null ? { phase: "off" } : { phase: "ready", count });
		});
		return () => {
			live = false;
		};
	}, [load]);

	useEffect(() => {
		if (state.phase !== "ready") return;
		const frame = requestAnimationFrame(() => setRolled(true));
		return () => cancelAnimationFrame(frame);
	}, [state.phase]);

	if (state.phase === "off") return null;
	const digits = state.phase === "ready" ? digitsOf(state.count) : digitsOf(0);

	return (
		<span
			className={
				"inline-flex -rotate-2 flex-col items-center rounded-[3px] border-2 border-ink px-1.5 pb-0.5 pt-[0.2rem] " +
				(state.phase === "loading" ? "animate-pulse" : "")
			}
			aria-busy={state.phase === "loading"}
		>
			<span className="label text-[0.5625rem] leading-none tracking-[0.16em] sm:text-[0.625rem]" aria-hidden>
				{content.visits.label}
			</span>
			<span className="inline-flex font-[family-name:var(--font-mono)] text-[0.875rem] font-bold sm:text-[0.9375rem]" aria-hidden>
				{digits.map((digit, place) => (
					<span
						key={place}
						data-digit
						className="wheel w-[1.1ch] text-center"
						style={{ ["--digit" as string]: rolled ? Number(digit) : 0, ["--place" as string]: place }}
					>
						<span>
							{TEN.map((n) => (
								<span key={n}>{n}</span>
							))}
						</span>
					</span>
				))}
			</span>
			<span className="sr-only" role="status">
				{state.phase === "ready" ? content.visits.spoken(formatCount(state.count)) : content.visits.loading}
			</span>
		</span>
	);
}
