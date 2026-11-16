import { expect, test } from "@playwright/test";

const headings = ["What it does today", "SEO, AIO and GEO, side by side", "How it works", "One finding, up close", "Questions, answered", "Run it on your own site"];
const counter = "https://abacus.jasoncameron.dev/**";

// The real counting service is never called from tests.
test.beforeEach(async ({ page }) => {
	await page.route(counter, (route) => route.fulfill({ status: 200, contentType: "application/json", body: '{"value":1234}' }));
});

test.describe("with JavaScript off", () => {
	test.use({ javaScriptEnabled: false });

	test("every section is readable and the FAQ opens", async ({ page }) => {
		await page.goto("/");
		await expect(page.getByRole("heading", { level: 1 })).toContainText("Every fix comes with");
		for (const name of headings) await expect(page.getByRole("heading", { level: 2, name })).toBeVisible();
		await page.getByText("Does it cost anything?").click();
		await expect(page.getByText(/Margincite is free\. The services you connect/)).toBeVisible();
		expect(await page.content()).not.toMatch(/opacity:\s*0[;"]/);
	});
});

test("the visit count rolls into the top bar", async ({ page }) => {
	await page.goto("/");
	await expect(page.getByRole("status").filter({ hasText: "visits to this page" })).toHaveText("1,234 visits to this page");
	await expect(page.locator("header [data-digit]")).toHaveCount(6);
});

test("the top bar works without the counter when the service is down", async ({ page }) => {
	await page.unroute(counter);
	await page.route(counter, (route) => route.abort());
	await page.goto("/");
	await expect(page.locator("header [data-digit]")).toHaveCount(0);
	await expect(page.getByRole("link", { name: "Margincite home" })).toBeVisible();
});

test("the copy button copies the install commands", async ({ page, context }) => {
	await context.grantPermissions(["clipboard-read", "clipboard-write"]);
	await page.goto("/");
	await page.getByRole("button", { name: "Copy commands" }).click();
	await expect(page.getByRole("button", { name: "Copied" })).toBeVisible();
	expect(await page.evaluate(() => navigator.clipboard.readText())).toContain("bash scripts/dev.sh");
});

for (const width of [320, 390, 768, 1024, 1280, 1920]) {
	test(`nothing is cut off at ${width} pixels wide`, async ({ page }) => {
		await page.setViewportSize({ width, height: 800 });
		await page.goto("/");
		await page.waitForTimeout(300);
		// The page clips sideways overflow, so measure the elements themselves. The terminal scrolls on its own.
		const overflowing = await page.evaluate(() =>
			[...document.querySelectorAll("main *, header *, footer *")]
				.filter((el) => !el.closest("svg") && !el.closest("pre") && el.getBoundingClientRect().width > 0)
				.filter((el) => el.getBoundingClientRect().right > window.innerWidth + 0.5 || el.getBoundingClientRect().left < -0.5)
				.map((el) => `${el.tagName}.${el.className}`.slice(0, 80)),
		);
		expect(overflowing).toEqual([]);
	});
}

for (const [width, height] of [[1280, 720], [1366, 768], [1440, 900]] as const) {
	test(`the headline and both buttons fit the first screen at ${width}x${height}`, async ({ page }) => {
		await page.setViewportSize({ width, height });
		await page.goto("/");
		for (const target of [page.getByRole("heading", { level: 1 }), page.getByRole("link", { name: "See how it works" })]) {
			const box = (await target.boundingBox())!;
			expect(box.y).toBeGreaterThanOrEqual(64);
			expect(box.y + box.height).toBeLessThanOrEqual(height);
		}
	});
}

test("phone content keeps a side gutter", async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 800 });
	await page.goto("/");
	const heading = page.getByRole("heading", { level: 1 });
	expect((await heading.boundingBox())!.x).toBeGreaterThanOrEqual(16);
	await page.getByRole("button", { name: "Open menu" }).click();
	const link = page.locator("#mobile-menu").getByRole("link", { name: "Features" });
	expect((await link.boundingBox())!.x).toBeGreaterThanOrEqual(16);
});

test("the header stays in view while scrolling", async ({ page }) => {
	await page.goto("/");
	await page.mouse.wheel(0, 1500);
	await expect(page.getByRole("link", { name: "Margincite home" })).toBeInViewport();
});

test("the skip link is readable when focused", async ({ page }) => {
	await page.goto("/");
	await page.keyboard.press("Tab");
	const skip = page.getByRole("link", { name: "Skip to content" });
	await expect(skip).toBeFocused();
	expect(await skip.evaluate((el) => getComputedStyle(el).backgroundColor)).not.toBe("rgba(0, 0, 0, 0)");
});

test("the mobile menu opens and leads to a section", async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 800 });
	await page.goto("/");
	await page.getByRole("button", { name: "Open menu" }).click();
	await page.locator("#mobile-menu").getByRole("link", { name: "How it works" }).click();
	await expect(page).toHaveURL(/#how$/);
	await expect(page.getByRole("button", { name: "Open menu" })).toBeVisible();
});

test("the example finding shows its three markers", async ({ page }) => {
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.goto("/");
	const figure = page.getByRole("figure", { name: /Example finding, illustrated/ });
	await expect(figure).toBeVisible();
	await expect(figure.locator("[data-marker]")).toHaveCount(3);
});

test("the page loads with no console errors or failed requests", async ({ page }) => {
	const problems: string[] = [];
	page.on("console", (message) => {
		if (message.type() === "error" || message.type() === "warning") problems.push(message.text());
	});
	page.on("pageerror", (error) => problems.push(error.message));
	page.on("requestfailed", (request) => problems.push(`failed: ${request.url()}`));
	page.on("response", (response) => {
		if (response.status() >= 400) problems.push(`${response.status()}: ${response.url()}`);
	});
	await page.goto("/");
	await page.waitForLoadState("networkidle");
	expect(problems).toEqual([]);
});
