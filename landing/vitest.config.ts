import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
	plugins: [react()],
	test: {
		environment: "jsdom",
		globals: true,
		setupFiles: ["tests/setup.ts"],
		include: ["tests/unit/**/*.test.{ts,tsx}", "tests/build/**/*.test.ts"],
		coverage: {
			provider: "v8",
			include: ["src/**"],
			exclude: ["src/entry-client.tsx"],
			thresholds: { lines: 80, functions: 80, branches: 80, statements: 80 },
		},
	},
});
