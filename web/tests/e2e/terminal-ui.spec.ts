import { expect, test, type Page } from "@playwright/test"
import { PNG } from "pngjs"
import { ensureWorkspace, sendWsAction } from "./helpers"

function decodeWsPayload(payload: unknown): { kind: "text" | "binary"; text: string } {
  if (typeof payload === "string") return { kind: "text", text: payload }
  if (payload == null) return { kind: "text", text: "" }

  if (payload instanceof Uint8Array) return { kind: "binary", text: Buffer.from(payload).toString("utf8") }
  if (payload instanceof ArrayBuffer) return { kind: "binary", text: Buffer.from(new Uint8Array(payload)).toString("utf8") }
  return { kind: "text", text: String(payload) }
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
}

function waitForPtyOutput(page: Page, match: RegExp, timeoutMs = 10_000): Promise<string> {
  return new Promise((resolve, reject) => {
    let buffer = ""
    const timeout = setTimeout(() => {
      reject(new Error(`Timed out waiting for PTY output: ${match}`))
    }, timeoutMs)

    page.on("websocket", (ws) => {
      if (!ws.url().includes("/api/pty/")) return
      ws.on("framereceived", (ev) => {
        const decoded = decodeWsPayload(ev.payload)
        if (decoded.kind !== "binary") return
        buffer = (buffer + decoded.text).slice(-16384)
        if (!match.test(buffer)) return
        clearTimeout(timeout)
        resolve(buffer)
      })
    })
  })
}

function waitForPtyOutputOutcome(
  page: Page,
  expected: RegExp,
  unwanted: RegExp,
  timeoutMs = 10_000,
): Promise<"expected" | "unwanted"> {
  return new Promise((resolve, reject) => {
    let buffer = ""
    const timeout = setTimeout(() => {
      reject(new Error(`Timed out waiting for PTY output: expected=${expected}, unwanted=${unwanted}`))
    }, timeoutMs)

    page.on("websocket", (ws) => {
      if (!ws.url().includes("/api/pty/")) return
      ws.on("framereceived", (ev) => {
        const decoded = decodeWsPayload(ev.payload)
        if (decoded.kind !== "binary") return
        buffer = (buffer + decoded.text).slice(-16384)
        if (expected.test(buffer)) {
          clearTimeout(timeout)
          resolve("expected")
          return
        }
        if (unwanted.test(buffer)) {
          clearTimeout(timeout)
          resolve("unwanted")
        }
      })
    })
  })
}

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
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

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
  await ensureWorkspace(page)
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  const expectedAfter = await terminal.evaluate((el) => getComputedStyle(el as Element).backgroundColor)
  const rgbAfter = parseRgb(expectedAfter)
  expect(rgbAfter, `unexpected terminal background after reload: ${expectedAfter}`).not.toBeNull()

  const shotAfter = await terminal.screenshot()
  const pngAfter = PNG.sync.read(shotAfter)
  const pixelAfter = samplePixel(pngAfter, Math.floor(pngAfter.width / 2), Math.max(0, pngAfter.height - 10))

  expect(Math.abs(pixelAfter.r - (rgbAfter?.r ?? 0))).toBeLessThanOrEqual(tol)
  expect(Math.abs(pixelAfter.g - (rgbAfter?.g ?? 0))).toBeLessThanOrEqual(tol)
  expect(Math.abs(pixelAfter.b - (rgbAfter?.b ?? 0))).toBeLessThanOrEqual(tol)
})

test("terminal scrollbar is thin like chat scrollbar", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  const widths = await terminal.evaluate((outer) => {
    const scrollbar = outer.querySelector(".xterm-scrollable-element > .scrollbar.vertical") as HTMLElement | null
    const slider = outer.querySelector(".xterm-scrollable-element > .scrollbar.vertical > .slider") as HTMLElement | null
    if (!scrollbar || !slider) return null
    return {
      scrollbar: scrollbar.getBoundingClientRect().width,
      slider: slider.getBoundingClientRect().width,
    }
  })

  expect(widths, "terminal scrollbar DOM not found").not.toBeNull()
  expect(widths?.scrollbar ?? 0).toBeGreaterThan(0)
  expect(widths?.slider ?? 0).toBeGreaterThan(0)

  const max = 10
  expect(widths?.scrollbar ?? 0).toBeLessThanOrEqual(max)
  expect(widths?.slider ?? 0).toBeLessThanOrEqual(max)
})

test("terminal paste sends input frames", async ({ page }) => {
  const token = `luban-e2e-paste-${Math.random().toString(16).slice(2)}`
  const payload = `echo ${token}\n`

  const sentFrame = new Promise<void>((resolve) => {
    page.on("websocket", (ws) => {
      if (!ws.url().includes("/api/pty/")) return
      ws.on("framesent", (ev) => {
        const decoded = decodeWsPayload(ev.payload)
        if (decoded.kind !== "binary") return
        if (decoded.text.includes(token)) {
          resolve()
        }
      })
    })
  })

  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await terminal.evaluate(
    (el, payload) => {
      const event = new Event("paste", { bubbles: true, cancelable: true }) as any
      Object.defineProperty(event, "clipboardData", {
        value: {
          getData: (t: string) => (t === "text/plain" ? payload : ""),
        },
      })
      el.dispatchEvent(event)
    },
    payload,
  )

  await sentFrame
})

test("terminal ctrl+arrow sends word navigation input frames", async ({ page }) => {
  const leftFrame = new Promise<void>((resolve) => {
    page.on("websocket", (ws) => {
      if (!ws.url().includes("/api/pty/")) return
      ws.on("framesent", (ev) => {
        const decoded = decodeWsPayload(ev.payload)
        if (decoded.kind !== "binary") return
        if (decoded.text.includes("\u001bb")) resolve()
      })
    })
  })

  const rightFrame = new Promise<void>((resolve) => {
    page.on("websocket", (ws) => {
      if (!ws.url().includes("/api/pty/")) return
      ws.on("framesent", (ev) => {
        const decoded = decodeWsPayload(ev.payload)
        if (decoded.kind !== "binary") return
        if (decoded.text.includes("\u001bf")) resolve()
      })
    })
  })

  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await terminal.click({ force: true })
  await page.keyboard.down("Control")
  await page.keyboard.press("ArrowLeft")
  await page.keyboard.up("Control")
  await leftFrame

  await terminal.click({ force: true })
  await page.keyboard.down("Control")
  await page.keyboard.press("ArrowRight")
  await page.keyboard.up("Control")
  await rightFrame
})

test("terminal backspace sends DEL input frame", async ({ page }) => {
  const backspaceFrame = new Promise<void>((resolve) => {
    page.on("websocket", (ws) => {
      if (!ws.url().includes("/api/pty/")) return
      ws.on("framesent", (ev) => {
        const decoded = decodeWsPayload(ev.payload)
        if (decoded.kind !== "binary") return
        if (decoded.text.includes("\u007f")) resolve()
      })
    })
  })

  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await terminal.click({ force: true })
  await page.keyboard.press("Backspace")
  await backspaceFrame
})

test("terminal enter executes commands", async ({ page }) => {
  const token = `luban-e2e-enter-${Math.random().toString(16).slice(2)}`
  const reversed = token.split("").reverse().join("")
  const expected = new RegExp(escapeRegExp(reversed))
  const output = waitForPtyOutput(page, expected, 20_000)

  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await terminal.click({ force: true })
  await page.keyboard.type(`printf '%s\\n' ${token} | rev`)
  await page.keyboard.press("Enter")

  await output
})

test("terminal backspace edits the current command line", async ({ page }) => {
  const token = `luban-e2e-backspace-edit-${Math.random().toString(16).slice(2)}`
  const expected = new RegExp(escapeRegExp(token))
  const unwanted = /command not found:\s*catX/
  const outcome = waitForPtyOutputOutcome(page, expected, unwanted, 20_000)

  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await terminal.click({ force: true })
  await page.keyboard.type(`printf '%s\\n' ${token} | catX`)
  await page.keyboard.press("Backspace")
  await page.keyboard.press("Enter")

  expect(await outcome).toBe("expected")
})

test("terminal does not leak OSC color query replies as visible text", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await terminal.click({ force: true })

  // Trigger an OSC query that xterm.js may answer; the reply must never become visible text.
  // If control bytes are lost on the input path, the shell would echo plain `rgb:` content.
  await page.keyboard.type("printf '\\e]10;?\\a'\n")

  await expect
    .poll(async () => {
      return await terminal.evaluate((outer) => {
        const lines = Array.from(outer.querySelectorAll(".xterm-rows > div"))
          .map((el) => (el as HTMLElement).innerText)
          .join("\n")
        return lines.includes("rgb:")
      })
    })
    .toBe(false)
})

test("terminal does not leak 8-bit OSC color query replies as visible text", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await terminal.click({ force: true })

  // Same invariant as the 7-bit OSC test, but using the single-byte C1 OSC prefix (0x9d).
  // Encoding this as UTF-8 would corrupt the byte stream and can cause visible `rgb:` leaks.
  await page.keyboard.type("printf '\\x9d10;?\\a'\n")

  await expect
    .poll(async () => {
      return await terminal.evaluate((outer) => {
        const lines = Array.from(outer.querySelectorAll(".xterm-rows > div"))
          .map((el) => (el as HTMLElement).innerText)
          .join("\n")
        return lines.includes("rgb:")
      })
    })
    .toBe(false)
})

test("terminal theme follows app theme changes", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await sendWsAction(page, { type: "appearance_theme_changed", theme: "light" })
  await expect
    .poll(async () => await page.evaluate(() => document.documentElement.classList.contains("dark")), { timeout: 10_000 })
    .toBe(false)

  await sendWsAction(page, { type: "appearance_theme_changed", theme: "dark" })
  await expect
    .poll(async () => await page.evaluate(() => document.documentElement.classList.contains("dark")), {
      timeout: 10_000,
    })
    .toBe(true)

  const expected = await terminal.evaluate((el) => getComputedStyle(el as Element).backgroundColor)
  const rgb = parseRgb(expected)
  expect(rgb, `unexpected terminal background: ${expected}`).not.toBeNull()

  const tol = 10
  await expect
    .poll(async () => {
      const shot = await terminal.screenshot()
      const png = PNG.sync.read(shot)
      const pixel = samplePixel(png, Math.floor(png.width / 2), Math.max(0, png.height - 10))
      return (
        Math.abs(pixel.r - (rgb?.r ?? 0)) <= tol &&
        Math.abs(pixel.g - (rgb?.g ?? 0)) <= tol &&
        Math.abs(pixel.b - (rgb?.b ?? 0)) <= tol
      )
    })
    .toBe(true)
})

test("terminal canvas stays within container bounds", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await expect
    .poll(async () => {
      return await terminal.evaluate((outer) => {
        const canvas = outer.querySelector("canvas")
        if (!canvas) return null
        const outerRect = (outer as HTMLElement).getBoundingClientRect()
        const canvasRect = canvas.getBoundingClientRect()
        return {
          outer: { top: outerRect.top, bottom: outerRect.bottom, left: outerRect.left, right: outerRect.right },
          canvas: { top: canvasRect.top, bottom: canvasRect.bottom, left: canvasRect.left, right: canvasRect.right },
        }
      })
    })
    .not.toBeNull()

  const rects = await terminal.evaluate((outer) => {
    const canvas = outer.querySelector("canvas")!
    const outerRect = (outer as HTMLElement).getBoundingClientRect()
    const canvasRect = canvas.getBoundingClientRect()
    return {
      outerRect: { top: outerRect.top, bottom: outerRect.bottom, left: outerRect.left, right: outerRect.right },
      canvasRect: { top: canvasRect.top, bottom: canvasRect.bottom, left: canvasRect.left, right: canvasRect.right },
    }
  })

  const tol = 1
  expect(rects.canvasRect.top).toBeGreaterThanOrEqual(rects.outerRect.top - tol)
  expect(rects.canvasRect.left).toBeGreaterThanOrEqual(rects.outerRect.left - tol)
  expect(rects.canvasRect.right).toBeLessThanOrEqual(rects.outerRect.right + tol)
  expect(rects.canvasRect.bottom).toBeLessThanOrEqual(rects.outerRect.bottom + tol)
})

test("terminal does not add extra padding above the viewport", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  const padding = await terminal.evaluate((el) => {
    const style = getComputedStyle(el as HTMLElement)
    return {
      top: style.paddingTop,
      right: style.paddingRight,
      bottom: style.paddingBottom,
      left: style.paddingLeft,
    }
  })

  expect(padding.top).toBe("0px")
  expect(padding.right).toBe("0px")
  expect(padding.bottom).toBe("0px")
  expect(padding.left).toBe("0px")
})

test("terminal content has horizontal padding", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  const insets = await terminal.evaluate((outer) => {
    const xterm = outer.querySelector(".xterm") as HTMLElement | null
    if (!xterm) return null
    const outerRect = (outer as HTMLElement).getBoundingClientRect()
    const xtermRect = xterm.getBoundingClientRect()
    return {
      left: xtermRect.left - outerRect.left,
      right: outerRect.right - xtermRect.right,
    }
  })

  expect(insets).not.toBeNull()
  expect(insets?.left ?? 0).toBeGreaterThanOrEqual(6)
  expect(insets?.right ?? 0).toBeGreaterThanOrEqual(6)
})

test("terminal viewport uses auto scrollbar behavior", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  const snapshot = await terminal.evaluate((outer) => {
    const viewport = outer.querySelector(".xterm-viewport") as HTMLElement | null
    if (!viewport) return null
    const viewOverflowY = getComputedStyle(viewport).overflowY

    const parentOverflows: Array<{ tag: string; overflowY: string }> = []
    let node: HTMLElement | null = viewport.parentElement
    for (let i = 0; i < 10 && node; i += 1) {
      if (node === (outer as HTMLElement)) break
      const overflowY = getComputedStyle(node).overflowY
      parentOverflows.push({ tag: node.tagName.toLowerCase(), overflowY })
      node = node.parentElement
    }

    return { viewOverflowY, parentOverflows }
  })

  expect(snapshot).not.toBeNull()
  expect(snapshot?.viewOverflowY).toBe("auto")
  for (const entry of snapshot?.parentOverflows ?? []) {
    expect(entry.overflowY, `unexpected ancestor overflow-y: ${entry.tag} ${entry.overflowY}`).not.toBe("auto")
    expect(entry.overflowY, `unexpected ancestor overflow-y: ${entry.tag} ${entry.overflowY}`).not.toBe("scroll")
  }
})

test("terminal scrollbar is styled via app CSS", async ({ page }) => {
  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  const hasStyle = await terminal.evaluate((outer) => {
    if (!(outer instanceof HTMLElement)) return false
    if (!outer.classList.contains("luban-terminal")) return false

    for (const sheet of Array.from(document.styleSheets)) {
      let rules: CSSRuleList | null = null
      try {
        rules = sheet.cssRules
      } catch {
        continue
      }
      for (const rule of Array.from(rules)) {
        if (rule instanceof CSSStyleRule) {
          const css = rule.cssText
          if (
            css.includes(".luban-terminal") &&
            css.includes(".xterm-viewport") &&
            css.includes("::-webkit-scrollbar")
          ) {
            return true
          }
        }
        if ("cssText" in rule) {
          const css = String((rule as any).cssText ?? "")
          if (
            css.includes(".luban-terminal") &&
            css.includes(".xterm-viewport") &&
            css.includes("::-webkit-scrollbar")
          ) {
            return true
          }
        }
      }
      // Some browsers may not expose nested rules for `@layer`; fall back to scanning the sheet text.
      try {
        const owner = (sheet as any).ownerNode as HTMLStyleElement | HTMLLinkElement | null
        const text = owner && "textContent" in owner ? String(owner.textContent ?? "") : ""
        if (
          text.includes(".luban-terminal") &&
          text.includes(".xterm-viewport") &&
          text.includes("::-webkit-scrollbar")
        ) {
          return true
        }
      } catch {
        // ignore
      }
    }
    return false
  })

  expect(hasStyle).toBe(true)
})

test("terminal restarts after the shell exits", async ({ page }) => {
  let ptySocketCount = 0
  let receivedInitialOutput = false
  page.on("websocket", (ws) => {
    if (!ws.url().includes("/api/pty/")) return
    ptySocketCount += 1

    ws.on("framereceived", (ev) => {
      const decoded = decodeWsPayload(ev.payload)
      if (decoded.kind !== "binary") return
      if (decoded.text.length === 0) return
      receivedInitialOutput = true
    })
  })

  await ensureWorkspace(page)

  const terminal = page.getByTestId("pty-terminal")
  await expect
    .poll(async () => await terminal.locator("canvas").count(), { timeout: 20_000 })
    .toBeGreaterThan(0)

  await expect.poll(async () => ptySocketCount, { timeout: 20_000 }).toBeGreaterThan(0)
  await expect.poll(async () => receivedInitialOutput, { timeout: 20_000 }).toBe(true)

  const pasteIntoTerminal = async (text: string) => {
    await terminal.evaluate(
      (el, text) => {
        const event = new Event("paste", { bubbles: true, cancelable: true }) as any
        Object.defineProperty(event, "clipboardData", {
          value: {
            getData: (t: string) => (t === "text/plain" ? text : ""),
          },
        })
        el.dispatchEvent(event)
      },
      text,
    )
  }

  // Prefer Ctrl+D (EOT) to exit at the prompt, but fall back to `exit\n` for shells
  // that ignore EOT (e.g. `IGNORE_EOF`) so this test stays deterministic.
  // First, try to interrupt any long-running foreground process to avoid state bleed
  // from other terminal tests that reuse the same PTY session.
  await pasteIntoTerminal("\u0003")
  await page.waitForTimeout(100)

  await pasteIntoTerminal("\u0004")
  try {
    await expect.poll(async () => ptySocketCount, { timeout: 3_000 }).toBeGreaterThan(1)
    return
  } catch {
    // ignore and fall back
  }

  // Killing the shell is more deterministic than `exit\n` in case the shell is configured
  // to ignore EOF or refuses to exit due to interactive constraints.
  await pasteIntoTerminal("kill -KILL $$\n")
  await expect.poll(async () => ptySocketCount, { timeout: 20_000 }).toBeGreaterThan(1)
})
