import type { EngineId } from "../api/types";

/** The AI engines, in the order the API lists them, with their names. Keys are added in Settings. */
export const ENGINES: { id: EngineId; name: string }[] = [
	{ id: "claude", name: "Claude" },
	{ id: "chatgpt", name: "ChatGPT" },
	{ id: "grok", name: "Grok" },
	{ id: "gemini", name: "Gemini" },
	{ id: "perplexity", name: "Perplexity" },
];

export const engineName = (id: EngineId): string => ENGINES.find((engine) => engine.id === id)?.name ?? id;
