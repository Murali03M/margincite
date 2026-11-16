import { readFileSync } from "node:fs";
import { join } from "node:path";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderToString } from "react-dom/server";
import { ThemeToggle } from "../../src/components/ThemeToggle";
import { THEME_KEY, applyTheme, readTheme } from "../../src/lib/theme";

function memoryStorage(initial: Record<string, string> = {}) {
	const data = new Map(Object.entries(initial));
	return { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v), data };
}

// Node's own localStorage shadows jsdom's and is unavailable, so each test gets a fresh one.
beforeEach(() => {
	vi.stubGlobal("localStorage", memoryStorage());
});

afterEach(() => {
	delete document.documentElement.dataset.theme;
});

it("is dark unless light was chosen", () => {
	expect(readTheme(memoryStorage())).toBe("dark");
	expect(readTheme(memoryStorage({ [THEME_KEY]: "light" }))).toBe("light");
	expect(readTheme(memoryStorage({ [THEME_KEY]: "neon" }))).toBe("dark");
	expect(readTheme(undefined)).toBe("dark");
});

it("survives blocked storage", () => {
	const blocked = {
		getItem: () => {
			throw new Error("blocked");
		},
		setItem: () => {
			throw new Error("blocked");
		},
	};
	expect(readTheme(blocked)).toBe("dark");
	expect(() => applyTheme(document.documentElement, "light", blocked)).not.toThrow();
	expect(document.documentElement.dataset.theme).toBe("light");
});

it("marks the page and remembers the choice", () => {
	const storage = memoryStorage();
	applyTheme(document.documentElement, "light", storage);
	expect(document.documentElement.dataset.theme).toBe("light");
	expect(storage.data.get(THEME_KEY)).toBe("light");
	applyTheme(document.documentElement, "dark", storage);
	expect(document.documentElement.dataset.theme).toBe("dark");
	expect(storage.data.get(THEME_KEY)).toBe("dark");
});

it("sets the remembered theme before the page paints, with the same storage key", () => {
	const html = readFileSync(join(__dirname, "..", "..", "index.html"), "utf8");
	const head = html.slice(0, html.indexOf("</head>"));
	expect(head).toContain(`localStorage.getItem("${THEME_KEY}")`);
});

it("renders the same button on the server as on first load, offering the light theme", () => {
	expect(renderToString(<ThemeToggle />)).toContain('aria-label="Switch to light theme"');
});

it("switches between the dark and light rooms", async () => {
	render(<ThemeToggle />);
	await userEvent.click(screen.getByRole("button", { name: "Switch to light theme" }));
	expect(document.documentElement.dataset.theme).toBe("light");
	expect(localStorage.getItem(THEME_KEY)).toBe("light");
	await userEvent.click(screen.getByRole("button", { name: "Switch to dark theme" }));
	expect(document.documentElement.dataset.theme).toBe("dark");
});

it("picks up a theme set before it loaded", async () => {
	document.documentElement.dataset.theme = "light";
	render(<ThemeToggle />);
	await act(async () => {});
	expect(screen.getByRole("button", { name: "Switch to dark theme" })).toBeInTheDocument();
});
