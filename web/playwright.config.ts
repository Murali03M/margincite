import { defineConfig, devices } from "@playwright/test";

// Not 5173 (landing dev), 5174 (this app's dev server) or 4173 (landing preview).
const PORT = 5175;

export default defineConfig({
	testDir: "tests/e2e",
	workers: 1,
	reporter: "list",
	use: { baseURL: `http://localhost:${PORT}` },
	projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
	webServer: {
		// Built into its own folder, so the browser tests never touch the real dist/.
		command: `DIST_DIR=dist-e2e pnpm exec vite build && DIST_DIR=dist-e2e pnpm exec vite preview --port ${PORT} --strictPort`,
		url: `http://localhost:${PORT}`,
		reuseExistingServer: false,
		timeout: 120_000,
	},
});
