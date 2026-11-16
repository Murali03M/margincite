import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { providerOut, settings, settingsReady } from "../mocks/fixtures";
import type { ProviderId, Settings, UsageRow } from "../../src/api/types";
import { renderApp } from "../render";

const CANARY = "canary-not-a-real-key";

type Calls = { path: string; method: string; body: unknown; search: string }[];

/** A settings store that remembers what was sent, and answers the way the backend does. */
function mockSettings(initial: Settings, usage: UsageRow[] = []) {
	const state = { data: initial, calls: [] as Calls, providerReply: null as null | ((p: ProviderId) => unknown) };
	const record = async (request: Request) => {
		const url = new URL(request.url);
		const text = await request.text();
		state.calls.push({ path: url.pathname.replace("/api/v1", ""), method: request.method, body: text ? JSON.parse(text) : null, search: url.search });
	};
	server.use(
		http.get("/api/v1/sites", () => ok([])),
		http.get("/api/v1/settings", () => ok(state.data)),
		http.get("/api/v1/usage", ({ request }) => {
			state.calls.push({ path: "/usage", method: "GET", body: null, search: new URL(request.url).search });
			return ok(usage);
		}),
		http.put("/api/v1/settings/providers/:provider", async ({ request, params }) => {
			await record(request.clone());
			const id = params.provider as ProviderId;
			const body = state.calls[state.calls.length - 1].body as { api_key?: string; model?: string };
			const current = state.data.providers.find((p) => p.provider === id)!;
			const next = state.providerReply
				? (state.providerReply(id) as ReturnType<typeof providerOut>)
				: { ...current, key_set: body.api_key ? true : current.key_set, source: "workspace" as const, model: body.model ?? current.model, status: "ok" as const, checked_at: "2026-11-15T10:00:00Z" };
			state.data = { ...state.data, providers: state.data.providers.map((p) => (p.provider === id ? next : p)) };
			return ok(next);
		}),
		http.post("/api/v1/settings/providers/:provider/test", async ({ request, params }) => {
			await record(request.clone());
			const current = state.data.providers.find((p) => p.provider === params.provider)!;
			const next = { ...current, status: "error" as const, error: "The provider said the key is not valid.", checked_at: "2026-11-15T11:00:00Z" };
			state.data = { ...state.data, providers: state.data.providers.map((p) => (p.provider === params.provider ? next : p)) };
			return ok(next);
		}),
		http.delete("/api/v1/settings/providers/:provider", async ({ request, params }) => {
			await record(request.clone());
			state.data = { ...state.data, providers: state.data.providers.map((p) => (p.provider === params.provider ? providerOut(p.provider) : p)) };
			return ok({ deleted: true });
		}),
		http.put("/api/v1/settings/google", async ({ request }) => {
			await record(request.clone());
			state.data = { ...state.data, google: { client_id_set: true, client_secret_set: true, source: "workspace" } };
			return ok({ saved: true });
		}),
		http.delete("/api/v1/settings/google", async ({ request }) => {
			await record(request.clone());
			state.data = { ...state.data, google: { client_id_set: false, client_secret_set: false, source: "none" } };
			return ok({ deleted: true });
		}),
		http.put("/api/v1/settings/dataforseo", async ({ request }) => {
			await record(request.clone());
			state.data = { ...state.data, dataforseo: { login_set: true, password_set: true, source: "workspace", status: "ok", error: null, balance: 12.5, checked_at: "2026-11-15T10:00:00Z" } };
			return ok({ saved: true });
		}),
		http.post("/api/v1/settings/dataforseo/test", async ({ request }) => {
			await record(request.clone());
			state.data = { ...state.data, dataforseo: { ...state.data.dataforseo, status: "error", error: "DataForSEO says the account is out of credit.", balance: 0 } };
			return ok({ tested: true });
		}),
		http.delete("/api/v1/settings/dataforseo", async ({ request }) => {
			await record(request.clone());
			state.data = settings({ ...state.data, dataforseo: settings().dataforseo });
			return ok({ deleted: true });
		}),
		http.put("/api/v1/settings/pagespeed", async ({ request }) => {
			await record(request.clone());
			state.data = { ...state.data, pagespeed: { key_set: true, source: "workspace" } };
			return ok({ saved: true });
		}),
		http.delete("/api/v1/settings/pagespeed", async ({ request }) => {
			await record(request.clone());
			state.data = { ...state.data, pagespeed: { key_set: false, source: "none" } };
			return ok({ deleted: true });
		}),
		http.delete("/api/v1/settings/crawler", async ({ request }) => {
			await record(request.clone());
			state.data = { ...state.data, crawler: { contact: null, contact_source: "none" } };
			return ok({ deleted: true });
		}),
		http.put("/api/v1/settings/crawler", async ({ request }) => {
			await record(request.clone());
			const body = state.calls[state.calls.length - 1].body as { contact: string };
			if (!body.contact.includes("@") && !body.contact.startsWith("http")) return fail(422, "Enter an email address or an http(s) address.");
			state.data = { ...state.data, crawler: { contact: body.contact, contact_source: "workspace" } };
			return ok({ saved: true });
		}),
	);
	return state;
}

const section = (name: string) => screen.getByRole("region", { name });

test("settings shows loading, then every group, and the header links to it", async () => {
	mockSettings(settings());
	renderApp("/");
	await userEvent.click(await screen.findByRole("link", { name: "Settings" }));
	expect(await screen.findByRole("heading", { level: 1, name: "Settings" })).toBeInTheDocument();
	for (const title of ["AI providers", "Google Search Console sign-in", "Search data (DataForSEO)", "Page speed", "Crawler", "Usage"]) {
		expect(section(title)).toBeInTheDocument();
	}
});

test("a failed load says so and Retry loads again", async () => {
	let calls = 0;
	server.use(
		http.get("/api/v1/settings", () => {
			calls += 1;
			return calls === 1 ? fail(500, "The server hit a problem.") : ok(settings());
		}),
		http.get("/api/v1/usage", () => ok([])),
	);
	renderApp("/settings");
	expect(await screen.findByText("The server hit a problem.")).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("heading", { name: "AI providers" })).toBeInTheDocument();
});

test("each provider shows its status and where its key comes from, never the key", async () => {
	mockSettings(
		settings({
			providers: [
				providerOut("claude", { key_set: true, source: "workspace", status: "ok", checked_at: "2026-11-14T10:00:00Z" }),
				providerOut("chatgpt", { key_set: true, source: "environment", status: "error", error: "Out of credit with the provider." }),
				providerOut("grok"),
				providerOut("gemini"),
				providerOut("perplexity"),
			],
		}),
	);
	renderApp("/settings");
	const claude = within(await screen.findByRole("region", { name: "Claude" }));
	expect(claude.getByText("Working")).toBeInTheDocument();
	expect(claude.getByText(/Saved here/)).toBeInTheDocument();
	expect(claude.getByLabelText("Claude API key")).toHaveValue("");
	expect(claude.getByLabelText("Claude API key")).toHaveAttribute("type", "password");
	expect(claude.getByLabelText("Claude API key")).toHaveAttribute("autocomplete", "off");
	expect(claude.getByLabelText("Claude model")).toHaveValue("claude-default");

	const chatgpt = within(screen.getByRole("region", { name: "ChatGPT" }));
	expect(chatgpt.getByText(/from the server file/i)).toBeInTheDocument();
	expect(chatgpt.getByText("Out of credit with the provider.")).toBeInTheDocument();
	expect(chatgpt.getByRole("button", { name: "Test ChatGPT" })).toBeEnabled();
	expect(chatgpt.queryByRole("button", { name: "Remove ChatGPT key" })).toBeNull();

	const grok = within(screen.getByRole("region", { name: "Grok" }));
	expect(grok.getByText("Not set")).toBeInTheDocument();
	expect(grok.getByRole("button", { name: "Test Grok" })).toBeDisabled();
	expect(screen.getByRole("region", { name: "Gemini" })).toBeInTheDocument();
	expect(screen.getByRole("region", { name: "Perplexity" })).toBeInTheDocument();
});

test("saving a key sends only the key, shows the provider's test result and never shows the key again", async () => {
	const state = mockSettings(settings());
	state.providerReply = (id) => providerOut(id, { key_set: true, source: "workspace", status: "error", error: "The provider did not accept that key.", checked_at: "2026-11-15T10:00:00Z" });
	renderApp("/settings");
	const grok = within(await screen.findByRole("region", { name: "Grok" }));
	const field = grok.getByLabelText("Grok API key");
	expect(grok.getByRole("button", { name: "Save Grok" })).toBeDisabled();
	await userEvent.type(field, CANARY);
	await userEvent.click(grok.getByRole("button", { name: "Save Grok" }));

	expect(await grok.findByText("The provider did not accept that key.")).toBeInTheDocument();
	const put = state.calls.find((call) => call.method === "PUT" && call.path === "/settings/providers/grok")!;
	expect(put.body).toEqual({ api_key: CANARY });
	expect(put.search).toBe("");
	expect(grok.getByLabelText("Grok API key")).toHaveValue("");
	expect(grok.getByText(/Saved here/)).toBeInTheDocument();
	expect(document.body.textContent).not.toContain(CANARY);
	expect(JSON.stringify({ ...localStorage })).not.toContain(CANARY);
	expect(JSON.stringify({ ...sessionStorage })).not.toContain(CANARY);
	expect(window.location.href).not.toContain(CANARY);
});

test("a working key shows Working, and changing only the model sends only the model", async () => {
	const state = mockSettings(settings());
	renderApp("/settings");
	const claude = within(await screen.findByRole("region", { name: "Claude" }));
	await userEvent.type(claude.getByLabelText("Claude API key"), CANARY);
	await userEvent.click(claude.getByRole("button", { name: "Save Claude" }));
	expect(await claude.findByText("Working")).toBeInTheDocument();

	const model = claude.getByLabelText("Claude model");
	await userEvent.clear(model);
	await userEvent.type(model, "claude-custom");
	await userEvent.click(claude.getByRole("button", { name: "Save Claude" }));
	await waitFor(() => expect(state.calls.filter((c) => c.method === "PUT")).toHaveLength(2));
	expect(state.calls.filter((c) => c.method === "PUT")[1].body).toEqual({ model: "claude-custom" });
	expect(document.body.textContent).not.toContain(CANARY);
});

test("Test runs the provider's check, and Remove asks first, then removes the saved key", async () => {
	const state = mockSettings(settings({ providers: [providerOut("claude", { key_set: true, source: "workspace", status: "ok" }), ...settings().providers.slice(1)] }));
	renderApp("/settings");
	const claude = within(await screen.findByRole("region", { name: "Claude" }));
	await userEvent.click(claude.getByRole("button", { name: "Test Claude" }));
	expect(await claude.findByText("The provider said the key is not valid.")).toBeInTheDocument();
	expect(state.calls.some((c) => c.method === "POST" && c.path === "/settings/providers/claude/test")).toBe(true);

	await userEvent.click(claude.getByRole("button", { name: "Remove Claude key" }));
	expect(state.calls.some((c) => c.method === "DELETE")).toBe(false);
	await userEvent.click(claude.getByRole("button", { name: "Yes, remove the Claude key" }));
	expect(await claude.findByText("Not set")).toBeInTheDocument();
	expect(state.calls.some((c) => c.method === "DELETE" && c.path === "/settings/providers/claude")).toBe(true);
});

test("a rejected save shows the server's message and keeps nothing secret on screen", async () => {
	mockSettings(settings());
	server.use(http.put("/api/v1/settings/providers/claude", () => fail(422, "Choose a model that exists.")));
	renderApp("/settings");
	const claude = within(await screen.findByRole("region", { name: "Claude" }));
	await userEvent.type(claude.getByLabelText("Claude API key"), CANARY);
	await userEvent.click(claude.getByRole("button", { name: "Save Claude" }));
	expect(await claude.findByText("Choose a model that exists.")).toBeInTheDocument();
	expect(document.body.textContent).not.toContain(CANARY);
});

test("a secret the server should never send is not drawn", async () => {
	const leaky = settings({ providers: [{ ...providerOut("claude", { key_set: true, source: "workspace" }), api_key: CANARY } as never, ...settings().providers.slice(1)] });
	mockSettings(leaky);
	renderApp("/settings");
	await screen.findByRole("region", { name: "Claude" });
	expect(document.body.textContent).not.toContain(CANARY);
	expect(document.body.innerHTML).not.toContain(CANARY);
});

test("without MARGINCITE_SECRET_KEY the key fields are off and the screen says why", async () => {
	mockSettings(settings({ secret_key_ready: false }));
	renderApp("/settings");
	expect(await screen.findByText(/MARGINCITE_SECRET_KEY/)).toBeInTheDocument();
	expect(within(screen.getByRole("region", { name: "Claude" })).getByLabelText("Claude API key")).toBeDisabled();
	expect(within(section("Search data (DataForSEO)")).getByLabelText("DataForSEO password")).toBeDisabled();
});

test("Google sign-in shows the redirect address with a copy button and a numbered guide", async () => {
	mockSettings(settings());
	const writeText = vi.fn().mockResolvedValue(undefined);
	Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
	renderApp("/settings");
	const google = within(await screen.findByRole("region", { name: "Google Search Console sign-in" }));
	const address = `${window.location.origin}/api/v1/oauth/google/callback`;
	expect(google.getByText(address)).toBeInTheDocument();
	await userEvent.click(google.getByRole("button", { name: "Copy the redirect address" }));
	expect(writeText).toHaveBeenCalledWith(address);
	expect(await google.findByText("Copied")).toBeInTheDocument();
	expect(within(google.getByRole("list", { name: "Steps" })).getAllByRole("listitem").length).toBeGreaterThanOrEqual(4);
	expect(google.getByLabelText("Google client secret")).toHaveAttribute("type", "password");
	expect(google.getByLabelText("Google client secret")).toHaveAttribute("autocomplete", "off");
});

test("copying can fail, and the screen says to copy by hand", async () => {
	mockSettings(settings());
	Object.defineProperty(navigator, "clipboard", { value: { writeText: vi.fn().mockRejectedValue(new Error("no")) }, configurable: true });
	renderApp("/settings");
	const google = within(await screen.findByRole("region", { name: "Google Search Console sign-in" }));
	await userEvent.click(google.getByRole("button", { name: "Copy the redirect address" }));
	expect(await google.findByText(/copy it by hand/i)).toBeInTheDocument();
});

test("saving the Google client sends both values, clears them and shows Saved", async () => {
	const state = mockSettings(settings());
	renderApp("/settings");
	const google = within(await screen.findByRole("region", { name: "Google Search Console sign-in" }));
	expect(google.getByRole("button", { name: "Save Google client" })).toBeDisabled();
	await userEvent.type(google.getByLabelText("Google client id"), "client-id-value");
	await userEvent.type(google.getByLabelText("Google client secret"), CANARY);
	await userEvent.click(google.getByRole("button", { name: "Save Google client" }));
	expect(await google.findByText(/Saved here/)).toBeInTheDocument();
	expect(state.calls.find((c) => c.path === "/settings/google")!.body).toEqual({ client_id: "client-id-value", client_secret: CANARY });
	expect(google.getByLabelText("Google client secret")).toHaveValue("");
	expect(google.getByLabelText("Google client id")).toHaveValue("");
	expect(document.body.textContent).not.toContain(CANARY);
	expect(document.body.textContent).not.toContain("client-id-value");

	await userEvent.click(google.getByRole("button", { name: "Remove Google client" }));
	await userEvent.click(google.getByRole("button", { name: "Yes, remove the Google client" }));
	await waitFor(() => expect(state.calls.some((c) => c.method === "DELETE" && c.path === "/settings/google")).toBe(true));
});

test("DataForSEO: save, balance, test failure, and what it unlocks", async () => {
	const state = mockSettings(settings());
	renderApp("/settings");
	const data = within(await screen.findByRole("region", { name: "Search data (DataForSEO)" }));
	for (const word of ["Rank tracking", "AI Overviews", "Search volume", "Keyword ideas"]) expect(data.getByText(new RegExp(word))).toBeInTheDocument();
	expect(data.getByRole("button", { name: "Test DataForSEO" })).toBeDisabled();

	await userEvent.type(data.getByLabelText("DataForSEO login"), "you@example.com");
	await userEvent.type(data.getByLabelText("DataForSEO password"), CANARY);
	expect(data.getByLabelText("DataForSEO password")).toHaveAttribute("type", "password");
	await userEvent.click(data.getByRole("button", { name: "Save DataForSEO" }));
	expect(await data.findByText(/Balance/)).toHaveTextContent("$12.50");
	expect(state.calls.find((c) => c.path === "/settings/dataforseo")!.body).toEqual({ login: "you@example.com", password: CANARY });
	expect(data.getByLabelText("DataForSEO password")).toHaveValue("");
	expect(document.body.textContent).not.toContain(CANARY);

	await userEvent.click(data.getByRole("button", { name: "Test DataForSEO" }));
	expect(await data.findByText("DataForSEO says the account is out of credit.")).toBeInTheDocument();
	await userEvent.click(data.getByRole("button", { name: "Remove DataForSEO login" }));
	await userEvent.click(data.getByRole("button", { name: "Yes, remove the DataForSEO login" }));
	expect(await data.findByText("Not set")).toBeInTheDocument();
});

test("page speed key is asked for, saved write-only and removable", async () => {
	const state = mockSettings(settings());
	renderApp("/settings");
	const speed = within(await screen.findByRole("region", { name: "Page speed" }));
	expect(speed.getByText(/checks without a key tend to fail/i)).toBeInTheDocument();
	await userEvent.type(speed.getByLabelText("PageSpeed API key"), CANARY);
	await userEvent.click(speed.getByRole("button", { name: "Save PageSpeed key" }));
	expect(await speed.findByText(/Saved here/)).toBeInTheDocument();
	expect(state.calls.find((c) => c.path === "/settings/pagespeed")!.body).toEqual({ api_key: CANARY });
	expect(document.body.textContent).not.toContain(CANARY);
	await userEvent.click(speed.getByRole("button", { name: "Remove PageSpeed key" }));
	await userEvent.click(speed.getByRole("button", { name: "Yes, remove the PageSpeed key" }));
	expect(await speed.findByText("Not set")).toBeInTheDocument();
});

test("the crawler contact explains why, saves, and shows the server's refusal", async () => {
	const state = mockSettings(settings());
	renderApp("/settings");
	const crawler = within(await screen.findByRole("region", { name: "Crawler" }));
	expect(crawler.getByText(/site owners/i)).toBeInTheDocument();
	await userEvent.type(crawler.getByLabelText("Contact address"), "nope");
	await userEvent.click(crawler.getByRole("button", { name: "Save contact" }));
	expect(await crawler.findByText("Enter an email address or an http(s) address.")).toBeInTheDocument();
	await userEvent.clear(crawler.getByLabelText("Contact address"));
	await userEvent.type(crawler.getByLabelText("Contact address"), "you@example.com");
	await userEvent.click(crawler.getByRole("button", { name: "Save contact" }));
	expect(await crawler.findByText("Saved")).toBeInTheDocument();
	expect(state.calls.filter((c) => c.path === "/settings/crawler").pop()!.body).toEqual({ contact: "you@example.com" });
});

test("a saved crawler contact is prefilled", async () => {
	mockSettings(settingsReady());
	renderApp("/settings");
	const crawler = within(await screen.findByRole("region", { name: "Crawler" }));
	expect(crawler.getByLabelText("Contact address")).toHaveValue("you@example.com");
});

test("usage adds calls and estimated cost by source, says estimate, and never shows a missing estimate as zero", async () => {
	const state = mockSettings(settings(), [
		{ source: "dataforseo", kind: "serp", calls: 10, estimated_cost_usd: 0.6, day: "2026-11-14" },
		{ source: "dataforseo", kind: "serp", calls: 5, estimated_cost_usd: 0.3, day: "2026-11-13" },
		{ source: "pagespeed", kind: "speed", calls: 4, estimated_cost_usd: null, day: "2026-11-14" },
		{ source: "claude", kind: "answer", calls: 7, estimated_cost_usd: 0.05, day: "2026-11-14" },
	]);
	renderApp("/settings");
	const usage = within(await screen.findByRole("region", { name: "Usage" }));
	const table = await usage.findByRole("table", { name: "Calls and estimated cost by source" });
	const dfs = within(table).getByText("DataForSEO").closest("tr") as HTMLElement;
	expect(dfs).toHaveTextContent("15");
	expect(dfs).toHaveTextContent("$0.90");
	const speed = within(table).getByText("Google PageSpeed").closest("tr") as HTMLElement;
	expect(speed).toHaveTextContent("No estimate");
	expect(speed).not.toHaveTextContent("$0");
	expect(within(table).getByText("Claude")).toBeInTheDocument();
	expect(usage.getByText(/estimates/i)).toBeInTheDocument();
	expect(state.calls.find((c) => c.path === "/usage")!.search).toBe("?days=30");
});

test("usage with no calls, and usage that fails with Retry", async () => {
	mockSettings(settings(), []);
	renderApp("/settings");
	expect(await within(await screen.findByRole("region", { name: "Usage" })).findByText(/no paid calls/i)).toBeInTheDocument();

	let calls = 0;
	server.use(
		http.get("/api/v1/usage", () => {
			calls += 1;
			return calls === 1 ? fail(500, "Usage could not be read.") : ok([{ source: "dataforseo", kind: "serp", calls: 1, estimated_cost_usd: 0.1, day: "2026-11-14" }]);
		}),
	);
	await userEvent.click(screen.getByRole("link", { name: "Margincite, home" }));
	await userEvent.click(await screen.findByRole("link", { name: "Settings" }));
	const usage = within(await screen.findByRole("region", { name: "Usage" }));
	expect(await usage.findByText("Usage could not be read.")).toBeInTheDocument();
	await userEvent.click(usage.getByRole("button", { name: "Retry" }));
	expect(await usage.findByRole("table")).toBeInTheDocument();
});

test("a network failure on save says so in plain words", async () => {
	mockSettings(settings());
	server.use(http.put("/api/v1/settings/pagespeed", () => HttpResponse.error()));
	renderApp("/settings");
	const speed = within(await screen.findByRole("region", { name: "Page speed" }));
	await userEvent.type(speed.getByLabelText("PageSpeed API key"), CANARY);
	await userEvent.click(speed.getByRole("button", { name: "Save PageSpeed key" }));
	expect(await speed.findByText(/can't be reached/i)).toBeInTheDocument();
	expect(document.body.textContent).not.toContain(CANARY);
});

test("Google sign-in shows the redirect address the server gives, not one worked out from this page", async () => {
	const uri = "https://margincite.example.org/api/v1/oauth/google/callback";
	mockSettings(settings({ google: { client_id_set: false, client_secret_set: false, source: "none", redirect_uri: uri } }));
	const writeText = vi.fn().mockResolvedValue(undefined);
	Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
	renderApp("/settings");
	const google = within(await screen.findByRole("region", { name: "Google Search Console sign-in" }));
	expect(google.getByText(uri)).toBeInTheDocument();
	await userEvent.click(google.getByRole("button", { name: "Copy the redirect address" }));
	expect(writeText).toHaveBeenCalledWith(uri);
});

test("a model that differs from the default can be put back with Use default, which sends an empty model", async () => {
	const state = mockSettings(settings({ providers: [providerOut("claude", { model: "claude-custom", default_model: "claude-base" }), ...settings().providers.slice(1)] }));
	state.providerReply = () => providerOut("claude", { model: "claude-base", default_model: "claude-base" });
	renderApp("/settings");
	const claude = within(await screen.findByRole("region", { name: "Claude" }));
	expect(within(await screen.findByRole("region", { name: "ChatGPT" })).queryByRole("button", { name: /use default/i })).toBeNull();
	await userEvent.click(claude.getByRole("button", { name: "Use default model for Claude" }));
	await waitFor(() => expect(claude.getByLabelText("Claude model")).toHaveValue("claude-base"));
	expect(state.calls.filter((c) => c.method === "PUT")[0].body).toEqual({ model: "" });
	expect(claude.queryByRole("button", { name: /use default/i })).toBeNull();
});

test("a saved crawler contact can be removed after a confirming step; one from the server file cannot", async () => {
	const state = mockSettings(settingsReady());
	renderApp("/settings");
	const crawler = within(await screen.findByRole("region", { name: "Crawler" }));
	await userEvent.click(crawler.getByRole("button", { name: "Remove crawler contact" }));
	expect(state.calls.some((c) => c.method === "DELETE")).toBe(false);
	await userEvent.click(crawler.getByRole("button", { name: "Yes, remove the crawler contact" }));
	await waitFor(() => expect(crawler.getByLabelText("Contact address")).toHaveValue(""));
	expect(state.calls.some((c) => c.method === "DELETE" && c.path === "/settings/crawler")).toBe(true);
	expect(crawler.queryByRole("button", { name: "Remove crawler contact" })).toBeNull();
});

test("a crawler contact from the server file has no Remove control", async () => {
	mockSettings(settings({ crawler: { contact: "ops@example.com", contact_source: "environment" } }));
	renderApp("/settings");
	const crawler = within(await screen.findByRole("region", { name: "Crawler" }));
	await crawler.findByLabelText("Contact address");
	expect(crawler.queryByRole("button", { name: /remove/i })).toBeNull();
});

test("usage names what each call was for in plain words and keeps an unknown kind as it is", async () => {
	mockSettings(settings(), [
		{ source: "pagespeed", kind: "page_test", calls: 1, estimated_cost_usd: null, day: "2026-11-14" },
		{ source: "dataforseo", kind: "serp", calls: 1, estimated_cost_usd: 0.1, day: "2026-11-14" },
		{ source: "dataforseo", kind: "keyword_volume", calls: 1, estimated_cost_usd: 0.1, day: "2026-11-14" },
		{ source: "dataforseo", kind: "keyword_ideas", calls: 1, estimated_cost_usd: 0.1, day: "2026-11-14" },
		{ source: "dataforseo", kind: "brand_new_kind", calls: 1, estimated_cost_usd: 0.1, day: "2026-11-14" },
	]);
	renderApp("/settings");
	const table = await within(await screen.findByRole("region", { name: "Usage" })).findByRole("table");
	for (const label of ["Page speed test", "Search result check", "Search volume lookup", "Keyword ideas search", "brand_new_kind"]) {
		expect(within(table).getByText(label)).toBeInTheDocument();
	}
	expect(within(table).queryByText("page_test")).toBeNull();
});
