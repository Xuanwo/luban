import { defineConfig } from "@playwright/test"

export default defineConfig({
  testDir: "./tests/e2e",
  globalSetup: "./tests/e2e/global-setup.ts",
  timeout: 120_000,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : [["list"]],
  use: {
    baseURL: "http://127.0.0.1:8421",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  webServer: {
    command:
      "cd .. && " +
      "rm -rf web/.playwright-home && " +
      "mkdir -p web/.playwright-home web/.playwright-project && " +
      "RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup} " +
      "CARGO_HOME=${CARGO_HOME:-$HOME/.cargo} " +
      "HOME=$PWD/web/.playwright-home " +
      "LUBAN_WEB_DIST_DIR=web/out " +
      "LUBAN_SERVER_ADDR=127.0.0.1:8421 " +
      "LUBAN_CODEX_BIN=/usr/bin/false " +
      "just run",
    url: "http://127.0.0.1:8421/api/health",
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
})
