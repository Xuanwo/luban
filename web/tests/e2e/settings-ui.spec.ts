import { expect, test } from "@playwright/test"
import { ensureWorkspace, fetchAppSnapshot } from "./helpers"

test("settings panel updates theme and appearance fonts", async ({ page }) => {
  await ensureWorkspace(page)

  await page.getByTestId("sidebar-open-settings").click()
  await expect(page.getByTestId("settings-panel")).toBeVisible({ timeout: 10_000 })

  await page.getByTestId("settings-theme-dark").click()
  await page.getByTitle("Close settings").click()

  await expect
    .poll(async () => await page.evaluate(() => document.documentElement.classList.contains("dark")), {
      timeout: 10_000,
    })
    .toBeTruthy()

  await page.getByTestId("sidebar-open-settings").click()
  await expect(page.getByTestId("settings-panel")).toBeVisible({ timeout: 10_000 })

  const uiFontInput = page.getByTestId("settings-ui-font")
  await uiFontInput.click()
  await uiFontInput.fill("Roboto")

  await expect
    .poll(
      async () =>
        await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--luban-font-ui").trim()),
      { timeout: 5_000 },
    )
    .toBe("Roboto")

  await expect
    .poll(async () => {
      const snap = await fetchAppSnapshot(page)
      return String(snap?.appearance?.fonts?.ui_font ?? "")
    })
    .toBe("Roboto")

  await page.evaluate(() => {
    localStorage.removeItem("theme")
    localStorage.removeItem("resolvedTheme")
  })
  await page.reload()

  await expect
    .poll(async () => await page.evaluate(() => document.documentElement.classList.contains("dark")), {
      timeout: 20_000,
    })
    .toBeTruthy()

  await expect
    .poll(
      async () =>
        await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--luban-font-ui").trim()),
      { timeout: 20_000 },
    )
    .toBe("Roboto")
})

test("task prompt templates persist across reload", async ({ page }) => {
  await ensureWorkspace(page)

  await page.getByTestId("sidebar-open-settings").click()
  await expect(page.getByTestId("settings-panel")).toBeVisible({ timeout: 10_000 })

  await page.getByRole("button", { name: "Task", exact: true }).click()
  await expect(page.getByTestId("task-prompt-editor")).toBeVisible({ timeout: 10_000 })

  await page.getByTestId("task-prompt-tab-other").click()
  const textarea = page.getByTestId("task-prompt-template")
  const original = await textarea.inputValue()

  const marker = `e2e-marker-${Date.now()}`
  await textarea.fill(`${original}\n${marker}\n`)

  await page.waitForTimeout(1500)
  await page.reload()

  await page.getByTestId("sidebar-open-settings").click()
  await expect(page.getByTestId("settings-panel")).toBeVisible({ timeout: 10_000 })

  await page.getByRole("button", { name: "Task", exact: true }).click()
  await page.getByTestId("task-prompt-tab-other").click()
  await expect
    .poll(async () => await page.getByTestId("task-prompt-template").inputValue(), { timeout: 10_000 })
    .toContain(marker)

  await page.getByTestId("task-prompt-template").fill(original)
  await page.waitForTimeout(1500)
  await page.getByTitle("Close settings").click()
})

test("system prompt templates persist across reload", async ({ page }) => {
  await ensureWorkspace(page)

  await page.getByTestId("sidebar-open-settings").click()
  await expect(page.getByTestId("settings-panel")).toBeVisible({ timeout: 10_000 })

  await page.getByRole("button", { name: "Task", exact: true }).click()
  await expect(page.getByTestId("task-prompt-editor")).toBeVisible({ timeout: 10_000 })

  await page.getByTestId("task-prompt-tab-infer-type").click()
  const textarea = page.getByTestId("task-prompt-template")
  const original = await textarea.inputValue()

  const marker = `e2e-system-marker-${Date.now()}`
  await textarea.fill(`${original}\n${marker}\n`)

  await page.waitForTimeout(1500)
  await page.reload()

  await page.getByTestId("sidebar-open-settings").click()
  await expect(page.getByTestId("settings-panel")).toBeVisible({ timeout: 10_000 })

  await page.getByRole("button", { name: "Task", exact: true }).click()
  await page.getByTestId("task-prompt-tab-infer-type").click()
  await expect
    .poll(async () => await page.getByTestId("task-prompt-template").inputValue(), { timeout: 10_000 })
    .toContain(marker)

  await page.getByTestId("task-prompt-template").fill(original)
  await page.waitForTimeout(1500)
  await page.getByTitle("Close settings").click()
})

test("agent selector defaults come from Codex config", async ({ page }) => {
  await ensureWorkspace(page)

  await expect
    .poll(async () => {
      const res = await page.request.get("/api/app")
      if (!res.ok()) return null
      const app = (await res.json()) as { agent?: { default_model_id?: string | null } }
      return app.agent?.default_model_id ?? null
    })
    .toBe("gpt-5.2-codex")

  const selector = page.getByTestId("agent-selector")
  await selector.click()

  const menu = page
    .locator("div")
    .filter({ hasText: "Agent" })
    .filter({ hasText: "Model" })
    .filter({ hasText: "Reasoning" })
    .first()

  const model = menu.getByRole("button", { name: "GPT-5.2-Codex", exact: true })
  await expect(model.locator("..").getByText("default", { exact: true })).toHaveCount(1)

  const effort = menu.getByRole("button", { name: "High", exact: true })
  await expect(effort.locator("..").getByText("default", { exact: true })).toHaveCount(1)
})

test("default selector button opens Codex config editor", async ({ page }) => {
  await ensureWorkspace(page)

  const selector = page.getByTestId("agent-selector")
  await selector.click()

  const menu = page
    .locator("div")
    .filter({ hasText: "Agent" })
    .filter({ hasText: "Model" })
    .filter({ hasText: "Reasoning" })
    .first()

  const defaultModel = menu.getByRole("button", { name: "GPT-5.2-Codex", exact: true })
  await defaultModel.hover()

  const openDefaults = defaultModel.locator("..").getByTitle("Edit Codex defaults")
  await openDefaults.click()

  await expect(page.getByTestId("settings-panel")).toBeVisible({ timeout: 10_000 })
  await expect(page.getByTestId("settings-codex-editor")).toBeVisible({ timeout: 10_000 })
  await expect(page.getByTestId("settings-codex-editor")).toHaveValue(/model\s*=\s*"gpt-5\.2-codex"/)

  await expect
    .poll(async () => {
      return await page.getByTestId("settings-codex-editor").evaluate((el) => document.activeElement === el)
    })
    .toBeTruthy()
})
