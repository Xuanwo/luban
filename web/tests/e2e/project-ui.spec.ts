import { expect, test, type Page } from "@playwright/test"
import { execFileSync } from "node:child_process"
import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import { requireEnv } from "./env"
import { activeWorkspaceId, sendWsAction } from "./helpers"

function runGit(cwd: string, args: string[]) {
  execFileSync("git", args, { cwd, stdio: "ignore" })
}

function projectToggleByPath(page: Page, projectPath: string) {
  return page.getByTitle(projectPath, { exact: true }).locator("..")
}

function ensureEmptyDir(dir: string) {
  fs.rmSync(dir, { recursive: true, force: true })
  fs.mkdirSync(dir, { recursive: true })
}

test("project can be deleted via sidebar confirmation dialog", async ({ page }) => {
  await page.goto("/")

  const root = requireEnv("LUBAN_E2E_ROOT")

  const tmpBase = path.join(os.tmpdir(), "luban-e2e-delete-project-with-a-very-very-long-name-")
  const projectDir = fs.mkdtempSync(tmpBase)
  ensureEmptyDir(projectDir)

  runGit(projectDir, ["init"])
  runGit(projectDir, ["config", "user.email", "e2e@example.com"])
  runGit(projectDir, ["config", "user.name", "luban-e2e"])
  runGit(projectDir, ["checkout", "-b", "main"])

  fs.writeFileSync(path.join(projectDir, "README.md"), "luban delete project e2e\n", "utf8")
  runGit(projectDir, ["add", "."])
  runGit(projectDir, ["commit", "-m", "init"])

  const projectPath = fs.realpathSync(projectDir)
  await sendWsAction(page, { type: "add_project", path: projectPath })

  const resolveProject = async () =>
    await page.evaluate(async (projectPath) => {
      const res = await fetch("/api/app")
      if (!res.ok) return null
      const app = (await res.json()) as { projects: { id: string; path: string }[] }
      const project = app.projects.find((p) => p.path === projectPath) ?? null
      if (!project) return null
      return { id: project.id }
    }, projectPath)

  await expect.poll(async () => await resolveProject(), { timeout: 15_000 }).not.toBeNull()
  const project = await resolveProject()
  if (!project) throw new Error("project not found after add_project")

  const projectToggle = projectToggleByPath(page, projectPath)
  await projectToggle.waitFor({ state: "visible", timeout: 15_000 })
  const projectContainer = projectToggle.locator("..").locator("..")

  await projectContainer.hover()
  await expect(projectContainer.getByTestId("project-pin-button")).toHaveCount(0)
  await expect(projectContainer.getByTestId("project-delete-button")).toBeVisible()
  const sidebarBox = await page.getByTestId("left-sidebar").boundingBox()
  const deleteButtonBox = await projectContainer.getByTestId("project-delete-button").boundingBox()
  if (!sidebarBox) throw new Error("left sidebar bounding box not found")
  if (!deleteButtonBox) throw new Error("delete button bounding box not found")
  expect(deleteButtonBox.x).toBeGreaterThanOrEqual(sidebarBox.x - 1)
  expect(deleteButtonBox.x + deleteButtonBox.width).toBeLessThanOrEqual(sidebarBox.x + sidebarBox.width + 1)
  await projectContainer.getByTestId("project-delete-button").click()

  await expect(page.getByTestId("project-delete-dialog")).toBeVisible({ timeout: 10_000 })
  await expect(page.getByText("Your local files will not be affected.", { exact: false })).toBeVisible()
  await page.getByTestId("project-delete-confirm").click()

  await expect(page.getByTitle(projectPath, { exact: true })).toHaveCount(0, { timeout: 20_000 })
  await expect
    .poll(async () => await resolveProject(), { timeout: 20_000 })
    .toBeNull()
})

test("deleting the active project switches to another project's main workspace", async ({ page }) => {
  await page.goto("/")

  const makeGitProject = (suffix: string) => {
    const remoteDir = fs.mkdtempSync(path.join(os.tmpdir(), `luban-e2e-delete-active-${suffix}-origin-`))
    execFileSync("git", ["init", "--bare"], { cwd: remoteDir, stdio: "ignore" })

    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), `luban-e2e-delete-active-${suffix}-`))
    const projectDir = path.join(repoRoot, "repo")
    execFileSync("git", ["clone", remoteDir, projectDir], { stdio: "ignore" })

    fs.writeFileSync(path.join(projectDir, "README.md"), `e2e ${suffix}\n`, "utf8")
    execFileSync("git", ["checkout", "-b", "main"], { cwd: projectDir, stdio: "ignore" })
    execFileSync("git", ["config", "user.email", "e2e@example.com"], { cwd: projectDir, stdio: "ignore" })
    execFileSync("git", ["config", "user.name", "e2e"], { cwd: projectDir, stdio: "ignore" })
    execFileSync("git", ["add", "-A"], { cwd: projectDir, stdio: "ignore" })
    execFileSync("git", ["commit", "-m", "init"], { cwd: projectDir, stdio: "ignore" })
    execFileSync("git", ["push", "-u", "origin", "main"], { cwd: projectDir, stdio: "ignore" })
    execFileSync("git", ["symbolic-ref", "HEAD", "refs/heads/main"], { cwd: remoteDir, stdio: "ignore" })
    execFileSync("git", ["fetch", "--prune", "origin"], { cwd: projectDir, stdio: "ignore" })
    execFileSync("git", ["remote", "set-head", "origin", "-a"], { cwd: projectDir, stdio: "ignore" })

    return fs.realpathSync(projectDir)
  }

  const projectPathA = makeGitProject("a")
  const projectPathB = makeGitProject("b")
  const expectedDisplayNames = computeDisplayNames([projectPathA, projectPathB])
  const displayNameB = expectedDisplayNames.get(projectPathB) ?? projectPathB
  const displayNameAfterDeletion = computeDisplayNames([projectPathB]).get(projectPathB) ?? displayNameB

  await sendWsAction(page, { type: "add_project", path: projectPathA })
  await sendWsAction(page, { type: "add_project", path: projectPathB })

  const resolve = async () =>
    await page.evaluate(async ({ projectPathA, projectPathB }) => {
      const res = await fetch("/api/app")
      if (!res.ok) return null
      const app = (await res.json()) as {
        projects: {
          id: string
          path: string
          workspaces: { id: number; status: string; workspace_name: string; worktree_path: string }[]
        }[]
      }
      const pa = app.projects.find((p) => p.path === projectPathA) ?? null
      const pb = app.projects.find((p) => p.path === projectPathB) ?? null
      if (!pa || !pb) return null
      const mainA =
        pa.workspaces.find((w) => w.status === "active" && w.workspace_name === "main" && w.worktree_path === pa.path) ??
        pa.workspaces.find((w) => w.status === "active" && w.workspace_name === "main") ??
        null
      const mainB =
        pb.workspaces.find((w) => w.status === "active" && w.workspace_name === "main" && w.worktree_path === pb.path) ??
        pb.workspaces.find((w) => w.status === "active" && w.workspace_name === "main") ??
        null
      return { projectA: { id: pa.id, mainId: mainA?.id ?? null }, projectB: { id: pb.id, mainId: mainB?.id ?? null } }
    }, { projectPathA, projectPathB })

  await expect.poll(async () => await resolve(), { timeout: 15_000 }).not.toBeNull()
  let snap = await resolve()
  if (!snap) throw new Error("projects not found after add_project")

  if (!snap.projectA.mainId) await sendWsAction(page, { type: "ensure_main_workspace", project_id: snap.projectA.id })
  if (!snap.projectB.mainId) await sendWsAction(page, { type: "ensure_main_workspace", project_id: snap.projectB.id })

  await expect
    .poll(async () => {
      const next = await resolve()
      return Boolean(next?.projectA.mainId) && Boolean(next?.projectB.mainId)
    }, { timeout: 90_000 })
    .toBeTruthy()

  snap = await resolve()
  if (!snap) throw new Error("projects not found")
  if (!snap.projectA.mainId || !snap.projectB.mainId) throw new Error("main workspace ids not found")

  const projectAToggle = projectToggleByPath(page, projectPathA)
  const projectBToggle = projectToggleByPath(page, projectPathB)
  await projectAToggle.waitFor({ state: "visible", timeout: 15_000 })
  await projectBToggle.waitFor({ state: "visible", timeout: 15_000 })

  await projectAToggle.click()
  await expect
    .poll(async () => String(await activeWorkspaceId(page, { timeoutMs: 500 })), { timeout: 20_000 })
    .toBe(String(snap.projectA.mainId))

  const projectAContainer = projectAToggle.locator("..").locator("..")
  await projectAContainer.hover()
  await projectAContainer.getByTestId("project-delete-button").click()
  await expect(page.getByTestId("project-delete-dialog")).toBeVisible({ timeout: 10_000 })
  await page.getByTestId("project-delete-confirm").click()
  await expect(page.getByTitle(projectPathA, { exact: true })).toHaveCount(0, { timeout: 20_000 })

  await expect
    .poll(async () => String(await activeWorkspaceId(page, { timeoutMs: 500 })), { timeout: 30_000 })
    .toBe(String(snap.projectB.mainId))
  await expect(page.getByTestId("active-project-name")).toHaveText(displayNameAfterDeletion, { timeout: 30_000 })
})

function splitPathSegments(pathValue: string): string[] {
  return pathValue
    .split(/[\\/]+/)
    .map((s) => s.trim())
    .filter(Boolean)
}

function computeDisplayNames(projectPaths: string[]): Map<string, string> {
  const byBasename = new Map<string, string[]>()
  for (const projectPath of projectPaths) {
    const segs = splitPathSegments(projectPath)
    const basename = segs[segs.length - 1] || projectPath
    const group = byBasename.get(basename) ?? []
    group.push(projectPath)
    byBasename.set(basename, group)
  }

  const result = new Map<string, string>()

  for (const [basename, group] of byBasename) {
    if (group.length === 1) {
      result.set(group[0]!, basename)
      continue
    }

    const pathSegments = group.map((p) => splitPathSegments(p).reverse())
    const maxDepth = Math.max(...pathSegments.map((s) => s.length))
    let depth = 1
    while (depth <= maxDepth) {
      const suffixes = pathSegments.map((segs) => segs.slice(0, depth).reverse().join("/"))
      const uniqueSuffixes = new Set(suffixes)
      if (uniqueSuffixes.size === group.length) {
        group.forEach((p, i) => result.set(p, suffixes[i]!))
        break
      }
      depth++
    }

    if (!result.has(group[0]!)) {
      group.forEach((p) => result.set(p, p))
    }
  }

  return result
}
