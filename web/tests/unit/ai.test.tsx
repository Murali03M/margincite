import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import type { AiAnswer, AiPrompt, AiRun, AiSettings } from "../../src/api/types";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";
import { aiAnswer, aiAnswerDetail, aiRun, integrations, site, summary } from "../mocks/fixtures";
import { renderApp } from "../render";

type State = {
	settings: AiSettings;
	prompts: AiPrompt[];
	runs: AiRun[];
	answers: AiAnswer[];
	bodies: Record<string, unknown>;
	calls: string[];
	runPolls: number;
};

const PROMPTS: AiPrompt[] = [
	{ id: "q1", text: "What is the best map maker?", created_at: "2026-11-10T10:00:00Z" },
	{ id: "q2", text: "Where can I buy road atlases?", created_at: "2026-11-10T10:01:00Z" },
];

const ANSWERS: AiAnswer[] = [
	aiAnswer({ id: "a1", prompt_id: "q1", prompt: PROMPTS[0].text, engine: "claude", mentioned: true, cited: true, citation_rank: 1 }),
	aiAnswer({ id: "a2", prompt_id: "q1", prompt: PROMPTS[0].text, engine: "gemini", mentioned: true, cited: false }),
	aiAnswer({ id: "a3", prompt_id: "q2", prompt: PROMPTS[1].text, engine: "claude", mentioned: false, cited: false }),
	aiAnswer({ id: "a4", prompt_id: "q2", prompt: PROMPTS[1].text, engine: "gemini", mentioned: false, cited: false, error: "The engine rejected the API key (HTTP 401)." }),
];

function mockAi(over: Partial<State> = {}) {
	const state: State = {
		settings: { brand_name: "Example", aliases: ["Example Maps"], competitors: ["rival.com"] },
		prompts: PROMPTS,
		runs: [aiRun()],
		answers: ANSWERS,
		bodies: {},
		calls: [],
		runPolls: 0,
		...over,
	};
	server.use(
		http.get("/api/v1/sites/s1", () => ok(site())),
		http.get("/api/v1/sites/s1/summary", () => ok(summary())),
		http.get("/api/v1/integrations", () => ok(integrations())),
		http.get("/api/v1/sites/s1/ai/settings", () => ok(state.settings)),
		http.put("/api/v1/sites/s1/ai/settings", async ({ request }) => {
			state.bodies.settings = await request.json();
			state.settings = state.bodies.settings as AiSettings;
			return ok(state.settings);
		}),
		http.get("/api/v1/sites/s1/ai/prompts", () => ok(state.prompts)),
		http.post("/api/v1/sites/s1/ai/prompts", async ({ request }) => {
			state.bodies.prompt = await request.json();
			const added = { id: "q9", text: (state.bodies.prompt as { text: string }).text, created_at: "2026-11-14T10:00:00Z" };
			state.prompts = [...state.prompts, added];
			return ok(added, null, 201);
		}),
		http.delete("/api/v1/ai/prompts/:id", ({ params }) => {
			state.calls.push(`delete ${String(params.id)}`);
			state.prompts = state.prompts.filter((p) => p.id !== params.id);
			return ok({ deleted: true });
		}),
		http.post("/api/v1/sites/s1/ai/prompts/suggest", () =>
			ok({ suggestions: ["Which company makes the best printed city maps?", "How do I pick a hiking map for Scotland?"] }),
		),
		http.get("/api/v1/sites/s1/ai/runs", () => ok(state.runs, { total: state.runs.length, page: 1, limit: 10 })),
		http.post("/api/v1/sites/s1/ai/runs", () => {
			state.calls.push("run");
			const started = aiRun({ id: "r2", status: "queued", answers_done: 0, finished_at: null, started_at: null });
			state.runs = [started, ...state.runs];
			return ok(started, null, 202);
		}),
		http.get("/api/v1/ai/runs/:id/answers", () => ok(state.answers)),
		http.get("/api/v1/sites/s1/ai/domains", () =>
			ok([
				{ domain: "example.com", citations: 5, answers: 3, is_you: true, is_competitor: false },
				{ domain: "rival.com", citations: 4, answers: 2, is_you: false, is_competitor: true },
				{ domain: "wikipedia.org", citations: 2, answers: 2, is_you: false, is_competitor: false },
			]),
		),
	);
	return state;
}

async function advance(ms: number) {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(ms);
	});
}

test("the grid shows each question against each engine and links to the answer", async () => {
	mockAi();
	renderApp("/sites/s1/ai");
	const grid = await screen.findByRole("table", { name: "Answers by question and engine" });
	const headers = within(grid).getAllByRole("columnheader").map((h) => h.textContent);
	expect(headers).toEqual(["Question", "Claude", "Gemini"]);
	const first = within(grid).getByText("What is the best map maker?").closest("tr") as HTMLElement;
	const cited = within(first).getByRole("link", { name: /Claude answer.*Cited/ });
	expect(cited).toHaveAttribute("href", "/sites/s1/ai/answers/a1");
	expect(within(first).getByRole("link", { name: /Gemini answer.*Mentioned/ })).toHaveAttribute("href", "/sites/s1/ai/answers/a2");
	const second = within(grid).getByText("Where can I buy road atlases?").closest("tr") as HTMLElement;
	expect(within(second).getByRole("link", { name: /Claude answer.*Not mentioned/ })).toBeInTheDocument();
	expect(within(second).getByRole("link", { name: /Gemini answer.*Error/ })).toBeInTheDocument();
});

test("top cited domains mark you and your competitors", async () => {
	mockAi();
	renderApp("/sites/s1/ai");
	const table = await screen.findByRole("table", { name: "Most cited domains" });
	expect(within(table).getByText("example.com").closest("tr")).toHaveTextContent("You");
	expect(within(table).getByText("rival.com").closest("tr")).toHaveTextContent("Competitor");
	expect(within(table).getByText("wikipedia.org").closest("tr")).not.toHaveTextContent(/You|Competitor/);
});

test("engines without a key are listed as not set up with a link to Settings", async () => {
	mockAi();
	renderApp("/sites/s1/ai");
	const list = await screen.findByRole("list", { name: "Engines not set up" });
	expect(within(list).getByText("ChatGPT").closest("li")).toHaveTextContent("add a key in Settings");
	expect(within(list).getByText("Grok").closest("li")).toHaveTextContent("add a key in Settings");
	expect(within(list).getAllByRole("link", { name: "Settings" })[0]).toHaveAttribute("href", "/settings");
	expect(within(list).queryByText("Claude")).toBeNull();
});

test("with no completed run the results explain what to do", async () => {
	mockAi({ runs: [] });
	renderApp("/sites/s1/ai");
	expect(await screen.findByText("No results yet")).toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Run now" })).toBeEnabled();
});

test("a failed latest run says so", async () => {
	mockAi({ runs: [aiRun({ id: "r3", status: "failed", error: "Every engine failed." })] });
	renderApp("/sites/s1/ai");
	expect(await screen.findByText(/the last run failed/i)).toBeInTheDocument();
	expect(screen.getByText("Every engine failed.")).toBeInTheDocument();
});

test("Run now starts a run and shows its progress", async () => {
	const state = mockAi();
	server.use(http.get("/api/v1/ai/runs/r2", () => ok(aiRun({ id: "r2", status: "running", answers_done: 1, answers_total: 4, finished_at: null }))));
	renderApp("/sites/s1/ai");
	await userEvent.click(await screen.findByRole("button", { name: "Run now" }));
	expect(await screen.findByText(/1 of 4 answers/)).toBeInTheDocument();
	expect(screen.getByRole("progressbar", { name: "Run progress" })).toHaveAttribute("value", "1");
	expect(state.calls).toContain("run");
	expect(screen.getByRole("button", { name: /Running|Starting/ })).toBeDisabled();
});

test.each([
	[409, "A run is already under way for this site."],
	[422, "Add at least one question before you run."],
	[429, "You have used all 3 manual runs for today."],
])("a refused run (%s) shows the server's message", async (status, message) => {
	mockAi();
	server.use(http.post("/api/v1/sites/s1/ai/runs", () => fail(status, message)));
	renderApp("/sites/s1/ai");
	await userEvent.click(await screen.findByRole("button", { name: "Run now" }));
	expect(await screen.findByText(message)).toBeInTheDocument();
});

test("progress is polled every three seconds and stops when the run ends", async () => {
	vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval"] });
	const state = mockAi({ runs: [aiRun({ id: "r2", status: "queued", answers_done: 0, finished_at: null })] });
	const sequence = [
		aiRun({ id: "r2", status: "queued", answers_done: 0, finished_at: null }),
		aiRun({ id: "r2", status: "running", answers_done: 2, finished_at: null }),
		aiRun({ id: "r2", status: "completed", answers_done: 4 }),
	];
	server.use(
		http.get("/api/v1/ai/runs/r2", () => {
			const next = sequence[Math.min(state.runPolls, sequence.length - 1)];
			state.runPolls += 1;
			if (next.status === "completed") state.runs = [next];
			return ok(next);
		}),
	);
	renderApp("/sites/s1/ai");

	for (let turn = 0; turn < 40; turn += 1) await advance(10);
	expect(state.runPolls).toBe(1);
	expect(screen.getByText(/waiting to start/i)).toBeInTheDocument();
	await advance(3000);
	expect(state.runPolls).toBe(2);
	expect(screen.getByText(/2 of 4 answers/)).toBeInTheDocument();
	await advance(2000);
	expect(state.runPolls).toBe(2);
	await advance(1000);
	expect(state.runPolls).toBe(3);
	await advance(30_000);
	for (let turn = 0; turn < 40; turn += 1) await advance(10);
	expect(state.runPolls).toBe(3);
	expect(screen.getByText(/run finished/i)).toBeInTheDocument();
	expect(screen.getByRole("table", { name: "Answers by question and engine" })).toBeInTheDocument();
});

test("settings are loaded, saved as lists and a refusal is shown", async () => {
	const state = mockAi();
	renderApp("/sites/s1/ai");
	expect(await screen.findByLabelText("Brand name")).toHaveValue("Example");
	expect(screen.getByLabelText("Other names (separated by commas)")).toHaveValue("Example Maps");
	expect(screen.getByLabelText("Competitor domains (separated by commas)")).toHaveValue("rival.com");
	await userEvent.clear(screen.getByLabelText("Other names (separated by commas)"));
	await userEvent.type(screen.getByLabelText("Other names (separated by commas)"), "Ex Maps, , Example Atlas ");
	await userEvent.clear(screen.getByLabelText("Competitor domains (separated by commas)"));
	await userEvent.type(screen.getByLabelText("Competitor domains (separated by commas)"), "rival.com, other.org");
	await userEvent.click(screen.getByRole("button", { name: "Save settings" }));
	expect(await screen.findByText("Saved.")).toBeInTheDocument();
	expect(state.bodies.settings).toEqual({ brand_name: "Example", aliases: ["Ex Maps", "Example Atlas"], competitors: ["rival.com", "other.org"] });

	server.use(http.put("/api/v1/sites/s1/ai/settings", () => fail(422, "That is not a domain.")));
	await userEvent.click(screen.getByRole("button", { name: "Save settings" }));
	expect(await screen.findByText("That is not a domain.")).toBeInTheDocument();
});

test("settings show an error with retry", async () => {
	mockAi();
	server.use(http.get("/api/v1/sites/s1/ai/settings", () => HttpResponse.error()));
	renderApp("/sites/s1/ai");
	const region = await screen.findByRole("region", { name: "Brand and competitors" });
	expect(await within(region).findByText(/can't be reached/i)).toBeInTheDocument();
	server.use(http.get("/api/v1/sites/s1/ai/settings", () => ok({ brand_name: "Back", aliases: [], competitors: [] })));
	await userEvent.click(within(region).getByRole("button", { name: "Retry" }));
	expect(await within(region).findByLabelText("Brand name")).toHaveValue("Back");
});

test("questions are listed, added and removed", async () => {
	const state = mockAi();
	renderApp("/sites/s1/ai");
	const list = await screen.findByRole("list", { name: "Questions" });
	expect(within(list).getByText("What is the best map maker?")).toBeInTheDocument();
	expect(screen.getByText("2 of 25 questions")).toBeInTheDocument();

	await userEvent.type(screen.getByLabelText("New question"), "Which atlas is best for cyclists?");
	await userEvent.click(screen.getByRole("button", { name: "Add question" }));
	expect(await within(await screen.findByRole("list", { name: "Questions" })).findByText("Which atlas is best for cyclists?")).toBeInTheDocument();
	expect(state.bodies.prompt).toEqual({ text: "Which atlas is best for cyclists?" });
	expect(screen.getByLabelText("New question")).toHaveValue("");

	await userEvent.click(screen.getByRole("button", { name: "Remove question: What is the best map maker?" }));
	await vi.waitFor(() => expect(state.calls).toContain("delete q1"));
	await vi.waitFor(() => expect(screen.queryByText("What is the best map maker?", { selector: "li span" })).toBeNull());
});

test("a refused question shows the server's reason and keeps the text", async () => {
	mockAi();
	server.use(http.post("/api/v1/sites/s1/ai/prompts", () => fail(422, "A question must be 10 to 300 characters.")));
	renderApp("/sites/s1/ai");
	await userEvent.type(await screen.findByLabelText("New question"), "short");
	await userEvent.click(screen.getByRole("button", { name: "Add question" }));
	expect(await screen.findByText("A question must be 10 to 300 characters.")).toBeInTheDocument();
	expect(screen.getByLabelText("New question")).toHaveValue("short");
});

test("an empty question is refused without asking the server", async () => {
	const state = mockAi();
	renderApp("/sites/s1/ai");
	await userEvent.click(await screen.findByRole("button", { name: "Add question" }));
	expect(await screen.findByText("Type the question first.")).toBeInTheDocument();
	expect(state.bodies.prompt).toBeUndefined();
});

test("no questions yet invites the first", async () => {
	mockAi({ prompts: [] });
	renderApp("/sites/s1/ai");
	expect(await screen.findByText(/no questions yet/i)).toBeInTheDocument();
});

test("suggested questions each have an Add button and leave the list once added", async () => {
	const state = mockAi();
	renderApp("/sites/s1/ai");
	await userEvent.click(await screen.findByRole("button", { name: "Suggest questions" }));
	const suggestions = await screen.findByRole("list", { name: "Suggested questions" });
	expect(within(suggestions).getAllByRole("button")).toHaveLength(2);
	await userEvent.click(within(suggestions).getByRole("button", { name: "Add: How do I pick a hiking map for Scotland?" }));
	await vi.waitFor(() => expect(state.bodies.prompt).toEqual({ text: "How do I pick a hiking map for Scotland?" }));
	await vi.waitFor(() => expect(within(screen.getByRole("list", { name: "Suggested questions" })).getAllByRole("button")).toHaveLength(1));
});

test("suggestions that cannot be made show the server's message", async () => {
	mockAi();
	server.use(http.post("/api/v1/sites/s1/ai/prompts/suggest", () => fail(422, "Set ANTHROPIC_API_KEY to get suggestions.")));
	renderApp("/sites/s1/ai");
	await userEvent.click(await screen.findByRole("button", { name: "Suggest questions" }));
	expect(await screen.findByText("Set ANTHROPIC_API_KEY to get suggestions.")).toBeInTheDocument();
});

test("a suggestion that cannot be added shows the reason and stays", async () => {
	mockAi();
	server.use(http.post("/api/v1/sites/s1/ai/prompts", () => fail(409, "A site can have at most 25 questions.")));
	renderApp("/sites/s1/ai");
	await userEvent.click(await screen.findByRole("button", { name: "Suggest questions" }));
	await userEvent.click(await screen.findByRole("button", { name: "Add: How do I pick a hiking map for Scotland?" }));
	expect(await screen.findByText("A site can have at most 25 questions.")).toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Add: How do I pick a hiking map for Scotland?" })).toBeInTheDocument();
});

test("the questions fail to load with a retry", async () => {
	mockAi();
	server.use(http.get("/api/v1/sites/s1/ai/prompts", () => HttpResponse.error()));
	renderApp("/sites/s1/ai");
	const region = await screen.findByRole("region", { name: "Questions to ask" });
	expect(await within(region).findByText(/can't be reached/i)).toBeInTheDocument();
});

test("the results fail to load with a retry", async () => {
	mockAi();
	server.use(http.get("/api/v1/sites/s1/ai/runs", () => HttpResponse.error()));
	renderApp("/sites/s1/ai");
	const region = await screen.findByRole("region", { name: "Latest results" });
	expect(await within(region).findByText(/can't be reached/i)).toBeInTheDocument();
});

test("an answer is shown as plain text with safe citation links", async () => {
	mockAi();
	server.use(
		http.get("/api/v1/ai/answers/a1", () =>
			ok(
				aiAnswerDetail({
					cited: true,
					citation_rank: 2,
					answer_text: "<script>alert(1)</script> Example makes maps.\n[click](javascript:alert(1))",
					citations: [
						{ url: "https://example.com/maps", title: "Example <b>maps</b>", domain: "example.com" },
						{ url: "javascript:alert(1)", title: null, domain: "evil.test" },
					],
					competitor_mentions: ["rival.com"],
					notes: ["A web search failed, so this answer may be less complete."],
				}),
			),
		),
	);
	renderApp("/sites/s1/ai/answers/a1");
	expect(await screen.findByRole("heading", { name: "What is the best map maker?" })).toBeInTheDocument();
	const text = screen.getByText(/Example makes maps\./);
	expect(text.textContent).toContain("<script>alert(1)</script>");
	expect(document.querySelector("script")).toBeNull();
	expect(document.querySelector("b")).toBeNull();
	const link = screen.getByRole("link", { name: "Example <b>maps</b>" });
	expect(link).toHaveAttribute("href", "https://example.com/maps");
	expect(link).toHaveAttribute("rel", "noopener noreferrer nofollow");
	const bad = screen.getByText("javascript:alert(1)");
	expect(bad.closest("a")).toBeNull();
	expect(screen.getByText(/Cited, position 2/)).toBeInTheDocument();
	expect(screen.getByText(/rival\.com/)).toBeInTheDocument();
	expect(screen.getByText("A web search failed, so this answer may be less complete.")).toBeInTheDocument();
	expect(screen.getByRole("link", { name: /back to AI Visibility/i })).toHaveAttribute("href", "/sites/s1/ai");
});

test("an answer that failed shows its error and no text", async () => {
	mockAi();
	server.use(
		http.get("/api/v1/ai/answers/a4", () =>
			ok(aiAnswerDetail({ id: "a4", mentioned: false, cited: false, answer_text: "", citations: [], error: "The engine rejected the API key (HTTP 401)." })),
		),
	);
	renderApp("/sites/s1/ai/answers/a4");
	expect(await screen.findByText("The engine rejected the API key (HTTP 401).")).toBeInTheDocument();
	expect(screen.getByText(/no citations/i)).toBeInTheDocument();
});

test("an unknown answer shows not found and a failing one shows retry", async () => {
	mockAi();
	server.use(http.get("/api/v1/ai/answers/nope", () => fail(404, "Answer not found.")));
	renderApp("/sites/s1/ai/answers/nope");
	expect(await screen.findByText(/that answer was not found/i)).toBeInTheDocument();
});

test("an answer that fails to load shows the error with retry", async () => {
	mockAi();
	server.use(http.get("/api/v1/ai/answers/a1", () => HttpResponse.error()));
	renderApp("/sites/s1/ai/answers/a1");
	expect(await screen.findByText("Loading the answer…")).toBeInTheDocument();
	expect(await screen.findByText(/can't be reached/i)).toBeInTheDocument();
	server.use(http.get("/api/v1/ai/answers/a1", () => ok(aiAnswerDetail())));
	await userEvent.click(screen.getByRole("button", { name: "Retry" }));
	expect(await screen.findByRole("heading", { name: "What is the best map maker?" })).toBeInTheDocument();
});

test("the engines are listed in the order the API lists them", async () => {
	const { ENGINES } = await import("../../src/ai/engines");
	expect(ENGINES.map((engine) => engine.id)).toEqual(["claude", "chatgpt", "grok", "gemini", "perplexity"]);
});
