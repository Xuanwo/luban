import type { Page } from "@playwright/test"
import fs from "node:fs"
import { requireEnv } from "./env"

export async function fetchAppSnapshot(page: Page): Promise<any> {
  const res = await page.request.get("/api/app")
  if (!res.ok()) throw new Error(`app snapshot fetch failed: ${res.status()}`)
  return await res.json()
}

export async function activeWorkspaceId(page: Page, opts?: { timeoutMs?: number }): Promise<number> {
  const timeoutMs = opts?.timeoutMs ?? 15_000
  const startedAt = Date.now()
  while (Date.now() - startedAt < timeoutMs) {
    try {
      const snap = await fetchAppSnapshot(page)
      const id = Number(snap?.ui?.active_workspace_id ?? NaN)
      if (Number.isFinite(id) && id > 0) return id
    } catch {
      // retry
    }
    await page.waitForTimeout(100)
  }
  throw new Error("missing active workspace id")
}

export async function sendWsAction<TAction extends Record<string, unknown>>(
  page: Page,
  action: TAction,
): Promise<{ rev: number }> {
  return await page.evaluate(async (action) => {
    const url = new URL("/api/events", window.location.href)
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:"

    const ws = new WebSocket(url.toString())
    await new Promise<void>((resolve, reject) => {
      ws.onopen = () => resolve()
      ws.onerror = () => reject(new Error("ws open failed"))
    })

    const requestId = `e2e-${Math.random().toString(16).slice(2)}`
    ws.send(JSON.stringify({ type: "action", request_id: requestId, action }))

    const rev = await new Promise<number>((resolve, reject) => {
      const timer = window.setTimeout(() => reject(new Error("ws ack timeout")), 10_000)
      ws.onmessage = (ev) => {
        const msg = JSON.parse(String(ev.data))
        if (msg.type === "ack" && msg.request_id === requestId) {
          window.clearTimeout(timer)
          resolve(Number(msg.rev))
        }
        if (msg.type === "error" && msg.request_id === requestId) {
          window.clearTimeout(timer)
          reject(new Error(String(msg.message)))
        }
      }
    })

    ws.close()
    return { rev }
  }, action)
}

export async function ensureWorkspace(page: Page) {
  await page.goto("/")

  const projectDir = fs.realpathSync(requireEnv("LUBAN_E2E_PROJECT_DIR"))

  await sendWsAction(page, { type: "add_project", path: projectDir })

  const projectToggle = page.getByTitle(projectDir, { exact: true }).locator("..")
  await projectToggle.waitFor({ timeout: 15_000 })
  const projectContainer = projectToggle.locator("..").locator("..")

  // Ensure the project is expanded. Avoid toggling it closed if a parallel test already expanded it.
  if ((await projectContainer.getByTestId("worktree-branch-name").count()) === 0) {
    await projectToggle.click()
  }

  const branches = projectContainer.getByTestId("worktree-branch-name")
  if ((await branches.count()) === 0) {
    await projectContainer.hover()
    const addWorktree = projectContainer.getByTitle("Add worktree")
    await addWorktree.waitFor({ state: "visible", timeout: 10_000 })
    if (!(await addWorktree.isDisabled())) {
      await addWorktree.click()
    }
    await branches.first().waitFor({ timeout: 90_000 })
  }
  await branches.first().click()

  const firstTabTitle = page.getByTestId("thread-tab-title").first()
  await firstTabTitle.waitFor({ state: "attached", timeout: 60_000 })

  const firstTab = firstTabTitle.locator("..")
  await firstTab.scrollIntoViewIfNeeded()
  await firstTab.click()
}
