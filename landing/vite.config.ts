import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// DIST_DIR lets the browser tests build somewhere other than the real dist/.
// BASE_PATH is the path the page is served under, such as /margincite/ on GitHub Pages.
export default defineConfig({
	plugins: [react(), tailwindcss()],
	base: process.env.BASE_PATH ?? "/",
	build: { outDir: process.env.DIST_DIR ?? "dist" },
});
