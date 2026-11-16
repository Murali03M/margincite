import { useState } from "react";
import type { ReactNode } from "react";
import { errorMessage } from "../api/client";
import type { SettingSource, TestStatus } from "../api/types";
import { Button } from "../ui/Button";
import { shortDate } from "../ui/format";
import { StatusTag, type Tone } from "../ui/StatusTag";

/** Where a setting comes from, in words. Never the value itself. */
export function KeyState({ set, source }: { set: boolean; source: SettingSource }) {
	if (!set) return <span className="text-sm text-ink-soft">Not set</span>;
	return (
		<span className="text-sm text-ink-soft">
			{source === "environment" ? "Saved, from the server file" : "Saved here"}
		</span>
	);
}

const STATUS_TONE: Record<TestStatus, Tone> = { ok: "course", error: "ink", unchecked: "line" };

/** The result of the last test of a key: a tag, the provider's reason when it failed, and when it ran. */
export function TestResult({ status, set, error, checkedAt }: { status: TestStatus; set: boolean; error: string | null; checkedAt: string | null }) {
	const word = !set ? "Not set up" : status === "ok" ? "Working" : status === "error" ? "Error" : "Not tested yet";
	return (
		<div className="grid gap-1.5">
			<div className="flex flex-wrap items-center gap-2">
				<StatusTag tone={set ? STATUS_TONE[status] : "line"}>{word}</StatusTag>
				{set && checkedAt && <span className="text-xs text-ink-muted">Tested {shortDate(checkedAt)}</span>}
			</div>
			{set && status === "error" && error && (
				<p role="alert" className="notice">
					{error}
				</p>
			)}
		</div>
	);
}

/** Remove with a confirming step, because a saved key cannot be read back and has to be typed again. */
export function ConfirmRemove({ what, onConfirm, busy, error }: { what: string; onConfirm: () => void; busy: boolean; error: unknown }) {
	const [asking, setAsking] = useState(false);
	return (
		<div className="grid gap-2">
			{asking ? (
				<div className="grid gap-2 rounded-md border border-line bg-raised p-3">
					<p className="text-sm text-ink">The saved {what} is deleted. You would have to enter it again.</p>
					<div className="flex gap-2">
						<Button compact busy={busy} aria-label={`Yes, remove the ${what}`} onClick={onConfirm}>
							Yes, remove
						</Button>
						<Button variant="line" compact onClick={() => setAsking(false)}>
							Keep it
						</Button>
					</div>
				</div>
			) : (
				<Button variant="line" compact aria-label={`Remove ${what}`} onClick={() => setAsking(true)}>
					Remove
				</Button>
			)}
			{error ? (
				<p role="alert" className="notice">
					{errorMessage(error)}
				</p>
			) : null}
		</div>
	);
}

/** A framed sub-section with a title, for one provider or one source inside a panel. */
export function Group({ title, children }: { title: string; children: ReactNode }) {
	return (
		<section aria-label={title} className="grid gap-3 rounded-md border border-line bg-canvas p-4">
			<h3 className="text-sm font-medium text-ink">{title}</h3>
			{children}
		</section>
	);
}

/** A refusal in plain words. Takes an error from a request, or a message already made from one. */
export function Problem({ error }: { error: unknown }) {
	if (!error) return null;
	return (
		<p role="alert" className="notice">
			{typeof error === "string" ? error : errorMessage(error)}
		</p>
	);
}
