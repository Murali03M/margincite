/**
 * The development token lives in sessionStorage. If the browser blocks storage,
 * it is kept in memory for the life of the page instead, so nothing throws.
 */
const KEY = "margincite.token";

let memory: string | null = null;
const listeners = new Set<() => void>();

function notify() {
	listeners.forEach((listener) => listener());
}

export function getToken(): string | null {
	try {
		return sessionStorage.getItem(KEY);
	} catch {
		return memory;
	}
}

export function setToken(token: string): void {
	try {
		sessionStorage.setItem(KEY, token);
	} catch {
		memory = token;
	}
	notify();
}

export function clearToken(): void {
	memory = null;
	try {
		sessionStorage.removeItem(KEY);
	} catch {
		/* blocked: nothing stored */
	}
	notify();
}

/** For useSyncExternalStore: tells React when the token changes. */
export function subscribeToken(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}
