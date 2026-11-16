import { defineConfig, devices } from "@playwright/test";

const ENDPOINT = "https://forms.example.test/f/abc";

export default defineConfig({
	testDir: "tests/e2e",
	workers: 1,
	reporter: "list",
	use: { baseURL: "http://localhost:4173" },
	projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
	webServer: {
		// Built into its own folder, so the test endpoint never ends up in the real dist/.
		command: `DIST_DIR=dist-e2e VITE_EARLY_ACCESS_ENDPOINT=${ENDPOINT} pnpm build && DIST_DIR=dist-e2e pnpm preview`,
		url: "http://localhost:4173",
		reuseExistingServer: false,
		timeout: 120_000,
	},
});
