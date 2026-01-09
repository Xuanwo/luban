import { expect, test } from "@playwright/test"
import { ensureWorkspace } from "./helpers"

test("worktree item shows branch name above short id", async ({ page }) => {
  await ensureWorkspace(page)

  await page.getByText("e2e-project", { exact: true }).click()
  const worktreeId = page.getByTestId("worktree-short-id").first()
  const branchName = page.getByTestId("worktree-branch-name").first()

  const idBox = await worktreeId.boundingBox()
  const branchBox = await branchName.boundingBox()
  expect(idBox, "worktree short id should be visible").not.toBeNull()
  expect(branchBox, "worktree branch name should be visible").not.toBeNull()
  expect((branchBox?.y ?? 0) < (idBox?.y ?? 0)).toBeTruthy()
})

test("new tab is appended to the end", async ({ page }) => {
  await ensureWorkspace(page)

  await page.getByTitle("New tab").click()
  await page.getByText(/^Thread 2$/).waitFor({ timeout: 20_000 })

  const tabs = page.locator('[data-testid="thread-tab-title"]')
  const lastText = await tabs.last().textContent()
  expect(lastText?.trim()).toBe("Thread 2")
})
