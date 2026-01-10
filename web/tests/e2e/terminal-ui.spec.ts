import { expect, test } from "@playwright/test"
import { PNG } from "pngjs"
import { ensureWorkspace } from "./helpers"

function parseRgb(color: string): { r: number; g: number; b: number } | null {
  const m = /^rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)$/.exec(color.trim())
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

test("terminal background matches card background and survives reload", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect(terminal.locator("canvas")).toHaveCount(1, { timeout: 20_000 })

  const expected = await terminal.evaluate((el) => getComputedStyle(el as Element).backgroundColor)
  const rgb = parseRgb(expected)
  expect(rgb, `unexpected terminal background: ${expected}`).not.toBeNull()

  const shot = await terminal.screenshot()
  const png = PNG.sync.read(shot)
  const pixel = samplePixel(png, Math.floor(png.width / 2), Math.max(0, png.height - 10))

  const tol = 10
  expect(Math.abs(pixel.r - (rgb?.r ?? 0))).toBeLessThanOrEqual(tol)
  expect(Math.abs(pixel.g - (rgb?.g ?? 0))).toBeLessThanOrEqual(tol)
  expect(Math.abs(pixel.b - (rgb?.b ?? 0))).toBeLessThanOrEqual(tol)

  await page.reload()
  await expect(page.getByTestId("thread-tab-title").first()).toBeVisible({ timeout: 60_000 })
  await expect(terminal.locator("canvas")).toHaveCount(1, { timeout: 20_000 })
})
