import { COUNTED_KEY, digitsOf, formatCount, loadVisits, visitsUrl } from "../../src/lib/visits";

function memoryStorage(initial: Record<string, string> = {}) {
	const data = new Map(Object.entries(initial));
	return {
		getItem: (key: string) => data.get(key) ?? null,
		setItem: (key: string, value: string) => void data.set(key, value),
		data,
	};
}

const answer = (body: unknown, ok = true) =>
	vi.fn(async () => ({ ok, json: async () => body }) as Response);

it("counts a visit the first time, and only reads the count after that", () => {
	expect(visitsUrl(false)).toBe("https://abacus.jasoncameron.dev/hit/margincite-site/visits");
	expect(visitsUrl(true)).toBe("https://abacus.jasoncameron.dev/get/margincite-site/visits");
});

it("pads the count to six fixed places, like a numbering stamp", () => {
	expect(digitsOf(1234)).toEqual(["0", "0", "1", "2", "3", "4"]);
	expect(digitsOf(0)).toEqual(["0", "0", "0", "0", "0", "0"]);
});

it("grows past six places instead of dropping digits", () => {
	expect(digitsOf(12345678)).toEqual("12345678".split(""));
});

it("writes the count for screen readers with separators", () => {
	expect(formatCount(1234567)).toBe("1,234,567");
});

it("counts the first visit in a tab and remembers it", async () => {
	const storage = memoryStorage();
	const fetcher = answer({ value: 42 });
	await expect(loadVisits({ fetcher, storage })).resolves.toBe(42);
	expect(fetcher).toHaveBeenCalledWith(visitsUrl(false), expect.anything());
	expect(storage.data.get(COUNTED_KEY)).toBe("1");
});

it("only reads the count on a reload in the same tab", async () => {
	const fetcher = answer({ value: 43 });
	await expect(loadVisits({ fetcher, storage: memoryStorage({ [COUNTED_KEY]: "1" }) })).resolves.toBe(43);
	expect(fetcher).toHaveBeenCalledWith(visitsUrl(true), expect.anything());
});

it("still counts when the browser blocks storage", async () => {
	const storage = {
		getItem: () => {
			throw new Error("blocked");
		},
		setItem: () => {
			throw new Error("blocked");
		},
	};
	const fetcher = answer({ value: 7 });
	await expect(loadVisits({ fetcher, storage })).resolves.toBe(7);
	expect(fetcher).toHaveBeenCalledWith(visitsUrl(false), expect.anything());
});

it.each([
	["a failed answer", answer({ value: 5 }, false)],
	["an answer without a number", answer({ error: "Key not found" })],
	["a negative number", answer({ value: -1 })],
	[
		"a network failure",
		vi.fn(async () => {
			throw new Error("offline");
		}),
	],
])("shows nothing after %s", async (_name, fetcher) => {
	await expect(loadVisits({ fetcher: fetcher as unknown as typeof fetch, storage: memoryStorage() })).resolves.toBeNull();
});

it("does not mark the visit as counted when counting failed", async () => {
	const storage = memoryStorage();
	await loadVisits({ fetcher: answer({}, false), storage });
	expect(storage.data.has(COUNTED_KEY)).toBe(false);
});
