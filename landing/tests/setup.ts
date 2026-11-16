import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

// jsdom has neither of these. Tests that care about them stub their own.
class NoopIntersectionObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
	takeRecords() {
		return [];
	}
}
globalThis.IntersectionObserver ??= NoopIntersectionObserver as unknown as typeof IntersectionObserver;
if (typeof window !== "undefined") window.matchMedia ??= ((query: string) => ({
	matches: false,
	media: query,
	addEventListener() {},
	removeEventListener() {},
})) as unknown as typeof window.matchMedia;

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});
