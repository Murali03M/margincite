// Dev helper: screenshot a URL or file. node scripts/shot.mjs <url> <out.png> <width> <height> [full]
import { chromium } from "@playwright/test";

const [url, out, width = "1440", height = "900", full] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({
	viewport: { width: Number(width), height: Number(height) },
	reducedMotion: "reduce",
});
await page.goto(url, { waitUntil: "networkidle" });
await page.screenshot({ path: out, fullPage: full === "full" });
await browser.close();
