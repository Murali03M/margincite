import { useState } from "react";
import { errorMessage } from "../api/client";
import { Button } from "../ui/Button";

export { Panel } from "../ui/Panel";

/** Names the server settings that are missing, exactly as they are written in backend/.env. */
export function MissingSettings({ names, why }: { names: string[]; why: string }) {
	return (
		<p className="notice">
			Set{" "}
			{names.map((name, index) => (
				<span key={name}>
					{index > 0 && (index === names.length - 1 ? " and " : ", ")}
					<code className="font-medium">{name}</code>
				</span>
			))}{" "}
			in <code>backend/.env</code>, then restart the backend. {why}
		</p>
	);
}

/** Disconnect with a confirming step, because it deletes the synced numbers. */
export function DisconnectButton({ busy, error, onConfirm }: { busy: boolean; error: unknown; onConfirm: () => void }) {
	const [asking, setAsking] = useState(false);
	return (
		<div className="grid gap-2">
			{asking ? (
				<div className="grid gap-2 rounded-md border border-line bg-raised p-3">
					<p className="text-sm text-ink">Disconnecting deletes the synced numbers and the stored key or token. Continue?</p>
					<div className="flex gap-2">
						<Button compact busy={busy} onClick={onConfirm}>
							Yes, disconnect
						</Button>
						<Button variant="line" compact onClick={() => setAsking(false)}>
							Keep it
						</Button>
					</div>
				</div>
			) : (
				<Button variant="line" compact onClick={() => setAsking(true)}>
					Disconnect
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
