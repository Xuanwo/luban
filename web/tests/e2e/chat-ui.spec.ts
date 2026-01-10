import { expect, test } from "@playwright/test"
import { ensureWorkspace } from "./helpers"

test("long tokens wrap without horizontal overflow", async ({ page }) => {
  await ensureWorkspace(page)

  const longToken = `e2e-${"a".repeat(600)}`
  const bubbles = page.getByTestId("user-message-bubble")
  const beforeCount = await bubbles.count()
  await page.getByTestId("chat-input").fill(longToken)
  await page.getByTestId("chat-send").click()

  await expect(bubbles).toHaveCount(beforeCount + 1, { timeout: 20_000 })
  const bubble = bubbles.nth(beforeCount)
  await expect(bubble).toContainText("e2e-")

  const overflow = await bubble.evaluate((el) => {
    const e = el as HTMLElement
    return e.scrollWidth - e.clientWidth
  })
  expect(overflow).toBeLessThanOrEqual(1)
})

test("scroll-to-bottom button appears only when away from bottom", async ({ page }) => {
  await ensureWorkspace(page)

  const payload = Array.from({ length: 160 }, (_, i) => `line ${i + 1}`).join("\n")
  await page.getByTestId("chat-input").fill(payload)
  await page.getByTestId("chat-send").click()
  await expect(page.getByTestId("user-message-bubble").filter({ hasText: "line 160" })).toBeVisible({
    timeout: 20_000,
  })

  const button = page.getByRole("button", { name: "Scroll to bottom" })
  await expect(button).toHaveCount(0)

  const scroller = page.getByTestId("chat-scroll-container")
  await scroller.hover()
  await page.mouse.wheel(0, -800)
  await expect(button).toBeVisible()

  await button.click()
  await expect(button).toHaveCount(0)

  const atBottom = await scroller.evaluate((el) => {
    const e = el as HTMLElement
    const distance = e.scrollHeight - e.scrollTop - e.clientHeight
    return distance < 24
  })
  expect(atBottom).toBeTruthy()
})
