import { execFileSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"

function runGit(cwd: string, args: string[]) {
  execFileSync("git", args, { cwd, stdio: "ignore" })
}

function ensureEmptyDir(dir: string) {
  fs.rmSync(dir, { recursive: true, force: true })
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(filePath: string, content: string) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true })
  fs.writeFileSync(filePath, content, "utf8")
}

export default async function globalSetup() {
  const projectRoot = path.resolve(__dirname, "../../..")
  const projectDir = path.join(projectRoot, "web", ".playwright-project", "e2e-project")
  const remoteDir = path.join(projectRoot, "web", ".playwright-project", "remote.git")

  ensureEmptyDir(path.dirname(projectDir))
  ensureEmptyDir(projectDir)
  ensureEmptyDir(remoteDir)

  runGit(remoteDir, ["init", "--bare"])
  runGit(remoteDir, ["symbolic-ref", "HEAD", "refs/heads/main"])

  runGit(projectDir, ["init"])
  runGit(projectDir, ["config", "user.email", "e2e@example.com"])
  runGit(projectDir, ["config", "user.name", "luban-e2e"])
  runGit(projectDir, ["checkout", "-b", "main"])

  writeFile(path.join(projectDir, "README.md"), "luban e2e project\n")
  runGit(projectDir, ["add", "."])
  runGit(projectDir, ["commit", "-m", "init"])

  runGit(projectDir, ["remote", "add", "origin", remoteDir])
  runGit(projectDir, ["push", "-u", "origin", "main"])

  process.env.LUBAN_E2E_PROJECT_DIR = projectDir
}

