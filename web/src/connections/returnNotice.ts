import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";

export type ReturnNotice = { kind: "connected" | "error"; message: string };

const REASONS: Record<string, string> = {
	invalid_state: "That sign-in link was not valid or was already used. Start the connection again from the Connections tab.",
	expired: "The sign-in took longer than 10 minutes. Start it again.",
	denied: "The Google sign-in was cancelled, so nothing was connected.",
	missing_code: "Google did not send back a sign-in code. Try again.",
	token_rejected: "Google rejected the sign-in. Try again.",
	no_refresh_token:
		"Google did not allow ongoing access. Remove Margincite from your Google account's third-party access, then connect again.",
	google_unavailable: "Google could not be reached. Try again in a moment.",
	not_configured: "Google sign-in is not set up on the server. The variables to set are listed below.",
	server_error: "Something went wrong on the server. Try again in a moment.",
};

const FALLBACK = "The Google sign-in did not finish. Try again.";

/** The plain sentence for a `reason` code Google's return carries. */
export const reasonText = (reason: string | null): string => (reason && REASONS[reason]) || FALLBACK;

/**
 * Reads `?google=connected` or `?google=error&reason=` that Google's return leaves in the address,
 * keeps the message, and removes the parameters so a reload does not repeat it.
 */
export function useGoogleReturn(): ReturnNotice | null {
	const [params, setParams] = useSearchParams();
	const [notice] = useState<ReturnNotice | null>(() => {
		const result = params.get("google");
		if (result === "connected") return { kind: "connected", message: "Google is connected." };
		if (result === "error") return { kind: "error", message: reasonText(params.get("reason")) };
		return null;
	});

	useEffect(() => {
		if (!params.has("google")) return;
		const next = new URLSearchParams(params);
		next.delete("google");
		next.delete("reason");
		setParams(next, { replace: true });
	}, [params, setParams]);

	return notice;
}
