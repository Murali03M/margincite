import { envelope } from "./envelope";

type Reply = { status: number; contentType: string; body: string };
type Provider = { provider: string; key_set: boolean; source: string; model: string; default_model: string; status: string; error: string | null; checked_at: string | null };

const IDS = ["claude", "chatgpt", "grok", "gemini", "perplexity"];

/**
 * Fake answers for the settings screen: providers, the crawler contact and search data.
 * A saved key is only remembered as "set"; the value is dropped, as the real server does.
 */
export function makeSettingsRoutes() {
	const providers: Provider[] = IDS.map((id) => ({
		provider: id,
		key_set: false,
		source: "none",
		model: `${id}-default`,
		default_model: `${id}-default`,
		status: "unchecked",
		error: null,
		checked_at: null,
	}));
	let contact: string | null = null;
	let dataforseo = false;

	const settings = () => ({
		providers,
		google: { client_id_set: false, client_secret_set: false, source: "none" },
		dataforseo: {
			login_set: dataforseo,
			password_set: dataforseo,
			source: dataforseo ? "workspace" : "none",
			status: dataforseo ? "ok" : "unchecked",
			error: null,
			balance: dataforseo ? 50 : null,
			checked_at: dataforseo ? "2026-11-15T10:00:00Z" : null,
		},
		pagespeed: { key_set: false, source: "none" },
		crawler: { contact, contact_source: contact ? "workspace" : "none" },
		secret_key_ready: true,
	});

	return (path: string, method: string, body: Record<string, unknown>): Reply | null => {
		if (path === "/settings" && method === "GET") return envelope(settings());
		if (path === "/usage") return envelope([]);
		const provider = /^\/settings\/providers\/(\w+)$/.exec(path);
		if (provider && method === "PUT") {
			const row = providers.find((p) => p.provider === provider[1]);
			if (!row) return null;
			if (typeof body.api_key === "string" && body.api_key) {
				Object.assign(row, { key_set: true, source: "workspace", status: "ok", checked_at: "2026-11-15T10:00:00Z" });
			}
			return envelope(row);
		}
		if (path === "/settings/crawler" && method === "PUT") {
			contact = String(body.contact);
			return envelope({ saved: true });
		}
		if (path === "/settings/dataforseo" && method === "PUT") {
			dataforseo = true;
			return envelope({ saved: true });
		}
		return null;
	};
}
