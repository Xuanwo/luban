import { sleep } from "../lib/utils.mjs"

async function inboxRowTaskTitle(row) {
  const title = row.getByTestId("inbox-notification-task-title").first()
  await title.waitFor({ state: "visible" })
  return ((await title.textContent()) ?? "").trim()
}

async function findInboxRowIndexByTitle({ page, title }) {
  const rows = page.locator('[data-testid^="inbox-notification-row-"]')
  const count = await rows.count()
  for (let i = 0; i < count; i += 1) {
    const row = rows.nth(i)
    const text = await inboxRowTaskTitle(row)
    if (text === title) return i
  }
  return -1
}

async function waitForInboxTitlePresence({ page, title, shouldExist, timeoutMs = 20_000 }) {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    const idx = await findInboxRowIndexByTitle({ page, title })
    if (shouldExist ? idx !== -1 : idx === -1) return
    await sleep(200)
  }
  const idx = await findInboxRowIndexByTitle({ page, title })
  throw new Error(
    `timeout waiting for inbox title presence; title=${JSON.stringify(title)} shouldExist=${shouldExist} idx=${idx}`
  )
}

async function waitForInboxRows({ page, timeoutMs = 20_000 }) {
  const rows = page.locator('[data-testid^="inbox-notification-row-"]')
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    if ((await rows.count()) > 0) return
    await sleep(200)
  }
  throw new Error("timeout waiting for inbox rows")
}

async function closeFilterMenu({ page }) {
  const menu = page.getByTestId("inbox-filter-menu")
  for (let i = 0; i < 3; i += 1) {
    if ((await menu.count()) === 0) return
    await page.keyboard.press("Escape")
    await sleep(50)
  }
  await menu.waitFor({ state: "hidden" })
}

async function waitForFilterBarHidden({ page, timeoutMs = 10_000 }) {
  const bar = page.getByTestId("inbox-filter-bar")
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    if ((await bar.count()) === 0) return
    await sleep(100)
  }
  throw new Error("timeout waiting for inbox filter bar to hide")
}

export async function runInboxSavedViews({ page }) {
  await page.evaluate(() => {
    localStorage.removeItem("luban:ui:inbox_views")
    localStorage.removeItem("luban:ui:inbox_filters")
    localStorage.removeItem("luban:ui:inbox_active_view")
  })

  await page.getByTestId("nav-inbox-button").click()
  await page.getByTestId("inbox-view").waitFor({ state: "visible" })
  await waitForInboxRows({ page })

  // Default hides done tasks.
  await waitForInboxTitlePresence({ page, title: "Done: completed successfully", shouldExist: false })

  // Filter to the local project.
  await page.getByTestId("inbox-filter-button").click()
  await page.getByTestId("inbox-filter-menu").waitFor({ state: "visible" })
  await page.getByTestId("inbox-filter-sub-project").hover()
  await page.getByTestId("inbox-filter-project-mock-project-2").click()
  await closeFilterMenu({ page })

  await waitForInboxTitlePresence({ page, title: "Local task", shouldExist: true })
  await waitForInboxTitlePresence({ page, title: "Mock task 1", shouldExist: false })

  // Include done items.
  await page.getByTestId("inbox-filter-button").click()
  await page.getByTestId("inbox-filter-menu").waitFor({ state: "visible" })
  await page.getByTestId("inbox-filter-sub-status").hover()
  await page.getByTestId("inbox-filter-status-done").click()
  await closeFilterMenu({ page })

  await waitForInboxTitlePresence({ page, title: "Local: done", shouldExist: true })

  // Save a view; it should enter rename mode with focus.
  await page.getByTestId("inbox-save-view-button").click()

  const renameInput = page.getByTestId("inbox-view-item-0-rename-input")
  await renameInput.waitFor({ state: "visible" })

  const initialName = await renameInput.inputValue()
  if (initialName !== "View 1") {
    throw new Error(`expected default view name to be "View 1", got: ${JSON.stringify(initialName)}`)
  }

  const focused = await renameInput.evaluate(el => el === document.activeElement)
  if (!focused) {
    throw new Error("expected newly created view rename input to be focused")
  }

  await renameInput.fill("Local Done")
  await renameInput.press("Enter")

  const label = page.getByTestId("inbox-view-item-0-label")
  await label.waitFor({ state: "visible" })
  const savedName = ((await label.textContent()) ?? "").trim()
  if (savedName !== "Local Done") {
    throw new Error(`expected saved view label to be "Local Done", got: ${JSON.stringify(savedName)}`)
  }

  // Double click rename should support cancel via outside click.
  await label.dblclick()
  await renameInput.waitFor({ state: "visible" })
  await renameInput.fill("Temp")
  await page.getByTestId("nav-inbox-button").click()
  await label.waitFor({ state: "visible" })
  const afterCancel = ((await label.textContent()) ?? "").trim()
  if (afterCancel !== "Local Done") {
    throw new Error(`expected rename cancel to preserve label, got: ${JSON.stringify(afterCancel)}`)
  }

  // Double click rename should support save via ✅.
  await label.dblclick()
  await renameInput.waitFor({ state: "visible" })
  await renameInput.fill("Local Done 2")
  await page.getByTestId("inbox-view-item-0-save").click()
  await label.waitFor({ state: "visible" })
  const afterSave = ((await label.textContent()) ?? "").trim()
  if (afterSave !== "Local Done 2") {
    throw new Error(`expected rename save to update label, got: ${JSON.stringify(afterSave)}`)
  }

  // Clear filters and then re-apply via the view.
  await page.getByTestId("inbox-filter-button").click()
  await page.getByTestId("inbox-filter-menu").waitFor({ state: "visible" })
  await page.getByTestId("inbox-filter-clear").click()
  await closeFilterMenu({ page })
  await waitForFilterBarHidden({ page })

  await waitForInboxTitlePresence({ page, title: "Mock task 1", shouldExist: true })
  await waitForInboxTitlePresence({ page, title: "Local: done", shouldExist: false })

  await label.click()
  await waitForInboxTitlePresence({ page, title: "Local: done", shouldExist: true })
  await waitForInboxTitlePresence({ page, title: "Mock task 1", shouldExist: false })

  // Delete view.
  await page.getByTestId("inbox-view-item-0-delete").click()
  await page.getByTestId("inbox-view-item-0").waitFor({ state: "detached" })

  // Restore defaults for later scenarios.
  await page.getByTestId("inbox-filter-button").click()
  await page.getByTestId("inbox-filter-menu").waitFor({ state: "visible" })
  await page.getByTestId("inbox-filter-clear").click()
  await closeFilterMenu({ page })
  await waitForFilterBarHidden({ page })

  await waitForInboxTitlePresence({ page, title: "Mock task 1", shouldExist: true })
  await waitForInboxTitlePresence({ page, title: "Local: done", shouldExist: false })
}
