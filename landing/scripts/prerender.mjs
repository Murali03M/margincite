#!/usr/bin/env node
/**
 * Last build step. Fills the built index.html with the server-rendered page
 * and its head tags, then writes robots.txt and sitemap.xml.
 */
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, process.env.DIST_DIR ?? "dist");
const ssr = join(root, "dist-ssr");

try {
	const server = await import(pathToFileURL(join(ssr, "entry-server.js")).href);
	const siteUrl = server.normalizeSiteUrl(process.env.SITE_URL ?? "https://margincite.com");

	const template = readFileSync(join(dist, "index.html"), "utf8");
	for (const marker of ["<!--app-head-->", "<!--app-html-->"]) {
		if (!template.includes(marker)) throw new Error(`index.html is missing ${marker}`);
	}
	const html = template
		.replace("<!--app-head-->", () => server.buildHead(siteUrl))
		.replace("<!--app-html-->", () => server.render());

	writeFileSync(join(dist, "index.html"), html);
	writeFileSync(join(dist, "robots.txt"), server.buildRobots(siteUrl));
	writeFileSync(join(dist, "sitemap.xml"), server.buildSitemap(siteUrl, new Date().toISOString().slice(0, 10)));
	console.log(`Pre-rendered ${siteUrl}/ (${(html.length / 1024).toFixed(1)} KB of HTML)`);
} catch (error) {
	console.error(`Pre-render failed: ${error instanceof Error ? error.message : error}`);
	process.exitCode = 1;
} finally {
	rmSync(ssr, { recursive: true, force: true });
}
