import type { ReactNode } from "react";
import { Button } from "./Button";

/** Shown while a request is running. Say what is loading. */
export function Loading({ label = "Loading…" }: { label?: string }) {
	return (
		<div role="status" className="flex items-center justify-center gap-2 py-12 text-ink-muted">
			<svg className="h-4 w-4 animate-spin" fill="none" viewBox="0 0 24 24" aria-hidden>
				<circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
				<path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
			</svg>
			<span className="text-sm">{label}</span>
		</div>
	);
}

/** Shown when there is nothing yet. Say what to do next, and offer the action. */
export function Empty({ title, children, action }: { title: string; children?: ReactNode; action?: ReactNode }) {
	return (
		<section className="mx-auto grid max-w-lg justify-items-center gap-2 rounded-md border border-line bg-raised p-6 text-center">
			<h2 className="text-[15px] font-medium text-ink">{title}</h2>
			{children && <p className="max-w-sm text-sm leading-relaxed text-ink-soft">{children}</p>}
			{action && <div className="mt-2">{action}</div>}
		</section>
	);
}

/** Shown when a request failed. The message says what happened; Retry runs it again. */
export function ErrorState({ message, onRetry }: { message: string; onRetry: () => void }) {
	return (
		<div role="alert" className="mx-auto grid max-w-lg justify-items-center gap-3 rounded-md border border-line bg-raised p-5 text-center">
			<p className="flex items-center gap-2 text-sm text-ink">
				<span className="dot dot-bad" aria-hidden />
				{message}
			</p>
			<Button variant="line" onClick={onRetry}>
				Retry
			</Button>
		</div>
	);
}
