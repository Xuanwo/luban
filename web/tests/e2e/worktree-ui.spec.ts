import { expect, test } from "@playwright/test"
import { PNG } from "pngjs"
import { ensureWorkspace } from "./helpers"

function parseRgb(color: string): { r: number; g: number; b: number } | null {
  const m = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)(?:\s*,\s*[\d.]+\s*)?\)$/.exec(color.trim())
  if (!m) return null
  return { r: Number(m[1]), g: Number(m[2]), b: Number(m[3]) }
}

function samplePixel(png: PNG, x: number, y: number): { r: number; g: number; b: number; a: number } {
  const ix = Math.max(0, Math.min(png.width - 1, x))
  const iy = Math.max(0, Math.min(png.height - 1, y))
  const idx = (png.width * iy + ix) * 4
  return {
    r: png.data[idx] ?? 0,
    g: png.data[idx + 1] ?? 0,
    b: png.data[idx + 2] ?? 0,
    a: png.data[idx + 3] ?? 0,
  }
}

test("worktree item shows branch name above short id", async ({ page }) => {
  await ensureWorkspace(page)

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

  const tabs = page.locator('[data-testid="thread-tab-title"]')
  const beforeCount = await tabs.count()
  expect(beforeCount).toBeGreaterThan(0)
  const beforeLast = (await tabs.last().textContent())?.trim()

  await page.getByTitle("New tab").click()
  await expect(tabs).toHaveCount(beforeCount + 1, { timeout: 20_000 })

  const afterLast = (await tabs.last().textContent())?.trim()
  expect(afterLast).not.toBe(beforeLast)
})

test("sidebar resize gutter does not break header divider line", async ({ page }) => {
  await ensureWorkspace(page)

  const aside = page.locator("aside").first()
  const sidebarHeader = page.locator("aside > div").first()

  const sidebarHeaderBorder = await sidebarHeader.evaluate((el) => getComputedStyle(el).borderBottomColor)
  const rgb = parseRgb(sidebarHeaderBorder)
  expect(rgb, `unexpected sidebar header border: ${sidebarHeaderBorder}`).not.toBeNull()

  const asideBox = await aside.boundingBox()
  const headerBox = await sidebarHeader.boundingBox()
  expect(asideBox, "sidebar should have bounds").not.toBeNull()
  expect(headerBox, "sidebar header should have bounds").not.toBeNull()

  const sampleX = Math.floor((asideBox?.x ?? 0) + (asideBox?.width ?? 0) + 1)
  const sampleY = Math.floor((headerBox?.y ?? 0) + (headerBox?.height ?? 0) - 1)

  const shot1 = await page.screenshot({
    clip: { x: Math.max(0, sampleX - 3), y: Math.max(0, sampleY - 3), width: 7, height: 7 },
  })
  const png1 = PNG.sync.read(shot1)
  const px1 = samplePixel(png1, 3, 3)

  const tol = 18
  expect(px1.a).toBeGreaterThan(200)
  expect(Math.abs(px1.r - (rgb?.r ?? 0))).toBeLessThanOrEqual(tol)
  expect(Math.abs(px1.g - (rgb?.g ?? 0))).toBeLessThanOrEqual(tol)
  expect(Math.abs(px1.b - (rgb?.b ?? 0))).toBeLessThanOrEqual(tol)

  const handle = page.getByTestId("sidebar-resizer")
  const handleBox = await handle.boundingBox()
  expect(handleBox, "sidebar resizer should have bounds").not.toBeNull()

  await page.mouse.move((handleBox?.x ?? 0) + (handleBox?.width ?? 0) / 2, (handleBox?.y ?? 0) + 40)
  await page.mouse.down()
  await page.mouse.move((handleBox?.x ?? 0) + (handleBox?.width ?? 0) / 2 + 80, (handleBox?.y ?? 0) + 40)
  await page.mouse.up()

  const asideBox2 = await aside.boundingBox()
  expect((asideBox2?.width ?? 0)).toBeGreaterThan((asideBox?.width ?? 0))

  const sampleX2 = Math.floor((asideBox2?.x ?? 0) + (asideBox2?.width ?? 0) + 1)
  const shot2 = await page.screenshot({
    clip: { x: Math.max(0, sampleX2 - 3), y: Math.max(0, sampleY - 3), width: 7, height: 7 },
  })
  const png2 = PNG.sync.read(shot2)
  const px2 = samplePixel(png2, 3, 3)

  expect(px2.a).toBeGreaterThan(200)
  expect(Math.abs(px2.r - (rgb?.r ?? 0))).toBeLessThanOrEqual(tol)
  expect(Math.abs(px2.g - (rgb?.g ?? 0))).toBeLessThanOrEqual(tol)
  expect(Math.abs(px2.b - (rgb?.b ?? 0))).toBeLessThanOrEqual(tol)
})
