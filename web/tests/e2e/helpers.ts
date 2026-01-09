import type { Page } from "@playwright/test"

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

  const projectDir = process.env.LUBAN_E2E_PROJECT_DIR
  if (!projectDir) throw new Error("LUBAN_E2E_PROJECT_DIR is not set")

  await sendWsAction(page, { type: "add_project", path: projectDir })

  await page.getByText("e2e-project", { exact: true }).waitFor({ timeout: 15_000 })

  await page.getByText("e2e-project", { exact: true }).click()
  await page.getByTitle("Add worktree").click()

  const branch = page.getByTestId("worktree-branch-name").first()
  await branch.waitFor({ timeout: 90_000 })
  await branch.click()

  await page.getByText(/^Thread 1$/).waitFor({ timeout: 20_000 })
}
