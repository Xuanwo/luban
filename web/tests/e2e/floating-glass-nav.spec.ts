import { expect, test, type Page } from "@playwright/test"
import fs from "node:fs"

import { ensureWorkspace, sendWsAction } from "./helpers"
import { requireEnv } from "./env"

function projectToggleByPath(page: Page, projectPath: string) {
  return page.getByTitle(projectPath, { exact: true }).locator("..")
}

test("floating glass selection is consistent across navigation surfaces", async ({ page }) => {
  await ensureWorkspace(page)

  await expect(page.getByTestId("right-sidebar-tab-terminal")).toHaveAttribute("data-active", "true")
  await expect(page.getByTestId("right-sidebar-tab-terminal")).toHaveClass(/\bluban-float-glass\b/)

  const projectPath = fs.realpathSync(requireEnv("LUBAN_E2E_PROJECT_DIR"))
  const projectToggle = projectToggleByPath(page, projectPath)
  await projectToggle.waitFor({ state: "visible", timeout: 15_000 })
  const projectContainer = projectToggle.locator("..").locator("..")

  const worktreeRows = projectContainer.getByTestId("worktree-row")
  await worktreeRows.first().waitFor({ timeout: 15_000 })

  const initialWorktreeCount = await worktreeRows.count()
  await sendWsAction(page, { type: "create_workspace", project_id: projectPath })
  await expect
    .poll(async () => await worktreeRows.count(), { timeout: 90_000 })
    .toBe(initialWorktreeCount + 1)

  await worktreeRows.last().focus()
  await page.keyboard.press("Enter")
  await expect(worktreeRows.last()).toHaveAttribute("data-active", "true")
  await expect(worktreeRows.last()).toHaveClass(/\bluban-float-glass\b/)

  const activeThreadTab = page.locator('[data-testid^="thread-tab-"][data-active="true"]').first()
  await expect(activeThreadTab).toBeVisible()
  await expect(activeThreadTab).toHaveClass(/\bluban-float-glass\b/)
})
