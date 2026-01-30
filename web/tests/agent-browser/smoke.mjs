import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

import { BrowserManager } from 'agent-browser/dist/browser.js';

async function canRun(command, args) {
  const proc = spawn(command, args, { stdio: 'ignore' });
  return await new Promise((resolve) => proc.on('close', (code) => resolve(code === 0)));
}

async function resolvePnpmCommand() {
  if (await canRun('pnpm', ['--version'])) return 'pnpm';
  if (await canRun('pnpm.cmd', ['--version'])) return 'pnpm.cmd';
  return null;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForHttpOk(url, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const resp = await fetch(url, { method: 'GET' });
      if (resp.ok) return;
    } catch {
      // ignore and retry
    }
    await sleep(250);
  }
  throw new Error(`web dev server did not become ready at ${url}`);
}

async function pickEphemeralPort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = server.address();
  server.close();
  if (!address || typeof address === 'string') throw new Error('failed to pick ephemeral port');
  return address.port;
}

function normalizeBaseUrl(value) {
  if (!value.startsWith('http://') && !value.startsWith('https://')) {
    throw new Error(`invalid base url (missing scheme): ${value}`);
  }
  return value.endsWith('/') ? value : `${value}/`;
}

async function waitForDataAttribute(locator, attr, expected, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const value = await locator.getAttribute(attr);
    if (value === expected) return;
    await sleep(250);
  }
  const value = await locator.getAttribute(attr);
  throw new Error(`expected ${attr}=${expected}, got ${value ?? 'null'}`);
}

async function waitForLocatorCount(locator, expected, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const count = await locator.count();
    if (count === expected) return;
    await sleep(250);
  }
  const count = await locator.count();
  throw new Error(`expected ${expected} matches, got ${count}`);
}

async function main() {
  const pnpmCommand = await resolvePnpmCommand();
  if (!pnpmCommand) {
    throw new Error('pnpm not found; install pnpm to run UI smoke tests');
  }

  const sessionId = process.env.LUBAN_AGENT_BROWSER_SESSION ?? `luban-${crypto.randomBytes(8).toString('hex')}`;
  const headed = process.env.LUBAN_AGENT_BROWSER_HEADED === '1';
  const profileDir = process.env.LUBAN_AGENT_BROWSER_PROFILE_DIR ?? path.join(os.tmpdir(), `luban-agent-browser-${sessionId}`);
  const shouldCleanupProfile = process.env.LUBAN_AGENT_BROWSER_PROFILE_DIR == null;

  process.stderr.write(`agent-browser session: ${sessionId}\n`);
  process.stderr.write(`agent-browser profile: ${profileDir}\n`);

  fs.mkdirSync(profileDir, { recursive: true });

  const explicitBaseUrl = process.env.LUBAN_AGENT_BROWSER_BASE_URL;
  let baseUrl;
  let dev = null;
  let devClosed = null;
  let logFile = null;
  let logStream = null;

  if (explicitBaseUrl) {
    baseUrl = normalizeBaseUrl(explicitBaseUrl);
  } else {
    const requestedPort =
      process.env.LUBAN_AGENT_BROWSER_PORT == null ? null : Number.parseInt(process.env.LUBAN_AGENT_BROWSER_PORT, 10);
    if (requestedPort != null && (!Number.isFinite(requestedPort) || requestedPort <= 0)) {
      throw new Error(`invalid LUBAN_AGENT_BROWSER_PORT: ${process.env.LUBAN_AGENT_BROWSER_PORT}`);
    }

    const tryPort = requestedPort ?? 3000;
    const defaultUrl = `http://127.0.0.1:${tryPort}/`;
    try {
      await waitForHttpOk(defaultUrl, 750);
      baseUrl = defaultUrl;
    } catch {
      const port = requestedPort ?? (await pickEphemeralPort());
      baseUrl = `http://127.0.0.1:${port}/`;

      logFile = path.join(os.tmpdir(), `luban-agent-browser-ui-smoke-${sessionId}.log`);
      logStream = fs.createWriteStream(logFile, { flags: 'w' });
      dev = spawn(pnpmCommand, ['exec', 'next', 'dev', '-p', String(port)], {
        env: {
          ...process.env,
          NEXT_PUBLIC_LUBAN_MODE: 'mock',
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      dev.stdout?.pipe(logStream);
      dev.stderr?.pipe(logStream);
      devClosed = new Promise((resolve) => dev.once('close', resolve));
    }
  }

  process.stderr.write(`ui base url: ${baseUrl}\n`);

  let browser;
  try {
    if (dev) {
      const start = Date.now();
      while (Date.now() - start < 60_000) {
        if (dev.exitCode != null) break;
        try {
          await waitForHttpOk(baseUrl, 750);
          break;
        } catch {
          // ignore and retry
        }
      }
      if (dev.exitCode != null) {
        process.stderr.write(`ui smoke failed; log: ${logFile}\n`);
        throw new Error('web dev server exited before it became ready');
      }
      await waitForHttpOk(baseUrl, 5_000);
    } else {
      await waitForHttpOk(baseUrl, 10_000);
    }

    browser = new BrowserManager();
    await browser.launch({
      id: sessionId,
      action: 'launch',
      headless: !headed,
      viewport: { width: 1280, height: 720 },
      profile: profileDir,
    });

    const page = browser.getPage();
    await page.goto(baseUrl, { waitUntil: 'networkidle' });

    await page.getByTestId('nav-sidebar').waitFor({ state: 'visible' });
    await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

    await page.getByTestId('new-task-button').click();
    await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });
    await page.getByTestId('new-task-input').fill('Fix: programmatic agent-browser smoke');
    await sleep(500);
    await page.keyboard.press('Escape');

    await page.getByTestId('nav-inbox-button').click();
    await page.getByTestId('inbox-view').waitFor({ state: 'visible' });

    const row0 = page.getByTestId('inbox-notification-row-0');
    await row0.waitFor({ state: 'visible' });
    await waitForDataAttribute(row0, 'data-read', 'false', 20_000);

    await row0.click();
    await row0.waitFor({ state: 'visible' });
    await waitForDataAttribute(row0, 'data-read', 'true', 20_000);

    const starButton = page.getByTestId('task-star-button');
    await starButton.waitFor({ state: 'visible' });

    const favoriteItems = page.locator('[data-testid^="favorite-task-"]');
    await waitForLocatorCount(favoriteItems, 0, 5_000);

    await starButton.click();
    await waitForDataAttribute(starButton, 'aria-pressed', 'true', 10_000);
    await waitForLocatorCount(favoriteItems, 1, 20_000);

    await starButton.click();
    await waitForDataAttribute(starButton, 'aria-pressed', 'false', 10_000);
    await waitForLocatorCount(favoriteItems, 0, 20_000);

    await page.getByTestId('workspace-switcher-button').click();
    await page.getByTestId('open-settings-button').click();
    await page.getByTestId('settings-panel').waitFor({ state: 'visible' });
  } catch (err) {
    if (logFile) {
      process.stderr.write(`ui smoke failed; log: ${logFile}\n`);
    }
    throw err;
  } finally {
    await browser?.close().catch(() => {});

    if (dev && dev.exitCode == null && dev.pid) {
      dev.kill('SIGTERM');
    }
    if (devClosed) {
      await devClosed;
    }
    logStream?.end();

    if (shouldCleanupProfile) {
      fs.rmSync(profileDir, { recursive: true, force: true });
    }
  }
}

await main();
