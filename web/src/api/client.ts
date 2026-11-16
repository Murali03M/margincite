import { getToken } from "../auth/session";
import type { Meta } from "./types";

const BASE = "/api/v1";

export const UNREACHABLE = "Margincite can't be reached. Check that the backend is running.";
const UNEXPECTED = "Margincite sent an unexpected response. Try again in a moment.";

export class ApiError extends Error {
	readonly status: number;
	constructor(status: number, message: string) {
		super(message);
		this.name = "ApiError";
		this.status = status;
	}
}

type Envelope<T> = { success: boolean; data: T | null; error: string | null; meta: Meta | null };

export type ApiInit = { method?: string; body?: unknown; token?: string | null };

function fallbackMessage(status: number): string {
	if (status === 502 || status === 503 || status === 504) return UNREACHABLE;
	if (status === 401) return "Your session has ended. Sign in again.";
	return UNEXPECTED;
}

async function readEnvelope<T>(res: Response): Promise<Envelope<T> | null> {
	try {
		return (await res.json()) as Envelope<T>;
	} catch {
		return null;
	}
}

/**
 * Calls the API. `path` is relative to /api/v1, for example "/sites".
 * Returns the envelope's data and meta; throws ApiError with the server's plain message otherwise.
 */
export async function api<T>(path: string, init: ApiInit = {}): Promise<{ data: T; meta: Meta | null }> {
	const token = init.token === undefined ? getToken() : init.token;
	const headers: Record<string, string> = { Accept: "application/json" };
	if (token) headers.Authorization = `Bearer ${token}`;
	if (init.body !== undefined) headers["Content-Type"] = "application/json";

	let res: Response;
	try {
		res = await fetch(`${BASE}${path}`, {
			method: init.method ?? "GET",
			headers,
			body: init.body === undefined ? undefined : JSON.stringify(init.body),
		});
	} catch {
		throw new ApiError(0, UNREACHABLE);
	}

	const envelope = await readEnvelope<T>(res);
	if (!res.ok || !envelope || envelope.success !== true) {
		throw new ApiError(res.status, envelope?.error ?? fallbackMessage(res.status));
	}
	return { data: envelope.data as T, meta: envelope.meta ?? null };
}

/** Builds "?a=1&b=2" from the parameters that have a value. */
export function query(params: Record<string, string | number | undefined>): string {
	const search = new URLSearchParams();
	for (const [key, value] of Object.entries(params)) {
		if (value !== undefined && value !== "") search.set(key, String(value));
	}
	const text = search.toString();
	return text ? `?${text}` : "";
}

/** A message that is safe to show for anything a request can throw. */
export function errorMessage(error: unknown): string {
	return error instanceof ApiError ? error.message : UNEXPECTED;
}
