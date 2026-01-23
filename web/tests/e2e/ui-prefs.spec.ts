import { expect, test } from "@playwright/test"
import { activeWorkspaceId, ensureWorkspace, fetchAppSnapshot } from "./helpers"

async function serverActiveThreadId(
  page: import("@playwright/test").Page,
  workspaceId: number,
): Promise<number> {
  const res = await page.request.get(`/api/workspaces/${workspaceId}/threads`)
  expect(res.ok()).toBeTruthy()
  const snapshot = (await res.json()) as { tabs: { active_tab: number } }
  return Number(snapshot.tabs.active_tab)
}

test("restores last active tab per workspace on reload", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const before = await serverActiveThreadId(page, workspaceId)
  await page.getByTitle("New tab").click()

  await expect.poll(async () => await serverActiveThreadId(page, workspaceId), { timeout: 30_000 }).not.toBe(before)
  const createdThreadId = await serverActiveThreadId(page, workspaceId)

  await page.reload()
  await page.getByTestId("chat-input").waitFor({ state: "visible", timeout: 60_000 })

  await expect.poll(async () => await serverActiveThreadId(page, workspaceId), { timeout: 30_000 }).toBe(createdThreadId)
  await expect
    .poll(async () => Number((await fetchAppSnapshot(page))?.ui?.active_thread_id ?? 0), { timeout: 30_000 })
    .toBe(createdThreadId)
})

test("remembers the last used open button selection", async ({ page }) => {
  await ensureWorkspace(page)

  await page.getByTestId("open-button-menu").click()
  await page.getByTestId("open-button-item-copy-path").click()
  await expect(page.getByTestId("open-button-primary")).toHaveText(/Copy Path/, { timeout: 10_000 })
  await expect
    .poll(async () => String((await fetchAppSnapshot(page))?.ui?.open_button_selection ?? ""), { timeout: 10_000 })
    .toContain("copy-path")

  await page.reload()
  await page.getByTestId("open-button-primary").waitFor({ timeout: 30_000 })
  await expect(page.getByTestId("open-button-primary")).toHaveText(/Copy Path/)
})
