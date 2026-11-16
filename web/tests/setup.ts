import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, vi } from "vitest";
import { clearToken } from "../src/auth/session";
import { server } from "./mocks/server";

// Several screens read three or four endpoints in a row; allow for a busy machine.
configure({ asyncUtilTimeout: 4000 });

// Node's fetch needs absolute addresses; the app uses same-origin paths.
const nodeFetch = globalThis.fetch;
globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) =>
	nodeFetch(typeof input === "string" && input.startsWith("/") ? new URL(input, window.location.origin) : input, init)) as typeof fetch;

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));

afterEach(() => {
	cleanup();
	server.resetHandlers();
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
	clearToken();
});

afterAll(() => server.close());
