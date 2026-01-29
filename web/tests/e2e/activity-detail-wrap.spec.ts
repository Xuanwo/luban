import { expect, test } from "@playwright/test"

import { ensureWorkspace } from "./helpers"

test("activity detail wraps long output tokens", async ({ page }) => {
  await ensureWorkspace(page)

  const runId = Math.random().toString(16).slice(2)
  const marker = `e2e-running-card-${runId}-e2e-long-output`

  await page.getByTestId("chat-input").fill(marker)
  await page.getByTestId("chat-send").click()

  const runningHeader = page.getByTestId("agent-running-header")
  await expect(runningHeader).toBeVisible({ timeout: 20_000 })
  await expect(runningHeader).toHaveCount(0, { timeout: 30_000 })

  const activityHeader = page.getByRole("button", { name: /Completed|Cancelled/i }).first()
  await activityHeader.click()

  const echo2 = page.getByRole("button", { name: /echo 2/ }).first()
  await expect(echo2).toBeVisible({ timeout: 20_000 })
  await echo2.click()

  const detail = echo2.locator("..").locator("pre").first()
  await expect(detail).toContainText("io::commit::conflict_resolver")

  const scrollContainer = page.getByTestId("chat-scroll-container")
  await expect
    .poll(async () => {
      const { scrollWidth, clientWidth } = await scrollContainer.evaluate((el) => ({
        scrollWidth: el.scrollWidth,
        clientWidth: el.clientWidth,
      }))
      return scrollWidth <= clientWidth + 1
    })
    .toBe(true)
})
