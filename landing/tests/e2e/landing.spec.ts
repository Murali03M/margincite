import { expect, test } from "@playwright/test";

const endpoint = "https://forms.example.test/f/abc";
const headings = ["What it does today", "How it works", "One note, up close", "Want the notes for your site?"];

test.describe("with JavaScript off", () => {
	test.use({ javaScriptEnabled: false });

	test("every section is readable", async ({ page }) => {
		await page.goto("/");
		await expect(page.getByRole("heading", { level: 1 })).toContainText("Notes in the");
		for (const name of headings) {
			await expect(page.getByRole("heading", { level: 2, name })).toBeVisible();
		}
		expect(await page.content()).not.toMatch(/opacity:\s*0[;"]/);
	});

	test("the form still posts to the service", async ({ page }) => {
		await page.goto("/");
		const form = page.locator("#early-access form");
		await expect(form).toHaveAttribute("action", endpoint);
		await expect(form).toHaveAttribute("method", "post");
	});
});

test("requesting early access confirms", async ({ page }) => {
	await page.route(endpoint, (route) => route.fulfill({ status: 200, contentType: "application/json", body: "{}" }));
	await page.goto("/");
	await page.getByLabel("Email address").fill("a@b.co");
	await page.getByRole("button", { name: "Request early access" }).click();
	await expect(page.getByRole("status")).toHaveText(/Thanks\. You are on the list\./);
});

test("an unreachable service shows an error and keeps the email", async ({ page }) => {
	await page.route(endpoint, (route) => route.abort());
	await page.goto("/");
	await page.getByLabel("Email address").fill("a@b.co");
	await page.getByRole("button", { name: "Request early access" }).click();
	await expect(page.getByRole("alert")).toHaveText(/We could not reach the sign-up service/);
	await expect(page.getByLabel("Email address")).toHaveValue("a@b.co");
});

for (const width of [320, 390, 768, 1024, 1280, 1920]) {
	test(`nothing is cut off at ${width} pixels wide`, async ({ page }) => {
		await page.setViewportSize({ width, height: 800 });
		await page.goto("/");
		// The page clips sideways overflow, so measure the elements themselves.
		const overflowing = await page.evaluate(() =>
			[...document.querySelectorAll("main *, header *, footer *")]
				.filter((el) => !el.closest("svg") && el.getBoundingClientRect().width > 0)
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
			expect(box.y).toBeGreaterThanOrEqual(72);
			expect(box.y + box.height).toBeLessThanOrEqual(height);
		}
	});
}

test("phone content keeps a side gutter", async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 800 });
	await page.goto("/");
	const note = page.getByRole("list", { name: "example.com: 17 notes" }).getByRole("listitem").first();
	expect((await note.boundingBox())!.x).toBeGreaterThanOrEqual(16);
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

test("the desktop map shows the three example fixes", async ({ page }) => {
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.goto("/");
	const map = page.getByRole("img", { name: /Illustrated map of an example site/ });
	await expect(map).toBeVisible();
	await expect(map.locator("[data-control]")).toHaveCount(3);
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
