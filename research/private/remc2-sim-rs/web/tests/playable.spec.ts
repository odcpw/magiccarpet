import { expect, test } from "@playwright/test";

test("boots the Rust core and accepts flight, fire, pause and reset controls", async ({
  page
}) => {
  const consoleErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => consoleErrors.push(error.message));

  await page.goto("/?automation=1");
  const enter = page.getByTestId("enter-button");
  await expect(enter).toBeEnabled();
  await expect(page.getByTestId("telemetry")).toContainText("Rust core · tick 0");
  await enter.click();
  await expect(page.getByTestId("intro")).toBeHidden();

  await page.keyboard.down("KeyW");
  await expect
    .poll(async () => Number(await page.locator("#speed-value").innerText()))
    .toBeGreaterThan(0);
  await page.keyboard.up("KeyW");
  await expect(page.getByTestId("telemetry")).not.toContainText("tick 0 ·");

  await page.keyboard.press("Space");
  await expect(page.locator("#game-shell")).toHaveAttribute("data-shots", "1");

  await page.keyboard.press("KeyP");
  await expect(page.locator("#pause-panel")).toBeVisible();
  const pausedTick = await page.locator("#game-shell").getAttribute("data-tick");
  await page.waitForTimeout(250);
  await expect(page.locator("#game-shell")).toHaveAttribute(
    "data-tick",
    pausedTick ?? ""
  );
  await page.keyboard.press("KeyP");
  await expect(page.locator("#pause-panel")).toBeHidden();

  await page.keyboard.press("KeyR");
  await expect(page.locator("#game-shell")).toHaveAttribute("data-resets", "1");
  await expect(page.locator("#speed-value")).toHaveText("0");

  expect(consoleErrors).toEqual([]);
});

test("renders the playable at desktop and compact viewport sizes", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/?automation=1");
  await expect(page.getByTestId("enter-button")).toBeEnabled();
  await page.getByTestId("enter-button").click();
  await expect(page.getByTestId("intro")).toBeHidden();

  await page.keyboard.down("KeyW");
  await expect
    .poll(async () => Number(await page.locator("#speed-value").innerText()))
    .toBeGreaterThan(20);
  await page.keyboard.up("KeyW");
  await page.screenshot({
    path: "test-results/playable-desktop.png",
    fullPage: true
  });

  await page.setViewportSize({ width: 820, height: 720 });
  await expect(page.locator("#game-canvas")).toBeVisible();
  await expect(page.getByTestId("objective-progress")).toBeVisible();
  await page.screenshot({
    path: "test-results/playable-compact.png",
    fullPage: true
  });
});
