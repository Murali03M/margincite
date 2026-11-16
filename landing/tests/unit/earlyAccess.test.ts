import { vi } from "vitest";
import { isValidEmail, submitEarlyAccess } from "../../src/lib/earlyAccess";

const endpoint = "https://forms.example.test/f/abc";
const okFetch = () => vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));

beforeEach(() => {
	vi.spyOn(console, "error").mockImplementation(() => {});
});

it.each(["a@b.co", "  First.Last@Example.com  "])("accepts %s", (email) => {
	expect(isValidEmail(email)).toBe(true);
});

it.each(["", "a@b", "a b@c.com", "@c.com", "a@", `${"x".repeat(250)}@c.com`])("rejects %s", (email) => {
	expect(isValidEmail(email)).toBe(false);
});

it("posts the trimmed email as JSON", async () => {
	const fetchFn = okFetch();
	const result = await submitEarlyAccess({ email: "  a@b.co ", honeypot: "" }, { endpoint, fetchFn });
	expect(result).toEqual({ ok: true });
	const [url, init] = fetchFn.mock.calls[0];
	expect(url).toBe(endpoint);
	expect(init.method).toBe("POST");
	expect(JSON.parse(init.body)).toEqual({ email: "a@b.co" });
	expect(init.headers).toMatchObject({ Accept: "application/json", "Content-Type": "application/json" });
});

it("reports an invalid email without calling the service", async () => {
	const fetchFn = okFetch();
	expect(await submitEarlyAccess({ email: "nope", honeypot: "" }, { endpoint, fetchFn })).toEqual({ ok: false, reason: "invalid_email" });
	expect(fetchFn).not.toHaveBeenCalled();
});

it("reports not_configured when there is no endpoint", async () => {
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint: undefined, fetchFn: okFetch() })).toEqual({ ok: false, reason: "not_configured" });
});

it("reports not_configured for an endpoint that is not https", async () => {
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint: "http://x.test", fetchFn: okFetch() })).toEqual({ ok: false, reason: "not_configured" });
});

it("pretends to succeed and sends nothing when the honeypot is filled", async () => {
	const fetchFn = okFetch();
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "Acme" }, { endpoint, fetchFn })).toEqual({ ok: true });
	expect(fetchFn).not.toHaveBeenCalled();
});

it("reports rejected on a non-2xx answer", async () => {
	const fetchFn = vi.fn().mockResolvedValue(new Response("{}", { status: 422 }));
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint, fetchFn })).toEqual({ ok: false, reason: "rejected" });
});

it("reports network when the request throws", async () => {
	const fetchFn = vi.fn().mockRejectedValue(new TypeError("failed to fetch"));
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint, fetchFn })).toEqual({ ok: false, reason: "network" });
});

it("gives up after ten seconds and reports network", async () => {
	vi.useFakeTimers();
	const fetchFn = vi.fn((_url: RequestInfo | URL, init?: RequestInit) =>
		new Promise<Response>((_resolve, reject) => {
			init?.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
		}),
	);
	const pending = submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint, fetchFn });
	await vi.advanceTimersByTimeAsync(10_000);
	expect(await pending).toEqual({ ok: false, reason: "network" });
	vi.useRealTimers();
});
