import { expect, test } from "@playwright/test"
import { activeWorkspaceId, ensureWorkspace, sendWsAction } from "./helpers"

async function activeThreadId(page: import("@playwright/test").Page, workspaceId: number): Promise<number> {
  const res = await page.request.get(`/api/workspaces/${workspaceId}/threads`)
  expect(res.ok()).toBeTruthy()
  const snapshot = (await res.json()) as { tabs: { active_tab?: number } }
  return Number(snapshot.tabs.active_tab ?? NaN)
}

test("switching between long threads keeps the viewport pinned to bottom", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const tabTitles = page.getByTestId("thread-tab-title")
  const beforeTabs = await tabTitles.count()
  expect(beforeTabs).toBeGreaterThanOrEqual(1)

  // Create two fresh threads via WebSocket actions so we don't depend on UI input behavior.
  await sendWsAction(page, { type: "create_workspace_thread", workspace_id: workspaceId })
  await expect(tabTitles).toHaveCount(beforeTabs + 1, { timeout: 20_000 })
  const threadB = await activeThreadId(page, workspaceId)
  expect(Number.isFinite(threadB) && threadB > 0).toBeTruthy()

  await sendWsAction(page, { type: "create_workspace_thread", workspace_id: workspaceId })
  await expect(tabTitles).toHaveCount(beforeTabs + 2, { timeout: 20_000 })
  const threadA = await activeThreadId(page, workspaceId)
  expect(Number.isFinite(threadA) && threadA > 0).toBeTruthy()

  const seed = async (threadId: number) => {
    const marker = `e2e-pagination-steps-${threadId}-${Math.random().toString(16).slice(2)}`

    await sendWsAction(page, {
      type: "send_agent_message",
      workspace_id: workspaceId,
      thread_id: threadId,
      text: marker,
      attachments: [],
    })

    // Wait for the fake agent to finish emitting all items. Avoid racing a second large run
    // while the engine command queue is still busy.
    await expect
      .poll(async () => {
        const res = await page.request.get(`/api/workspaces/${workspaceId}/conversations/${threadId}?limit=1`)
        if (!res.ok()) return { entries_total: 0, run_status: "idle", queue_paused: false }
        const snap = (await res.json()) as { entries_total?: number; run_status?: string; queue_paused?: boolean }
        return {
          entries_total: Number(snap.entries_total ?? 0),
          run_status: String(snap.run_status ?? "idle"),
          queue_paused: Boolean(snap.queue_paused ?? false),
        }
      }, { timeout: 90_000 })
      .toMatchObject({ run_status: "idle", queue_paused: true })

    await expect
      .poll(async () => {
        const res = await page.request.get(`/api/workspaces/${workspaceId}/conversations/${threadId}?limit=1`)
        if (!res.ok()) return 0
        const snap = (await res.json()) as { entries_total?: number }
        return Number(snap.entries_total ?? 0)
      }, { timeout: 90_000 })
      .toBeGreaterThan(2000)
  }

  // Seed sequentially to avoid interleaving multiple large fake-agent floods.
  await seed(threadA)
  await seed(threadB)

  const measure = async () => {
    const result = await page.evaluate(async () => {
      const el = document.querySelector('[data-testid="chat-scroll-container"]') as HTMLElement | null
      if (!el) return null
      const start = performance.now()
      let maxDistanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight
      while (performance.now() - start < 800) {
        maxDistanceToBottom = Math.max(maxDistanceToBottom, el.scrollHeight - el.scrollTop - el.clientHeight)
        await new Promise(requestAnimationFrame)
      }
      return { endDistanceToBottom: el.scrollHeight - el.scrollTop - el.clientHeight, maxDistanceToBottom }
    })
    if (!result) throw new Error("missing chat scroll container")
    return result
  }

  const tabAIndex = beforeTabs + 1
  const tabBIndex = beforeTabs

  await tabTitles.nth(tabAIndex).locator("..").click()
  const m1 = await measure()
  expect(m1.endDistanceToBottom).toBeLessThanOrEqual(80)
  expect(m1.maxDistanceToBottom).toBeLessThanOrEqual(1000)

  await tabTitles.nth(tabBIndex).locator("..").click()
  const m2 = await measure()
  expect(m2.endDistanceToBottom).toBeLessThanOrEqual(80)
  expect(m2.maxDistanceToBottom).toBeLessThanOrEqual(1000)

  void threadA
  void threadB
})

test("loads older conversation entries when scrolling to top", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const tabTitles = page.getByTestId("thread-tab-title")
  const beforeTabs = await tabTitles.count()
  const existingTab = tabTitles.first().locator("..")
  await page.getByTitle("New tab").click()
  await expect(tabTitles).toHaveCount(beforeTabs + 1, { timeout: 20_000 })
  const newTab = tabTitles.last().locator("..")
  await newTab.scrollIntoViewIfNeeded()
  await newTab.click()

  const threadsRes = await page.request.get(`/api/workspaces/${workspaceId}/threads`)
  expect(threadsRes.ok()).toBeTruthy()
  const threads = (await threadsRes.json()) as { tabs: { active_tab: number } }
  const threadId = Number(threads.tabs.active_tab)
  expect(Number.isFinite(threadId) && threadId > 0).toBeTruthy()
  const ids = { workspaceId, threadId }

  const runId = Math.random().toString(16).slice(2)
  const marker = `e2e-pagination-steps-${runId}`

  await page.getByTestId("chat-input").fill(marker)
  await page.getByTestId("chat-send").click()

  await expect
    .poll(async () => {
      const res = await page.request.get(
        `/api/workspaces/${ids.workspaceId}/conversations/${ids.threadId}?limit=1`,
      )
      if (!res.ok()) return { run_status: "idle", queue_paused: false }
      const snap = (await res.json()) as { run_status?: string; queue_paused?: boolean }
      return {
        run_status: String(snap.run_status ?? "idle"),
        queue_paused: Boolean(snap.queue_paused ?? false),
      }
    }, { timeout: 90_000 })
    .toMatchObject({ run_status: "idle", queue_paused: true })

  await expect
    .poll(async () => {
      const res = await page.request.get(
        `/api/workspaces/${ids.workspaceId}/conversations/${ids.threadId}?limit=1`,
      )
      if (!res.ok()) return 0
      const snap = (await res.json()) as { entries_total?: number }
      return snap.entries_total ?? 0
    }, { timeout: 60_000 })
    .toBeGreaterThan(2000)

  // Force the UI to refresh the conversation via HTTP so the client-side state has
  // `entries_start` populated before we attempt to load older pages.
  const container = page.getByTestId("chat-scroll-container")
  await expect(container).toBeVisible()

  const refreshResponse = page.waitForResponse((res) => {
    if (res.request().method() !== "GET") return false
    const url = res.url()
    if (!url.includes(`/api/workspaces/${ids.workspaceId}/conversations/${ids.threadId}`)) return false
    try {
      const parsed = new URL(url)
      return parsed.searchParams.get("limit") === "2000" && !parsed.searchParams.has("before") && res.status() === 200
    } catch {
      return false
    }
  })

  await existingTab.scrollIntoViewIfNeeded()
  await existingTab.click()
  await newTab.scrollIntoViewIfNeeded()
  await newTab.click()

  const entriesStart = await refreshResponse.then(async (res) => {
    const snap = (await res.json()) as { entries_start?: number }
    return snap.entries_start ?? 0
  })
  expect(entriesStart).toBeGreaterThan(0)

  const beforeRequest = page.waitForRequest((req) => {
    if (req.method() !== "GET") return false
    const url = req.url()
    if (!url.includes(`/api/workspaces/${ids.workspaceId}/conversations/${ids.threadId}`)) return false
    try {
      const parsed = new URL(url)
      return parsed.searchParams.get("limit") === "2000" && parsed.searchParams.has("before")
    } catch {
      return false
    }
  })

  await container.hover()
  await container.evaluate((el) => {
    el.scrollTop = 0
  })
  await page.mouse.wheel(0, -2000)

  const req = await beforeRequest
  const before = Number(new URL(req.url()).searchParams.get("before") ?? NaN)
  expect(Number.isFinite(before) && before > 0).toBeTruthy()

  await expect(page.getByTestId("chat-input")).toBeVisible()
  await expect(page.getByText("Application error:", { exact: false })).toHaveCount(0)
})
