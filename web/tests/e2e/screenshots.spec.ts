import { mkdirSync } from "node:fs";
import { test } from "@playwright/test";
import { expect } from "@playwright/test";
import { fakeApi, signIn } from "./fakeApi";

const SHOTS = ".shots";

/** Not a check of behaviour: it writes the pictures a person looks at after a restyle. */
test("screenshots_of_the_site_overview", async ({ page }) => {
	mkdirSync(SHOTS, { recursive: true });
	await fakeApi(page);
	await page.setViewportSize({ width: 1440, height: 900 });
	await signIn(page);
	await page.getByRole("link", { name: "Add a site" }).click();
	await page.getByLabel("Website address").fill("example.com");
	await page.getByRole("button", { name: "Add site" }).click();
	await expect(page.getByText("out of 100")).toBeVisible({ timeout: 10_000 });
	await page.waitForLoadState("networkidle");
	await page.screenshot({ path: `${SHOTS}/overview-desktop.png` });

	await page.setViewportSize({ width: 390, height: 844 });
	await page.reload();
	await expect(page.getByText("out of 100")).toBeVisible();
	await page.waitForLoadState("networkidle");
	await page.screenshot({ path: `${SHOTS}/overview-mobile.png` });

	await page.getByRole("button", { name: "Open menu" }).click();
	await page.screenshot({ path: `${SHOTS}/overview-mobile-menu.png` });
});
