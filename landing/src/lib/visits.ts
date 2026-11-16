/**
 * The visit count in the top bar. GitHub Pages serves static files only, so the
 * count lives with Abacus, a free, open-source counting service with no account
 * and no cookies. A tab counts its first view and only reads the count after
 * that, so reloading does not inflate it. Any failure shows nothing.
 */

const SERVICE = "https://abacus.jasoncameron.dev";
const NAMESPACE = "margincite-site";
const COUNTER = "visits";
const PLACES = 6;
const TIMEOUT_MS = 20_000;

export const COUNTED_KEY = "margincite:visit-counted";

type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

export function visitsUrl(alreadyCounted: boolean): string {
	return `${SERVICE}/${alreadyCounted ? "get" : "hit"}/${NAMESPACE}/${COUNTER}`;
}

/** The count in fixed places with leading zeros, as a numbering stamp prints it. */
export function digitsOf(count: number): string[] {
	return String(Math.max(0, Math.floor(count))).padStart(PLACES, "0").split("");
}

export function formatCount(count: number): string {
	return new Intl.NumberFormat("en").format(count);
}

function read(storage: Storage | undefined): boolean {
	try {
		return storage?.getItem(COUNTED_KEY) === "1";
	} catch {
		return false;
	}
}

function remember(storage: Storage | undefined): void {
	try {
		storage?.setItem(COUNTED_KEY, "1");
	} catch {
		// Storage is blocked. The next reload counts again, which is acceptable.
	}
}

export async function loadVisits({
	fetcher = fetch,
	storage,
	timeoutMs = TIMEOUT_MS,
}: {
	fetcher?: typeof fetch;
	storage?: Storage;
	timeoutMs?: number;
} = {}): Promise<number | null> {
	const counted = read(storage);
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);
	try {
		const response = await fetcher(visitsUrl(counted), { signal: controller.signal, headers: { Accept: "application/json" } });
		if (!response.ok) return null;
		const body: unknown = await response.json();
		const value = (body as { value?: unknown } | null)?.value;
		if (typeof value !== "number" || !Number.isFinite(value) || value < 0) return null;
		if (!counted) remember(storage);
		return value;
	} catch {
		return null;
	} finally {
		clearTimeout(timer);
	}
}
