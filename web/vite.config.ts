import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The browser only ever talks to this origin; the backend's address stays here.
const BACKEND = "http://127.0.0.1:8787";

export default defineConfig({
	plugins: [react(), tailwindcss()],
	// DIST_DIR lets the browser tests build somewhere other than the real dist/.
	build: { outDir: process.env.DIST_DIR ?? "dist" },
	server: {
		port: 5174,
		strictPort: true,
		proxy: { "/api": BACKEND, "/healthz": BACKEND },
	},
});
