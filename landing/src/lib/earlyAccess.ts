import type { ErrorReason } from "../content";

export type SubmitResult = { ok: true } | { ok: false; reason: ErrorReason };

type SubmitInput = { email: string; honeypot: string };
type SubmitDeps = { endpoint: string | undefined; fetchFn: typeof fetch };

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const MAX_EMAIL_LENGTH = 254;
const TIMEOUT_MS = 10_000;

export function isValidEmail(raw: string): boolean {
	const email = raw.trim();
	return email.length <= MAX_EMAIL_LENGTH && EMAIL_PATTERN.test(email);
}

function isHttpsUrl(value: string | undefined): value is string {
	if (!value) return false;
	try {
		return new URL(value).protocol === "https:";
	} catch {
		return false;
	}
}

async function send(
	email: string,
	endpoint: string,
	fetchFn: typeof fetch,
): Promise<SubmitResult> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
	try {
		const response = await fetchFn(endpoint, {
			method: "POST",
			headers: {
				Accept: "application/json",
				"Content-Type": "application/json",
			},
			body: JSON.stringify({ email }),
			signal: controller.signal,
		});
		if (response.ok) return { ok: true };
		console.error(`Early-access sign-up was rejected with status ${response.status}`);
		return { ok: false, reason: "rejected" };
	} catch (error) {
		console.error("Early-access sign-up could not reach the service", error);
		return { ok: false, reason: "network" };
	} finally {
		clearTimeout(timer);
	}
}

export async function submitEarlyAccess(
	input: SubmitInput,
	deps: SubmitDeps,
): Promise<SubmitResult> {
	// A filled honeypot means a bot. Answer as if it worked and send nothing.
	if (input.honeypot.trim() !== "") return { ok: true };
	if (!isValidEmail(input.email)) return { ok: false, reason: "invalid_email" };
	if (!isHttpsUrl(deps.endpoint)) {
		console.error("VITE_EARLY_ACCESS_ENDPOINT is missing or is not an https address");
		return { ok: false, reason: "not_configured" };
	}
	return send(input.email.trim(), deps.endpoint, deps.fetchFn);
}
