import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { goTo } from "../../src/connections/navigate";
import type { BingStatus, GoogleStatus } from "../../src/api/types";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { integrations, site, summary } from "../mocks/fixtures";
import { renderApp } from "../render";

vi.mock("../../src/connections/navigate", () => ({ goTo: vi.fn() }));

const NOT_GOOGLE: GoogleStatus = { status: "not_connected", property: null, synced_at: null, error: null };
const NOT_BING: BingStatus = { status: "not_connected", site_url: null, synced_at: null, error: null };

function mockConnections(opts: { google?: GoogleStatus; bing?: BingStatus; server?: ReturnType<typeof integrations> } = {}) {
	const state = { google: opts.google ?? NOT_GOOGLE, bing: opts.bing ?? NOT_BING, calls: [] as string[], bodies: {} as Record<string, unknown> };
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/integrations", () => ok(opts.server ?? integrations())),
		http.get("/api/v1/sites/s1/integrations/google", () => ok(state.google)),
		http.get("/api/v1/sites/s1/integrations/bing", () => ok(state.bing)),
		http.post("/api/v1/sites/s1/integrations/google/start", () => {
			state.calls.push("google start");
			return ok({ auth_url: "https://accounts.google.com/o/oauth2/v2/auth?state=abc" });
		}),
		http.get("/api/v1/sites/s1/integrations/google/properties", () =>
			ok([
				{ property: "sc-domain:example.com", permission: "siteOwner" },
				{ property: "https://example.com/", permission: "siteFullUser" },
			]),
		),
		http.put("/api/v1/sites/s1/integrations/google", async ({ request }) => {
			state.bodies.google = await request.json();
			state.google = { status: "connected", property: "sc-domain:example.com", synced_at: null, error: null };
			return ok(state.google);
		}),
		http.post("/api/v1/sites/s1/integrations/google/sync", () => {
			state.calls.push("google sync");
			return ok(state.google, null, 202);
		}),
		http.delete("/api/v1/sites/s1/integrations/google", () => {
			state.calls.push("google delete");
			state.google = NOT_GOOGLE;
			return ok(state.google);
		}),
		http.put("/api/v1/sites/s1/integrations/bing", async ({ request }) => {
			state.calls.push(`bing put ${request.url}`);
			state.bodies.bing = await request.json();
			state.bing = { status: "connected", site_url: "https://example.com/", synced_at: null, error: null };
			return ok(state.bing);
		}),
		http.post("/api/v1/sites/s1/integrations/bing/sync", () => {
			state.calls.push("bing sync");
			return ok(state.bing, null, 202);
		}),
		http.delete("/api/v1/sites/s1/integrations/bing", () => {
			state.calls.push("bing delete");
			state.bing = NOT_BING;
			return ok(state.bing);
		}),
	);
	return state;
}

const google = () => screen.getByRole("region", { name: "Google Search Console" });
const bing = () => screen.getByRole("region", { name: "Bing Webmaster Tools" });

beforeEach(() => vi.mocked(goTo).mockClear());

test("a missing secret key is named exactly and missing Google credentials point to Settings", async () => {
	mockConnections({ server: integrations({ secret_key_set: false, google: { configured: false } }) });
	renderApp("/sites/s1/connections");
	await screen.findByRole("region", { name: "Google Search Console" });
	expect(await within(google()).findByText("MARGINCITE_SECRET_KEY")).toBeInTheDocument();
	expect(within(google()).getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
	expect(within(google()).queryByText("GOOGLE_CLIENT_ID")).toBeNull();
	expect(within(google()).getByRole("button", { name: "Connect Google Search Console" })).toBeDisabled();
	expect(within(bing()).getByText("MARGINCITE_SECRET_KEY")).toBeInTheDocument();
	expect(await within(bing()).findByRole("button", { name: "Save" })).toBeDisabled();
});

test("connecting asks the server for the sign-in address and sends the browser there", async () => {
	const state = mockConnections();
	renderApp("/sites/s1/connections");
	await userEvent.click(await screen.findByRole("button", { name: "Connect Google Search Console" }));
	await vi.waitFor(() => expect(goTo).toHaveBeenCalledWith("https://accounts.google.com/o/oauth2/v2/auth?state=abc"));
	expect(state.calls).toContain("google start");
});

test("a refusal to start the sign-in is shown", async () => {
	mockConnections();
	server.use(http.post("/api/v1/sites/s1/integrations/google/start", () => fail(422, "Google sign-in is not set up on this server.")));
	renderApp("/sites/s1/connections");
	await userEvent.click(await screen.findByRole("button", { name: "Connect Google Search Console" }));
	expect(await screen.findByText("Google sign-in is not set up on this server.")).toBeInTheDocument();
	expect(goTo).not.toHaveBeenCalled();
});

test("a sign-in address that is not a web address is never followed", async () => {
	mockConnections();
	server.use(http.post("/api/v1/sites/s1/integrations/google/start", () => ok({ auth_url: "javascript:alert(1)" })));
	renderApp("/sites/s1/connections");
	await userEvent.click(await screen.findByRole("button", { name: "Connect Google Search Console" }));
	expect(await screen.findByText(/unexpected sign-in address/i)).toBeInTheDocument();
	expect(goTo).not.toHaveBeenCalled();
});

test("coming back connected says so and shows the property chooser", async () => {
	const state = mockConnections({ google: { status: "choose_property", property: null, synced_at: null, error: null } });
	renderApp("/sites/s1/connections?google=connected");
	expect(await screen.findByText(/google is connected/i)).toBeInTheDocument();
	const chooser = await screen.findByRole("group", { name: "Choose the Search Console property for this site" });
	await userEvent.click(within(chooser).getByRole("radio", { name: /sc-domain:example\.com/ }));
	await userEvent.click(screen.getByRole("button", { name: "Use this property" }));
	expect(await within(google()).findByText("sc-domain:example.com")).toBeInTheDocument();
	expect(state.bodies.google).toEqual({ property: "sc-domain:example.com" });
	expect(within(google()).getByRole("button", { name: "Sync now" })).toBeInTheDocument();
});

test("the choose button waits for a choice and a refused property shows the reason", async () => {
	mockConnections({ google: { status: "choose_property", property: null, synced_at: null, error: null } });
	server.use(http.put("/api/v1/sites/s1/integrations/google", () => fail(422, "That property is not one of your Search Console properties.")));
	renderApp("/sites/s1/connections");
	const button = await screen.findByRole("button", { name: "Use this property" });
	expect(button).toBeDisabled();
	await userEvent.click(screen.getByRole("radio", { name: /https:\/\/example\.com\// }));
	await userEvent.click(button);
	expect(await screen.findByText("That property is not one of your Search Console properties.")).toBeInTheDocument();
});

test("no Search Console properties explains what to do", async () => {
	mockConnections({ google: { status: "choose_property", property: null, synced_at: null, error: null } });
	server.use(http.get("/api/v1/sites/s1/integrations/google/properties", () => ok([])));
	renderApp("/sites/s1/connections");
	expect(await screen.findByText(/lists no properties you can read/i)).toBeInTheDocument();
});

test("the property list fails with a retry", async () => {
	mockConnections({ google: { status: "choose_property", property: null, synced_at: null, error: null } });
	server.use(http.get("/api/v1/sites/s1/integrations/google/properties", () => fail(502, "Google could not be reached.")));
	renderApp("/sites/s1/connections");
	expect(await screen.findByText("Google could not be reached.")).toBeInTheDocument();
	server.use(http.get("/api/v1/sites/s1/integrations/google/properties", () => ok([{ property: "sc-domain:example.com", permission: "siteOwner" }])));
	await userEvent.click(within(google()).getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("radio", { name: /sc-domain:example\.com/ })).toBeInTheDocument();
});

test.each([
	["denied", /cancelled/i],
	["expired", /10 minutes/i],
	["invalid_state", /not valid or was already used/i],
	["no_refresh_token", /ongoing access/i],
	["something_new", /did not finish/i],
])("coming back with the error %s explains it", async (reason, words) => {
	mockConnections();
	renderApp(`/sites/s1/connections?google=error&reason=${reason}`);
	expect(await screen.findByRole("alert")).toHaveTextContent(words);
});

test("a connected property can be synced and disconnected", async () => {
	const state = mockConnections({ google: { status: "connected", property: "sc-domain:example.com", synced_at: "2026-11-12T08:00:00Z", error: null } });
	renderApp("/sites/s1/connections");
	expect(await within(await screen.findByRole("region", { name: "Google Search Console" })).findByText("sc-domain:example.com")).toBeInTheDocument();
	expect(within(google()).getByText(/last synced 12 Nov 2026/i)).toBeInTheDocument();
	await userEvent.click(within(google()).getByRole("button", { name: "Sync now" }));
	expect(await within(google()).findByText(/sync has started/i)).toBeInTheDocument();
	expect(state.calls).toContain("google sync");

	await userEvent.click(within(google()).getByRole("button", { name: "Disconnect" }));
	expect(within(google()).getByText(/deletes the synced numbers/i)).toBeInTheDocument();
	await userEvent.click(within(google()).getByRole("button", { name: "Yes, disconnect" }));
	expect(await within(google()).findByRole("button", { name: "Connect Google Search Console" })).toBeInTheDocument();
	expect(state.calls).toContain("google delete");
});

test("disconnecting can be cancelled", async () => {
	mockConnections({ google: { status: "connected", property: "sc-domain:example.com", synced_at: null, error: null } });
	renderApp("/sites/s1/connections");
	await userEvent.click(await within(await screen.findByRole("region", { name: "Google Search Console" })).findByRole("button", { name: "Disconnect" }));
	await userEvent.click(within(google()).getByRole("button", { name: "Keep it" }));
	expect(within(google()).getByRole("button", { name: "Disconnect" })).toBeInTheDocument();
});

test("a connected property can be changed", async () => {
	mockConnections({ google: { status: "connected", property: "sc-domain:example.com", synced_at: null, error: null } });
	renderApp("/sites/s1/connections");
	await userEvent.click(await screen.findByRole("button", { name: "Change property" }));
	expect(await screen.findByRole("group", { name: "Choose the Search Console property for this site" })).toBeInTheDocument();
});

test("a connection Google no longer accepts asks to connect again", async () => {
	mockConnections({ google: { status: "needs_reconnecting", property: "sc-domain:example.com", synced_at: null, error: "Google no longer accepts this connection. Connect Google Search Console again." } });
	renderApp("/sites/s1/connections");
	expect(await screen.findByText("Google no longer accepts this connection. Connect Google Search Console again.")).toBeInTheDocument();
	expect(within(google()).getByRole("button", { name: "Connect Google Search Console" })).toBeEnabled();
});

test("a failed sync or disconnect shows the server's message", async () => {
	mockConnections({ google: { status: "connected", property: "sc-domain:example.com", synced_at: null, error: null } });
	server.use(http.post("/api/v1/sites/s1/integrations/google/sync", () => fail(422, "Choose a property first.")));
	renderApp("/sites/s1/connections");
	await userEvent.click(await within(await screen.findByRole("region", { name: "Google Search Console" })).findByRole("button", { name: "Sync now" }));
	expect(await screen.findByText("Choose a property first.")).toBeInTheDocument();
});

test("Bing: the form explains where the key lives and keeps it out of the address", async () => {
	const state = mockConnections();
	renderApp("/sites/s1/connections");
	const region = await screen.findByRole("region", { name: "Bing Webmaster Tools" });
	const key = await within(region).findByLabelText("API key");
	expect(within(region).getByText(/settings, then api access/i)).toBeInTheDocument();
	expect(key).toHaveAttribute("type", "password");
	expect(within(region).getByLabelText("Site address")).toHaveValue("https://example.com/");
	await userEvent.type(key, "secret-key-123");
	await userEvent.click(within(region).getByRole("button", { name: "Save" }));
	expect(await within(bing()).findByText(/connected to https:\/\/example\.com\//i)).toBeInTheDocument();
	expect(state.bodies.bing).toEqual({ api_key: "secret-key-123", site_url: "https://example.com/" });
	expect(state.calls.join(" ")).not.toContain("secret-key-123");
	expect(within(bing()).getByLabelText("API key")).toHaveValue("");
	const stored = [window.sessionStorage, window.localStorage].map((area) => {
		try {
			return JSON.stringify(Object.entries(area ?? {}));
		} catch {
			return "";
		}
	});
	expect(stored.join(" ")).not.toContain("secret-key-123");
});

test("Bing: the server's 422 reason is shown and the typed values stay", async () => {
	mockConnections();
	server.use(http.put("/api/v1/sites/s1/integrations/bing", () => fail(422, "Bing did not accept that API key. Copy it again from Bing Webmaster Tools.")));
	renderApp("/sites/s1/connections");
	const region = await screen.findByRole("region", { name: "Bing Webmaster Tools" });
	await userEvent.type(await within(region).findByLabelText("API key"), "wrong");
	await userEvent.click(within(region).getByRole("button", { name: "Save" }));
	expect(await within(region).findByText("Bing did not accept that API key. Copy it again from Bing Webmaster Tools.")).toBeInTheDocument();
	expect(within(region).getByLabelText("API key")).toHaveValue("wrong");
});

test("Bing: empty fields are refused without asking the server", async () => {
	const state = mockConnections();
	renderApp("/sites/s1/connections");
	const region = await screen.findByRole("region", { name: "Bing Webmaster Tools" });
	await userEvent.click(await within(region).findByRole("button", { name: "Save" }));
	expect(await within(region).findByText("Paste the API key and the site address first.")).toBeInTheDocument();
	expect(state.calls).toEqual([]);
});

test("Bing: a connected site can be synced and disconnected", async () => {
	const state = mockConnections({ bing: { status: "connected", site_url: "https://example.com/", synced_at: "2026-11-10T08:00:00Z", error: null } });
	renderApp("/sites/s1/connections");
	const region = await screen.findByRole("region", { name: "Bing Webmaster Tools" });
	expect(await within(region).findByText(/last synced 10 Nov 2026/i)).toBeInTheDocument();
	await userEvent.click(within(region).getByRole("button", { name: "Sync now" }));
	expect(await within(region).findByText(/sync has started/i)).toBeInTheDocument();
	await userEvent.click(within(region).getByRole("button", { name: "Disconnect" }));
	await userEvent.click(within(region).getByRole("button", { name: "Yes, disconnect" }));
	await vi.waitFor(() => expect(state.calls).toContain("bing delete"));
	expect(await within(bing()).findByText("Not connected")).toBeInTheDocument();
});

test("Bing: a key Bing no longer accepts shows its message", async () => {
	mockConnections({ bing: { status: "needs_reconnecting", site_url: "https://example.com/", synced_at: null, error: "Bing no longer accepts this API key." } });
	renderApp("/sites/s1/connections");
	expect(await screen.findByText("Bing no longer accepts this API key.")).toBeInTheDocument();
});

test("AI engines are listed as set up or not, with a link to Settings", async () => {
	mockConnections();
	renderApp("/sites/s1/connections");
	const list = await screen.findByRole("list", { name: "AI engines" });
	const claude = within(list).getByText("Claude").closest("li") as HTMLElement;
	expect(claude).toHaveTextContent("Set up");
	expect(claude).toHaveTextContent("claude-sonnet-5-5");
	const chatgpt = within(list).getByText("ChatGPT").closest("li") as HTMLElement;
	expect(chatgpt).toHaveTextContent("Not set up");
	expect(chatgpt).toHaveTextContent("Add a key in Settings");
	expect(within(list).getByText("Grok").closest("li")).toHaveTextContent("Not set up");
	expect(within(list).getByText("Gemini").closest("li")).toHaveTextContent("Set up");
	expect(within(list.closest("section") as HTMLElement).getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
});

test("each panel has its own error and retry", async () => {
	mockConnections();
	server.use(http.get("/api/v1/sites/s1/integrations/google", () => HttpResponse.error()));
	renderApp("/sites/s1/connections");
	const region = await screen.findByRole("region", { name: "Google Search Console" });
	expect(await within(region).findByText(/can't be reached/i)).toBeInTheDocument();
	server.use(http.get("/api/v1/sites/s1/integrations/google", () => ok(NOT_GOOGLE)));
	await userEvent.click(within(region).getByRole("button", { name: "Retry" }));
	expect(await within(region).findByRole("button", { name: "Connect Google Search Console" })).toBeInTheDocument();
});

test("the server settings fail to load with a retry", async () => {
	mockConnections();
	server.use(http.get("/api/v1/integrations", () => HttpResponse.error()));
	renderApp("/sites/s1/connections");
	expect(await screen.findByText("Loading the connections…")).toBeInTheDocument();
	expect(await screen.findByText(/can't be reached/i)).toBeInTheDocument();
	server.use(http.get("/api/v1/integrations", () => ok(integrations())));
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("region", { name: "Google Search Console" })).toBeInTheDocument();
});

test("a Google error that lands on the sites list is explained there", async () => {
	server.use(http.get("/api/v1/sites", () => ok([])));
	renderApp("/?google=error&reason=invalid_state");
	expect(await screen.findByRole("alert")).toHaveTextContent(/not valid or was already used/i);
});
