import { expect, test, type Page } from "@playwright/test";
import { fakeApi, signIn, startSignedIn } from "./fakeApi";

test("sign_in_add_a_site_watch_the_crawl_and_open_a_suggestion", async ({ page }) => {
	await fakeApi(page);
	await signIn(page);

	await expect(page.getByRole("heading", { name: "Add your first site" })).toBeVisible();
	await page.getByRole("link", { name: "Add a site" }).click();
	await page.getByLabel("Website address").fill("example.com");
	await page.getByRole("button", { name: "Add site" }).click();

	await expect(page).toHaveURL(/\/sites\/s1$/);
	await expect(page.getByText("3 pages crawled so far.")).toBeVisible();
	await expect(page.getByText("No score yet. It appears when the crawl finishes.")).toBeVisible();
	await expect(page.getByText("9 pages crawled so far.")).toBeVisible({ timeout: 10_000 });
	await expect(page.getByText("out of 100")).toBeVisible({ timeout: 10_000 });
	await expect(page.getByText("72", { exact: true }).first()).toBeVisible();

	await page.getByRole("list", { name: "Top suggestions" }).getByRole("link", { name: "Pages are missing a title" }).click();
	await expect(page).toHaveURL(/\/suggestions\/g1\?site=s1$/);
	await expect(page.getByRole("heading", { level: 1, name: "Pages are missing a title" })).toBeVisible();
	await expect(page.getByText("Title length: 0")).toHaveCount(2);

	const row = page.getByRole("row").filter({ hasText: "example.com/contact" });
	await row.getByRole("button", { name: /^Done:/ }).click();
	await expect(row.locator(".tag")).toHaveText("Done");
	await expect(page.getByText("1 open, 1 done, 0 dismissed")).toBeVisible();

	await page.getByRole("link", { name: "Back to suggestions" }).click();
	await expect(page).toHaveURL(/\/sites\/s1\/suggestions$/);
});

test("keywords_then_an_ai_run_from_a_suggested_question_to_an_answer", async ({ page }) => {
	const api = await fakeApi(page);
	api.siteAdded = true;
	await startSignedIn(page);

	await page.goto("/sites/s1/keywords");
	for (const word of ["road maps", "city guides"]) {
		await page.getByLabel("Keyword", { exact: true }).fill(word);
		await page.getByRole("button", { name: "Add keyword" }).click();
		await expect(page.getByRole("table", { name: "Target keywords" }).getByText(word)).toBeVisible();
	}

	await page.getByRole("navigation", { name: "Site sections" }).getByRole("link", { name: "AI Visibility" }).click();
	await expect(page.getByRole("heading", { name: "AI Visibility" })).toBeVisible();
	await expect(page.getByText("No results yet")).toBeVisible();

	await page.getByRole("button", { name: "Suggest questions" }).click();
	const suggested = "Which company makes the best printed city maps?";
	await page.getByRole("button", { name: `Add: ${suggested}` }).click();
	await expect(page.getByRole("list", { name: "Questions" }).getByText(suggested)).toBeVisible();

	await page.getByRole("button", { name: "Run now" }).click();
	await expect(page.getByRole("progressbar", { name: "Run progress" })).toBeVisible();
	const grid = page.getByRole("table", { name: "Answers by question and engine" });
	await expect(grid).toBeVisible({ timeout: 15_000 });
	await expect(grid.getByRole("columnheader", { name: "Claude" })).toBeVisible();

	await grid.getByRole("link", { name: "Claude answer: Mentioned" }).click();
	await expect(page.getByRole("heading", { name: suggested })).toBeVisible();
	await expect(page.getByText("Example makes good maps.")).toBeVisible();
	await page.getByRole("link", { name: /Back to AI Visibility/ }).click();
	await expect(page).toHaveURL(/\/sites\/s1\/ai$/);
});

const SCREENS: [string, string | RegExp][] = [
	["/", "Your sites"],
	["/sites/s1", "Crawl facts"],
	["/sites/s1/suggestions", "Suggestions"],
	["/sites/s1/pages", "Pages"],
	["/suggestions/g1?site=s1", "Pages are missing a title"],
	["/pages/p1", /A page with a very long title/],
];

test("with_the_backend_down_every_screen_says_so_and_recovers_on_retry", async ({ page }) => {
	const api = await fakeApi(page);
	api.siteAdded = true;
	await startSignedIn(page);
	for (const [path, heading] of SCREENS) {
		api.down = true;
		await page.goto(path);
		await expect(page.getByRole("alert")).toContainText("Margincite can't be reached");
		api.down = false;
		await page.getByRole("button", { name: "Retry" }).click();
		await expect(page.getByRole("heading", { name: heading }).first()).toBeVisible();
		await expect(page.getByRole("alert")).toHaveCount(0);
	}
});

test("an_expired_token_returns_to_sign_in", async ({ page }) => {
	const api = await fakeApi(page);
	api.siteAdded = true;
	await signIn(page);
	await expect(page.getByRole("heading", { name: "Your sites" })).toBeVisible();
	api.expired = true;
	await page.reload();
	await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
	api.expired = false;
	await page.getByLabel("Access token").fill("dev-token");
	await page.getByRole("button", { name: "Sign in" }).click();
	await expect(page.getByRole("heading", { name: "Your sites" })).toBeVisible();
});

const NEW_SCREENS = [
	"/sites/s1/checks",
	"/sites/s1/keywords",
	"/sites/s1/ai",
	"/sites/s1/search",
	"/sites/s1/backlinks",
	"/sites/s1/connections",
	"/settings",
	"/sites/s1/rankings",
];
const OVERFLOW_SCREENS = [...SCREENS.map(([path]) => path), ...NEW_SCREENS, "/sites/s1/settings", "/sites/new"];

/** Elements wider than the window, not counting what sits inside a table's own scrolling box. */
async function overflowing(page: Page): Promise<string[]> {
	return page.evaluate(() =>
		[...document.querySelectorAll("main *, header *")]
			.filter((el) => !el.closest("svg") && el.getBoundingClientRect().width > 0)
			.filter((el) => {
				const box = el.closest('[role="region"][tabindex="0"], [data-scroll]');
				return !box || box === el;
			})
			.filter((el) => el.getBoundingClientRect().right > window.innerWidth + 0.5 || el.getBoundingClientRect().left < -0.5)
			.map((el) => `${el.tagName}.${String(el.className)}`.slice(0, 80)),
	);
}

for (const width of [390, 1280]) {
	test(`nothing_is_cut_off_at_${width}_pixels_wide`, async ({ page }) => {
		const api = await fakeApi(page);
		api.siteAdded = true;
		await startSignedIn(page);
		await page.setViewportSize({ width, height: 800 });
		for (const path of OVERFLOW_SCREENS) {
			await page.goto(path);
			await page.waitForLoadState("networkidle");
			await expect(page.getByRole("heading").first()).toBeVisible();
			expect(await overflowing(page), path).toEqual([]);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), `${path} scrolls sideways`).toBe(true);
		}
	});
}

test("a wide table scrolls inside its own box on a phone", async ({ page }) => {
	const api = await fakeApi(page);
	api.siteAdded = true;
	await startSignedIn(page);
	await page.setViewportSize({ width: 390, height: 800 });
	await page.goto("/sites/s1/pages");
	const box = page.getByRole("region", { name: "Pages from the latest crawl" });
	await expect(box).toBeVisible();
	expect(await box.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});

test("the_app_can_be_used_with_the_keyboard_alone", async ({ page }) => {
	await fakeApi(page);
	await startSignedIn(page);
	await page.goto("/");
	await expect(page.getByRole("heading", { name: "Add your first site" })).toBeVisible();

	const focusIsVisible = () =>
		page.evaluate(() => {
			const style = getComputedStyle(document.activeElement as Element);
			return style.outlineStyle !== "none" && Number.parseFloat(style.outlineWidth) > 0;
		});
	const focusedName = () => page.evaluate(() => (document.activeElement?.textContent ?? "").trim());

	for (let step = 0; step < 12 && (await focusedName()) !== "Add a site"; step += 1) {
		await page.keyboard.press("Tab");
		expect(await focusIsVisible(), `focus ring at step ${step}`).toBe(true);
	}
	expect(await focusedName()).toBe("Add a site");
	await page.keyboard.press("Enter");

	await expect(page.getByRole("heading", { name: "Add a site" })).toBeVisible();
	const field = page.getByLabel("Website address");
	for (let step = 0; step < 12 && !(await field.evaluate((el) => el === document.activeElement)); step += 1) {
		await page.keyboard.press("Tab");
		expect(await focusIsVisible(), `focus ring on the add page, step ${step}`).toBe(true);
	}
	await expect(field).toBeFocused();
	expect(await focusIsVisible()).toBe(true);
	await page.keyboard.type("example.com");
	await page.keyboard.press("Tab");
	expect(await focusedName()).toBe("Add site");
	expect(await focusIsVisible()).toBe(true);
	await page.keyboard.press("Enter");

	await expect(page).toHaveURL(/\/sites\/s1$/);
	await expect(page.getByRole("heading", { level: 1, name: "example.com" })).toBeVisible();
});

test("on a phone the sections sit in a drawer that opens from the menu button and closes on Escape", async ({ page }) => {
	const api = await fakeApi(page);
	api.siteAdded = true;
	await startSignedIn(page);
	await page.setViewportSize({ width: 390, height: 800 });
	await page.goto("/sites/s1");
	const sections = page.getByRole("navigation", { name: "Site sections" });
	await expect(sections).toBeHidden();
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);

	const menu = page.getByRole("button", { name: "Open menu" });
	await menu.click();
	await expect(sections).toBeVisible();
	await expect(sections.getByRole("link", { name: "Site settings" })).toBeVisible();
	await page.keyboard.press("Escape");
	await expect(sections).toBeHidden();
	await expect(menu).toBeFocused();

	await menu.click();
	await sections.getByRole("link", { name: "Checks" }).click();
	await expect(page).toHaveURL(/\/sites\/s1\/checks$/);
	await expect(sections).toBeHidden();
});

test("on a wide screen the sidebar is always there and the menu button is not", async ({ page }) => {
	const api = await fakeApi(page);
	api.siteAdded = true;
	await startSignedIn(page);
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto("/sites/s1");
	await expect(page.getByRole("navigation", { name: "Site sections" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Open menu" })).toBeHidden();
	const box = await page.getByRole("complementary", { name: "Sidebar" }).boundingBox();
	expect(box?.width).toBe(272);
});

test("settings_then_a_keyword_then_rankings", async ({ page }) => {
	const api = await fakeApi(page);
	api.siteAdded = true;
	await startSignedIn(page);
	await page.setViewportSize({ width: 390, height: 800 });
	await page.goto("/");

	await expect(page.getByRole("region", { name: "Get set up" })).toBeVisible();
	await page.getByRole("button", { name: "Open menu" }).click();
	await page.getByRole("complementary", { name: "Sidebar" }).getByRole("link", { name: "Settings" }).click();
	await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();

	const contact = page.getByRole("region", { name: "Crawler" });
	await contact.getByLabel("Contact address").fill("you@example.com");
	await contact.getByRole("button", { name: "Save contact" }).click();
	await expect(contact.getByText("Saved", { exact: true })).toBeVisible();

	const key = "canary-not-a-real-key";
	const claude = page.getByRole("region", { name: "Claude" });
	await claude.getByLabel("Claude API key").fill(key);
	await claude.getByRole("button", { name: "Save Claude" }).click();
	await expect(claude.getByText("Working")).toBeVisible();
	await expect(claude.getByLabel("Claude API key")).toHaveValue("");
	await expect(page.locator("body")).not.toContainText(key);
	expect(await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }))).not.toContain(key);
	expect(page.url()).not.toContain(key);

	const data = page.getByRole("region", { name: "Search data (DataForSEO)" });
	await data.getByLabel("DataForSEO login").fill("you@example.com");
	await data.getByLabel("DataForSEO password").fill("canary-not-a-real-password");
	await data.getByRole("button", { name: "Save DataForSEO" }).click();
	await expect(data.getByText(/Balance/)).toBeVisible();
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);

	await page.goto("/sites/s1/keywords");
	await page.getByLabel("Keyword", { exact: true }).fill("road maps");
	await page.getByRole("button", { name: "Add keyword" }).click();
	await expect(page.getByRole("table", { name: "Target keywords" }).getByText("road maps")).toBeVisible();

	await page.getByRole("button", { name: "Open menu" }).click();
	await page.getByRole("navigation", { name: "Site sections" }).getByRole("link", { name: "Rankings" }).click();
	const table = page.getByRole("table", { name: "Keyword rankings" });
	await expect(table.getByText("road maps")).toBeVisible();
	await expect(table).toContainText("Up 2 places");
	await page.getByRole("button", { name: "Show top 10 for road maps" }).click();
	await expect(page.getByRole("table", { name: "Top 10 results for road maps" })).toBeVisible();
	await expect(page.getByRole("img", { name: /position history for road maps/i })).toBeVisible();
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});
