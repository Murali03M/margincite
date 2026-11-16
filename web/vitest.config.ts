import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
	plugins: [react()],
	test: {
		environment: "jsdom",
		testTimeout: 15000,
		globals: true,
		setupFiles: ["tests/setup.ts"],
		include: ["tests/unit/**/*.test.{ts,tsx}"],
		coverage: {
			provider: "v8",
			include: ["src/**"],
			exclude: ["src/main.tsx"],
			thresholds: { lines: 80, functions: 80, branches: 80, statements: 80 },
		},
	},
});
