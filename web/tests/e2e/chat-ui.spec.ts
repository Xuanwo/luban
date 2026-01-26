import { expect, test } from "@playwright/test"
import { activeWorkspaceId, ensureWorkspace, sendWsAction } from "./helpers"

async function activeThreadId(page: import("@playwright/test").Page, workspaceId: number): Promise<number> {
  const res = await page.request.get(`/api/workspaces/${workspaceId}/threads`)
  expect(res.ok()).toBeTruthy()
  const snapshot = (await res.json()) as { tabs: { active_tab: number } }
  return Number(snapshot.tabs.active_tab)
}

async function clickThreadTab(page: import("@playwright/test").Page, threadId: number) {
  const tab = page.getByTestId(`thread-tab-${threadId}`)
  await tab.scrollIntoViewIfNeeded()
  await tab.click()
}

async function createThreadViaUi(page: import("@playwright/test").Page, workspaceId: number): Promise<number> {
  const before = await activeThreadId(page, workspaceId)
  await page.getByTitle("New tab").click()
  await expect
    .poll(async () => await activeThreadId(page, workspaceId), { timeout: 30_000 })
    .not.toBe(before)
  return await activeThreadId(page, workspaceId)
}

async function queuedPromptTexts(page: import("@playwright/test").Page, workspaceId: number, threadId: number): Promise<string[]> {
  const res = await page.request.get(`/api/workspaces/${workspaceId}/conversations/${threadId}`)
  if (!res.ok()) return []
  const snapshot = (await res.json()) as { pending_prompts?: { text: string }[] }
  if (!Array.isArray(snapshot.pending_prompts)) return []
  return snapshot.pending_prompts.map((p) => String(p.text))
}

async function queuedPrompts(page: import("@playwright/test").Page, workspaceId: number, threadId: number): Promise<{ id: number; text: string }[]> {
  const res = await page.request.get(`/api/workspaces/${workspaceId}/conversations/${threadId}`)
  if (!res.ok()) return []
  const snapshot = (await res.json()) as { pending_prompts?: { id: number; text: string }[] }
  if (!Array.isArray(snapshot.pending_prompts)) return []
  return snapshot.pending_prompts.map((p) => ({ id: Number(p.id), text: String(p.text) }))
}

test("long tokens wrap without horizontal overflow", async ({ page }) => {
  await ensureWorkspace(page)

  const runId = Math.random().toString(16).slice(2)
  const marker = `e2e-${runId}-`
  const longToken = `${marker}${"a".repeat(600)}`
  await page.getByTestId("chat-input").fill(longToken)
  await expect(page.getByTestId("chat-send")).toBeEnabled({ timeout: 20_000 })
  await page.getByTestId("chat-send").click()

  const bubble = page.getByTestId("user-message-bubble").filter({ hasText: marker }).first()
  await expect(bubble).toBeVisible({ timeout: 20_000 })

  const overflow = await bubble.evaluate((el) => {
    const e = el as HTMLElement
    return e.scrollWidth - e.clientWidth
  })
  expect(overflow).toBeLessThanOrEqual(1)
})

test("pressing enter submits a user message", async ({ page }) => {
  await ensureWorkspace(page)

  const runId = Math.random().toString(16).slice(2)
  const marker = `e2e-enter-${runId}`

  await page.getByTestId("chat-input").fill(marker)
  await page.getByTestId("chat-input").press("Enter")

  const bubble = page.getByTestId("user-message-bubble").filter({ hasText: marker }).first()
  await expect(bubble).toBeVisible({ timeout: 20_000 })
  await expect(page.getByTestId("chat-input")).toHaveValue("")
})

test("user messages can be copied", async ({ page }) => {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"])
  await ensureWorkspace(page)

  const runId = Math.random().toString(16).slice(2)
  const marker = `e2e-copy-${runId}`

  await page.getByTestId("chat-input").fill(marker)
  await page.getByTestId("chat-input").press("Enter")

  const bubble = page.getByTestId("user-message-bubble").filter({ hasText: marker }).first()
  await expect(bubble).toBeVisible({ timeout: 20_000 })

  await bubble.hover()
  await bubble.getByTestId("user-message-copy").click()

  await expect
    .poll(async () => await page.evaluate(() => navigator.clipboard.readText()), { timeout: 5_000 })
    .toBe(marker)
})

test("selected text can be copied while an agent turn is running", async ({ page }) => {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"])
  await ensureWorkspace(page)

  const runId = Math.random().toString(16).slice(2)
  const marker = `e2e-select-copy-${runId}`
  const selection = `sel-${runId}`
  const prompt = `${marker} e2e-running-card ${selection}`

  const workspaceId = await activeWorkspaceId(page)
  const threadId = await createThreadViaUi(page, workspaceId)

  await page.getByTestId("chat-input").fill(prompt)
  await page.getByTestId("chat-input").press("Enter")

  const bubble = page.getByTestId("user-message-bubble").filter({ hasText: marker }).first()
  await expect(bubble).toBeVisible({ timeout: 20_000 })

  await expect(page.getByText("echo 1")).toBeVisible({ timeout: 20_000 })

  await bubble.evaluate(
    (el, needle) => {
      const root = el as HTMLElement
      const text = root.textContent ?? ""
      const startIndex = text.indexOf(needle)
      if (startIndex < 0) throw new Error(`missing selection text: ${needle}`)

      const endIndex = startIndex + needle.length

      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
      let cursor = 0
      let startNode: Text | null = null
      let startOffset = 0
      let endNode: Text | null = null
      let endOffset = 0

      while (walker.nextNode()) {
        const node = walker.currentNode as Text
        const len = node.nodeValue?.length ?? 0
        const nextCursor = cursor + len

        if (!startNode && startIndex >= cursor && startIndex <= nextCursor) {
          startNode = node
          startOffset = startIndex - cursor
        }
        if (!endNode && endIndex >= cursor && endIndex <= nextCursor) {
          endNode = node
          endOffset = endIndex - cursor
        }

        if (startNode && endNode) break
        cursor = nextCursor
      }

      if (!startNode || !endNode) throw new Error("failed to locate selection range nodes")

      const range = document.createRange()
      range.setStart(startNode, startOffset)
      range.setEnd(endNode, endOffset)

      const sel = window.getSelection()
      if (!sel) throw new Error("missing selection")
      sel.removeAllRanges()
      sel.addRange(range)
      document.dispatchEvent(new Event("selectionchange"))
    },
    selection,
  )

  await expect
    .poll(
      async () => {
        const res = await page.request.get(`/api/workspaces/${workspaceId}/conversations/${threadId}`)
        if (!res.ok()) return false
        const snapshot = (await res.json()) as any
        const inProgress = Array.isArray(snapshot?.in_progress_items) ? snapshot.in_progress_items : []
        const entries = Array.isArray(snapshot?.entries) ? snapshot.entries : []

        const hasCommand = (items: any[]): boolean =>
          items.some(
            (item: any) =>
              item?.kind === "command_execution" && String(item?.payload?.command ?? "").includes("echo 2"),
          )

        const hasEntry =
          entries.some(
            (entry: any) =>
              entry?.type === "agent_item" &&
              entry?.kind === "command_execution" &&
              String(entry?.payload?.command ?? "").includes("echo 2"),
          ) || hasCommand(inProgress)

        return Boolean(hasEntry)
      },
      { timeout: 20_000 },
    )
    .toBeTruthy()

  await expect
    .poll(async () => await page.evaluate(() => window.getSelection()?.toString() ?? ""), { timeout: 5_000 })
    .toBe(selection)

  const copyCombo = process.platform === "darwin" ? "Meta+C" : "Control+C"
  await page.keyboard.press(copyCombo)

  await expect
    .poll(async () => await page.evaluate(() => navigator.clipboard.readText()), { timeout: 5_000 })
    .toBe(selection)

  await page.evaluate(() => {
    window.getSelection()?.removeAllRanges()
    document.dispatchEvent(new Event("selectionchange"))
  })
})

test("selected text can be copied from a streaming assistant message", async ({ page }) => {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"])
  await ensureWorkspace(page)

  const needle = "e2e-selection-needle"
  const runId = Math.random().toString(16).slice(2)
  const prompt = `e2e-streaming-message e2e-streaming-${runId}`

  const workspaceId = await activeWorkspaceId(page)
  const threadId = await createThreadViaUi(page, workspaceId)

  await page.getByTestId("chat-input").fill(prompt)
  await page.getByTestId("chat-input").press("Enter")

  const assistant = page.getByTestId("assistant-message").filter({ hasText: needle }).first()
  await expect(assistant).toBeVisible({ timeout: 20_000 })

  await assistant.evaluate(
    (el, needle) => {
      const root = el as HTMLElement
      const text = root.textContent ?? ""
      const startIndex = text.indexOf(needle)
      if (startIndex < 0) throw new Error(`missing selection text: ${needle}`)
      const endIndex = startIndex + needle.length

      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
      let cursor = 0
      let startNode: Text | null = null
      let startOffset = 0
      let endNode: Text | null = null
      let endOffset = 0

      while (walker.nextNode()) {
        const node = walker.currentNode as Text
        const len = node.nodeValue?.length ?? 0
        const nextCursor = cursor + len

        if (!startNode && startIndex >= cursor && startIndex <= nextCursor) {
          startNode = node
          startOffset = startIndex - cursor
        }
        if (!endNode && endIndex >= cursor && endIndex <= nextCursor) {
          endNode = node
          endOffset = endIndex - cursor
        }

        if (startNode && endNode) break
        cursor = nextCursor
      }

      if (!startNode || !endNode) throw new Error("failed to locate selection range nodes")

      const range = document.createRange()
      range.setStart(startNode, startOffset)
      range.setEnd(endNode, endOffset)

      const sel = window.getSelection()
      if (!sel) throw new Error("missing selection")
      sel.removeAllRanges()
      sel.addRange(range)
      document.dispatchEvent(new Event("selectionchange"))
    },
    needle,
  )

  await expect
    .poll(
      async () => {
        const res = await page.request.get(`/api/workspaces/${workspaceId}/conversations/${threadId}`)
        if (!res.ok()) return ""
        const snapshot = (await res.json()) as any
        const items = Array.isArray(snapshot?.in_progress_items) ? snapshot.in_progress_items : []
        const agentMessage = items.find((item: any) => item?.kind === "agent_message") ?? null
        return String(agentMessage?.payload?.text ?? "")
      },
      { timeout: 20_000 },
    )
    .toContain("chunk-10")

  await expect
    .poll(async () => await page.evaluate(() => window.getSelection()?.toString() ?? ""), { timeout: 5_000 })
    .toBe(needle)

  const copyCombo = process.platform === "darwin" ? "Meta+C" : "Control+C"
  await page.keyboard.press(copyCombo)

  await expect
    .poll(async () => await page.evaluate(() => navigator.clipboard.readText()), { timeout: 5_000 })
    .toBe(needle)

  await page.evaluate(() => {
    window.getSelection()?.removeAllRanges()
    document.dispatchEvent(new Event("selectionchange"))
  })
})

test("chat input scrolls when content exceeds max height", async ({ page }) => {
  await ensureWorkspace(page)

  const payload = Array.from({ length: 40 }, (_, i) => `line ${i + 1}`).join("\n")
  const input = page.getByTestId("chat-input")
  await input.fill(payload)

  const metrics = await input.evaluate((el) => {
    const e = el as HTMLTextAreaElement
    e.scrollTop = 99999
    return {
      clientHeight: e.clientHeight,
      scrollHeight: e.scrollHeight,
      scrollTop: e.scrollTop,
      overflowY: getComputedStyle(e).overflowY,
    }
  })

  expect(metrics.scrollHeight).toBeGreaterThan(metrics.clientHeight)
  expect(["auto", "scroll"]).toContain(metrics.overflowY)
  expect(metrics.scrollTop).toBeGreaterThan(0)
})

test("enter commits IME composition without sending", async ({ page }) => {
  await ensureWorkspace(page)

  const runId = Math.random().toString(16).slice(2)
  const marker = `e2e-ime-${runId}`

  const input = page.getByTestId("chat-input")
  await input.fill(marker)

  await input.evaluate((el) => {
    el.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true, data: "x" }))
  })

  await input.press("Enter")
  await expect(page.getByTestId("user-message-bubble").filter({ hasText: marker })).toHaveCount(0)

  await input.evaluate((el) => {
    el.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "x" }))
  })
  await page.waitForTimeout(80)

  await input.press("Enter")
  await expect(page.getByTestId("user-message-bubble").filter({ hasText: marker }).first()).toBeVisible({ timeout: 20_000 })
})

test("scroll-to-bottom button appears only when away from bottom", async ({ page }) => {
  await ensureWorkspace(page)

  const runId = Math.random().toString(16).slice(2)
  const payload = Array.from({ length: 160 }, (_, i) => `line ${i + 1} ${runId}`).join("\n")
  await page.getByTestId("chat-input").fill(payload)
  await expect(page.getByTestId("chat-send")).toBeEnabled({ timeout: 20_000 })
  await page.getByTestId("chat-send").click()
  await expect(page.getByTestId("user-message-bubble").filter({ hasText: `line 160 ${runId}` }).first()).toBeVisible(
    { timeout: 20_000 },
  )

  const button = page.getByRole("button", { name: "Scroll to bottom" })
  await expect(button).toHaveCount(0)

  const scroller = page.getByTestId("chat-scroll-container")
  await scroller.hover()
  await page.mouse.wheel(0, -800)
  await expect(button).toBeVisible()

  await button.click()
  await expect(button).toHaveCount(0)

  await expect
    .poll(
      async () =>
        await scroller.evaluate((el) => {
          const e = el as HTMLElement
          const distance = e.scrollHeight - e.scrollTop - e.clientHeight
          return distance < 24
        }),
      { timeout: 5_000 },
    )
    .toBeTruthy()
})

test("external links open without navigating current page", async ({ page }) => {
  await ensureWorkspace(page)

  const startUrl = page.url()
  await page.evaluate(() => {
    const a = document.createElement("a")
    a.href = "https://example.com/"
    a.textContent = "example"
    a.setAttribute("data-testid", "external-link")
    document.body.appendChild(a)
  })

  const popupPromise = page.waitForEvent("popup", { timeout: 10_000 })
  await page.getByTestId("external-link").click()
  const popup = await popupPromise
  await popup.close()

  expect(page.url()).toBe(startUrl)
})

test("file attachments upload and render in user messages", async ({ page }) => {
  await ensureWorkspace(page)

  const runId = Math.random().toString(16).slice(2)
  const marker = `e2e-attach-${runId}`

  await page.getByTestId("chat-attach-input").setInputFiles({
    name: "notes.txt",
    mimeType: "text/plain",
    buffer: Buffer.from(`hello ${runId}\n`, "utf8"),
  })

  await page.getByTestId("chat-input").fill(marker)
  await expect(page.getByTestId("chat-send")).toBeEnabled({ timeout: 20_000 })
  await page.getByTestId("chat-send").click()

  const bubble = page.getByTestId("user-message-bubble").filter({ hasText: marker }).first()
  await expect(bubble).toBeVisible({ timeout: 20_000 })

  const attachment = bubble.getByTestId("user-message-attachment").first()
  await expect(attachment).toBeVisible({ timeout: 20_000 })
  await expect(attachment).toContainText(/notes-\d+\.txt/)

  await page.getByTestId("right-sidebar-tab-context").click()
  const row = page.getByTestId("context-file-row").filter({ hasText: "notes-" }).first()
  await expect(row).toBeVisible({ timeout: 20_000 })
  await row.hover()
  await row.getByTestId("context-add-to-chat").click({ force: true, timeout: 20_000 })
  await expect(page.getByTestId("chat-attachment-tile").first()).toBeVisible({ timeout: 20_000 })
})

test("queued messages can be reordered and edited", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await activeThreadId(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  let firstQueued = ""
  let secondQueued = ""
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const seed = `e2e-queued-${runId}-${attempt}`
    await sendWsAction(page, { type: "send_agent_message", workspace_id: workspaceId, thread_id: threadId, text: `${seed}-run`, attachments: [] })
    await sendWsAction(page, { type: "send_agent_message", workspace_id: workspaceId, thread_id: threadId, text: `${seed}-a`, attachments: [] })
    await sendWsAction(page, { type: "send_agent_message", workspace_id: workspaceId, thread_id: threadId, text: `${seed}-b`, attachments: [] })

    try {
      await expect
        .poll(async () => (await queuedPromptTexts(page, workspaceId, threadId)).slice(0, 2), { timeout: 10_000 })
        .toHaveLength(2)
      const queued = await queuedPromptTexts(page, workspaceId, threadId)
      ;[firstQueued, secondQueued] = queued.slice(0, 2)
      break
    } catch {
      // Retry until we have at least two queued prompts.
    }
  }
  expect(firstQueued.length).toBeGreaterThan(0)
  expect(secondQueued.length).toBeGreaterThan(0)

  await expect(page.getByTestId("queued-prompts")).toBeVisible({ timeout: 20_000 })

  const queuedItems = page.getByTestId("queued-prompt-item")
  await expect(queuedItems).toHaveCount(2, { timeout: 20_000 })
  await expect(queuedItems.nth(0)).toContainText(firstQueued)
  await expect(queuedItems.nth(1)).toContainText(secondQueued)

  const prompts = await queuedPrompts(page, workspaceId, threadId)
  expect(prompts.length).toBeGreaterThanOrEqual(2)
  const [firstPrompt, secondPrompt] = prompts
  expect(firstPrompt).toBeTruthy()
  expect(secondPrompt).toBeTruthy()

  await sendWsAction(page, {
    type: "reorder_queued_prompt",
    workspace_id: workspaceId,
    thread_id: threadId,
    active_id: secondPrompt.id,
    over_id: firstPrompt.id,
  })

  await expect(queuedItems.nth(0)).toContainText(secondQueued, { timeout: 20_000 })

  await queuedItems.nth(0).hover()
  await queuedItems.nth(0).locator('[data-testid="queued-prompt-edit"]').click({ force: true, timeout: 20_000 })
  const input = page.getByTestId("queued-prompt-input")
  await expect(input).toBeVisible({ timeout: 20_000 })
  await expect.poll(async () => await input.getAttribute("placeholder")).toBe("Edit message...")
  const updated = `e2e-queued-${runId}-edited`
  await input.fill(updated)
  await page.getByTestId("queued-save").click()

  await expect(queuedItems.nth(0)).toContainText(updated, { timeout: 20_000 })

  const lastItem = queuedItems.nth(1)
  await lastItem.hover()
  await lastItem.locator('[data-testid="queued-prompt-cancel"]').click({ force: true, timeout: 20_000 })
  await expect(queuedItems).toHaveCount(1, { timeout: 20_000 })
})

test("cancel -> submit turns the previous run into a cancelled activity stream and starts a new turn", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-cancel-submit-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-run`,
    attachments: [],
  })

  const cancelButton = page.getByTestId("agent-running-cancel")
  await expect(cancelButton).toBeVisible({ timeout: 20_000 })
  await cancelButton.click()

  const editor = page.getByTestId("agent-running-input")
  await expect(editor).toBeVisible({ timeout: 20_000 })

  const interrupt = `${seed}-interrupt`
  await editor.fill(interrupt)
  await page.getByTestId("agent-running-submit").click()

  await expect(page.getByTestId("agent-running-cancel")).toBeVisible({ timeout: 20_000 })
  await expect(page.getByTestId("user-message-bubble").filter({ hasText: interrupt }).first()).toBeVisible({ timeout: 20_000 })
  await expect(page.getByText("Cancelled after").first()).toBeVisible({ timeout: 20_000 })
  await expect(page.getByText(/^Completed \d+ steps$/).first()).toBeVisible({ timeout: 20_000 })
})

test("cancel -> escape pauses when queued prompts exist and shows resume", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-cancel-queued-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-run`,
    attachments: [],
  })
  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-queued`,
    attachments: [],
  })

  await expect
    .poll(async () => (await queuedPromptTexts(page, workspaceId, threadId)).length, { timeout: 10_000 })
    .toBeGreaterThan(0)

  const cancelButton = page.getByTestId("agent-running-cancel")
  await expect(cancelButton).toBeVisible({ timeout: 20_000 })
  await cancelButton.click()

  const editor = page.getByTestId("agent-running-input")
  await expect(editor).toBeVisible({ timeout: 20_000 })
  await editor.press("Escape")

  const resumeButton = page.getByTestId("agent-running-resume")
  await expect(resumeButton).toBeVisible({ timeout: 20_000 })
  await resumeButton.click()
  await expect(page.getByTestId("agent-running-input")).toBeVisible({ timeout: 20_000 })
})

test("cancel -> escape without queued prompts shows cancelled activity stream", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-cancel-no-queue-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-run`,
    attachments: [],
  })

  const cancelButton = page.getByTestId("agent-running-cancel")
  await expect(cancelButton).toBeVisible({ timeout: 20_000 })
  await cancelButton.click()

  const editor = page.getByTestId("agent-running-input")
  await expect(editor).toBeVisible({ timeout: 20_000 })
  await editor.press("Escape")

  await expect(page.getByText("Cancelled after").first()).toBeVisible({ timeout: 20_000 })
  await expect(page.getByTestId("agent-running-resume")).toHaveCount(0)
})

test("sending from chat input while paused queues instead of starting a new run", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-cancel-queue-input-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-run`,
    attachments: [],
  })
  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-queued`,
    attachments: [],
  })

  await expect
    .poll(async () => (await queuedPromptTexts(page, workspaceId, threadId)).length, { timeout: 10_000 })
    .toBeGreaterThan(0)

  await page.getByTestId("agent-running-cancel").click()
  await page.getByTestId("agent-running-input").press("Escape")

  const resume = page.getByTestId("agent-running-resume")
  await expect(resume).toBeVisible({ timeout: 20_000 })

  const queuedBefore = await queuedPromptTexts(page, workspaceId, threadId)
  expect(queuedBefore.length).toBeGreaterThan(0)

  const marker = `${seed}-chat`
  await page.getByTestId("chat-input").fill(marker)
  await page.getByTestId("chat-send").click()

  await expect
    .poll(async () => (await queuedPromptTexts(page, workspaceId, threadId)), { timeout: 10_000 })
    .toContain(marker)

  await expect(page.getByTestId("user-message-bubble").filter({ hasText: marker })).toHaveCount(0)
  await expect(resume).toBeVisible({ timeout: 20_000 })
})

test("agent running timer increments while running", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-cancel-timer-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-run`,
    attachments: [],
  })

  await expect(page.getByTestId("agent-running-cancel")).toBeVisible({ timeout: 20_000 })

  const timer = page.getByTestId("agent-running-timer")
  await expect(timer).toBeVisible({ timeout: 20_000 })

  await expect
    .poll(async () => (await timer.textContent())?.trim() ?? "", { timeout: 10_000 })
    .not.toBe("00:00")

  // Stop the running turn so it doesn't leak into other tests.
  await page.getByTestId("agent-running-cancel").click()
  await page.getByTestId("agent-running-input").press("Escape")
})

test("agent running timer survives switching tabs", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const firstThreadId = await activeThreadId(page, workspaceId)
  expect(firstThreadId).toBeGreaterThan(0)
  const secondThreadId = await createThreadViaUi(page, workspaceId)
  expect(secondThreadId).toBeGreaterThan(0)

  await clickThreadTab(page, firstThreadId)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-cancel-tab-switch-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: firstThreadId,
    text: `${seed}-run`,
    attachments: [],
  })

  await expect(page.getByTestId("agent-running-cancel")).toBeVisible({ timeout: 20_000 })

  const timer = page.getByTestId("agent-running-timer")
  await expect(timer).toBeVisible({ timeout: 20_000 })

  await expect
    .poll(async () => (await timer.textContent())?.trim() ?? "", { timeout: 10_000 })
    .not.toBe("00:00")
  const initial = (await timer.textContent())?.trim() ?? ""

  await clickThreadTab(page, secondThreadId)
  await expect(page.getByTestId("agent-running-timer")).toHaveCount(0, { timeout: 20_000 })

  await clickThreadTab(page, firstThreadId)
  await expect(timer).toBeVisible({ timeout: 20_000 })
  await expect(timer).not.toHaveText("00:00")
  await expect
    .poll(async () => (await timer.textContent())?.trim() ?? "", { timeout: 10_000 })
    .not.toBe(initial)

  // Cleanup: cancel this run so it doesn't leak into other tests.
  await page.getByTestId("agent-running-cancel").click()
  await page.getByTestId("agent-running-input").press("Escape")
})

test("double-Esc while agent is running opens cancel editor", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-double-esc-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-run`,
    attachments: [],
  })

  await expect(page.getByTestId("agent-running-cancel")).toBeVisible({ timeout: 20_000 })
  await expect(page.getByTestId("agent-running-input")).toHaveCount(0)

  await page.keyboard.press("Escape")
  await expect(page.getByTestId("esc-cancel-hint")).toBeVisible({ timeout: 20_000 })
  await expect(page.getByTestId("agent-running-input")).toHaveCount(0)

  await page.keyboard.press("Escape")
  await expect(page.getByTestId("agent-running-input")).toBeVisible({ timeout: 20_000 })

  // Cleanup: cancel this run so it doesn't leak into other tests.
  await page.getByTestId("agent-running-input").press("Escape")
})

test("Escape closes foreground modal before triggering chat cancel", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)
  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-esc-modal-priority-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-e2e-running-card`,
    attachments: [],
  })

  await expect(page.getByTestId("agent-running-cancel")).toBeVisible({ timeout: 20_000 })
  await expect(page.getByTestId("esc-cancel-hint")).toHaveCount(0)

  await page.getByTestId("left-sidebar").getByRole("button", { name: "New Task", exact: true }).click()
  await expect(page.getByTestId("new-task-modal")).toBeVisible({ timeout: 10_000 })

  await page.keyboard.press("Escape")

  await expect(page.getByTestId("new-task-modal")).toHaveCount(0, { timeout: 10_000 })
  await expect
    .poll(async () => await page.getByTestId("esc-cancel-hint").count(), { timeout: 1000 })
    .toBe(0)

  // Cleanup: cancel this run so it doesn't leak into other tests.
  await page.getByTestId("agent-running-cancel").click()
  await page.getByTestId("agent-running-input").press("Escape")
})

test("feedback modal supports two-stage polish flow", async ({ page }) => {
  await ensureWorkspace(page)

  await page.getByTestId("sidebar-open-feedback").click()
  const modal = page.getByTestId("feedback-modal")
  await expect(modal).toBeVisible({ timeout: 10_000 })

  await modal.getByPlaceholder("Describe the bug or feature you'd like...").fill("The sidebar status is stuck.")
  await modal.getByRole("button", { name: "Polish", exact: true }).click()

  await expect(modal.getByRole("heading", { name: "Review Issue" })).toBeVisible({ timeout: 10_000 })
  await expect(modal.getByPlaceholder("Issue title...")).toBeVisible()
  await expect(modal.getByPlaceholder("Issue body...")).toBeVisible()

  await page.keyboard.press("Escape")
  await expect(modal).toHaveCount(0, { timeout: 10_000 })
})

test("esc cancel hint auto-hides when countdown ends", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-esc-hint-timeout-${runId}`

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-e2e-running-card`,
    attachments: [],
  })

  await expect(page.getByTestId("agent-running-cancel")).toBeVisible({ timeout: 20_000 })

  await page.keyboard.press("Escape")
  await expect(page.getByTestId("esc-cancel-hint")).toBeVisible({ timeout: 20_000 })

  await expect
    .poll(async () => await page.getByTestId("esc-cancel-hint").count(), { timeout: 10_000 })
    .toBe(0)

  // Cleanup: cancel this run so it doesn't leak into other tests.
  await page.getByTestId("agent-running-cancel").click()
  await page.getByTestId("agent-running-input").press("Escape")
})

test("expanded agent running card keeps header anchored as activities grow", async ({ page }) => {
  await ensureWorkspace(page)

  const workspaceId = await activeWorkspaceId(page)

  const threadId = await createThreadViaUi(page, workspaceId)
  expect(threadId).toBeGreaterThan(0)

  const runId = Math.random().toString(16).slice(2)
  const seed = `e2e-running-card-${runId}`

  // Ensure the chat view can actually scroll; otherwise the header can't be kept anchored when the
  // expanded activity list grows.
  const filler = Array.from({ length: 120 }, (_, i) => `filler ${i + 1} ${runId}`).join("\n")
  await page.getByTestId("chat-input").fill(filler)
  await page.getByTestId("chat-send").click()
  await expect(page.getByTestId("user-message-bubble").filter({ hasText: `filler 120 ${runId}` }).first()).toBeVisible({
    timeout: 20_000,
  })
  // Avoid racing the filler run; ensure the conversation is idle before starting the main run.
  await expect
    .poll(
      async () => {
        const res = await page.request.get(`/api/workspaces/${workspaceId}/conversations/${threadId}`)
        if (!res.ok()) return "unknown"
        const snapshot = (await res.json()) as { run_status?: string }
        return String(snapshot.run_status ?? "unknown")
      },
      { timeout: 20_000 },
    )
    .toBe("idle")

  await sendWsAction(page, {
    type: "send_agent_message",
    workspace_id: workspaceId,
    thread_id: threadId,
    text: `${seed}-run`,
    attachments: [],
  })

  await expect(page.getByTestId("agent-running-cancel")).toBeVisible({ timeout: 20_000 })

  const header = page.getByTestId("agent-running-header")
  await expect(header).toBeVisible({ timeout: 20_000 })

  // Ensure we have at least one completed activity before expanding, otherwise the expanded view
  // doesn't render the history section above the header.
  await expect(header).toContainText("echo 2", { timeout: 20_000 })

  await header.click()
  const history = page.getByTestId("agent-running-history")
  await expect(history.getByText("echo 1")).toBeVisible({ timeout: 20_000 })

  const y0 = (await header.boundingBox())?.y ?? 0
  expect(y0).toBeGreaterThan(0)

  // Wait for another activity to become "history" and verify the header stays anchored.
  await expect(history.getByText("echo 2")).toBeVisible({ timeout: 20_000 })
  const y1 = (await header.boundingBox())?.y ?? 0
  expect(Math.abs(y1 - y0)).toBeLessThanOrEqual(2)

  // Cleanup: cancel this run so it doesn't leak into other tests.
  await page.getByTestId("agent-running-cancel").click()
  await page.getByTestId("agent-running-input").press("Escape")
})

test("/command autocompletes Codex custom prompts", async ({ page }) => {
  await ensureWorkspace(page)

  const input = page.getByTestId("chat-input")
  await input.fill("/rev")

  const menu = page.getByTestId("chat-command-menu")
  await expect(menu).toBeVisible({ timeout: 20_000 })

  const item = page.getByTestId("chat-command-item").filter({ hasText: "review" }).first()
  await expect(item).toBeVisible({ timeout: 20_000 })
  await item.click()

  await expect.poll(async () => await input.inputValue(), { timeout: 10_000 }).toContain("Review a change locally.")
  await expect(menu).toHaveCount(0)
})

test("@mention autocompletes workspace files", async ({ page }) => {
  await ensureWorkspace(page)

  const input = page.getByTestId("chat-input")
  await input.fill("@rdm")

  const menu = page.getByTestId("chat-mention-menu")
  await expect(menu).toBeVisible({ timeout: 20_000 })

  const docsItem = page.getByTestId("chat-mention-item").filter({ hasText: "docs/README.md" }).first()
  await expect(docsItem).toBeVisible({ timeout: 20_000 })
  await docsItem.hover()
  await input.press("Enter")

  await expect.poll(async () => await input.inputValue(), { timeout: 10_000 }).toBe("@docs/README.md ")
  await expect(menu).toHaveCount(0)
})
