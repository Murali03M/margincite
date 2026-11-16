import { Link } from "react-router-dom";
import { ApiError, errorMessage } from "../api/client";

/** True when the server's own message sends the person to Settings, so the link is offered only then. */
export function mentionsSettings(message: string): boolean {
	return /\bsettings\b/i.test(message);
}

/** A refusal from the server. It links to Settings only when its message says to go there. */
export function Refusal({ error }: { error: unknown }) {
	if (!error) return null;
	return (
		<p role="alert" className="notice">
			{errorMessage(error)}
			{error instanceof ApiError && error.status === 422 && mentionsSettings(error.message) && (
				<>
					{" "}
					<Link to="/settings" className="font-medium text-ink">
						Settings
					</Link>
				</>
			)}
		</p>
	);
}
