import { http, HttpResponse } from "msw";
import { keyword, metrics, settingsReady, siteSettings, speedReport, summary } from "./fixtures";

export const GOOD_TOKEN = "good-token";

type Meta = { total: number; page: number; limit: number };

/** A successful envelope, the way the backend writes it. */
export function ok<T>(data: T, meta: Meta | null = null, status = 200) {
	return HttpResponse.json({ success: true, data, error: null, meta }, { status });
}

/** An error envelope with the server's plain message. */
export function fail(status: number, error: string) {
	return HttpResponse.json({ success: false, data: null, error, meta: null }, { status });
}

export const me = { user_id: "u1", workspace_id: "w1", email: "owner@example.com" };

/** Handlers every test starts with. A test adds its own with `server.use(...)`. */
export const defaultHandlers = [
	http.get("/api/v1/me", ({ request }) =>
		request.headers.get("authorization") === `Bearer ${GOOD_TOKEN}`
			? ok(me)
			: fail(401, "Token not recognised"),
	),
	// The sidebar lists the sites with their score and open count on every signed-in screen.
	http.get("/api/v1/sites", () => ok([])),
	http.get("/api/v1/sites/:id/summary", () => ok(summary())),
	// Every site page and card reads the metrics; a test that cares overrides this.
	http.get("/api/v1/sites/:id/metrics", () => ok(metrics())),
	// The sites list reads these for its first-run guide, and the checks and AI tabs for their panels.
	http.get("/api/v1/settings", () => ok(settingsReady())),
	http.get("/api/v1/sites/:id/keywords", () => ok([keyword()])),
	http.get("/api/v1/sites/:id/keywords/volumes", () => ok({ status: "idle", checked_at: null, error: null })),
	http.get("/api/v1/sites/:id/settings", () => ok(siteSettings())),
	http.get("/api/v1/sites/:id/speed", () => ok(speedReport())),
	http.get("/api/v1/sites/:id/ai-overviews", () => ok([])),
];
