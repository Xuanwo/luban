import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

import { BrowserManager } from 'agent-browser/dist/browser.js';

import { waitForHttpOk } from './lib/utils.mjs';
import { runActivityAttachments } from './scenarios/activity-attachments.mjs';
import { runActivityTerminalCommand } from './scenarios/activity-terminal-command.mjs';
import { runInboxRead } from './scenarios/inbox-read.mjs';
import { runInboxPreviewLine } from './scenarios/inbox-preview-line.mjs';
import { runInboxSortStability } from './scenarios/inbox-sort-stability.mjs';
import { runInboxStatusChange } from './scenarios/inbox-status-change.mjs';
import { runLatestEventsVisible } from './scenarios/latest-events-visible.mjs';
import { runActivityWindowing } from './scenarios/activity-windowing.mjs';
import { runAgentRunnerIcons } from './scenarios/agent-runner-icons.mjs';
import { runAgentRunningEventNoChevron } from './scenarios/agent-running-event-no-chevron.mjs';
import { runEscapeDoesNotLeakFromChatInput } from './scenarios/escape-does-not-leak-from-chat-input.mjs';
import { runNewTaskModal } from './scenarios/new-task-modal.mjs';
import { runNewTaskDoubleSubmitNoDuplicate } from './scenarios/new-task-double-submit-no-duplicate.mjs';
import { runNewTaskDefaultProjectFollowsContext } from './scenarios/new-task-default-project-follows-context.mjs';
import { runNewTaskGitProjectWithoutWorkdirs } from './scenarios/new-task-git-project-without-workdirs.mjs';
import { runNewTaskProjectAvatars } from './scenarios/new-task-project-avatars.mjs';
import { runNewTaskDrafts } from './scenarios/new-task-drafts.mjs';
import { runNoRightSidebar } from './scenarios/no-right-sidebar.mjs';
import { runSettingsPanel } from './scenarios/settings-panel.mjs';
import { runSidebarProjectAvatars } from './scenarios/sidebar-project-avatars.mjs';
import { runTaskListProjectIcon } from './scenarios/task-list-project-icon.mjs';
import { runStarFavorites } from './scenarios/star-favorites.mjs';
import { runTaskStatusChange } from './scenarios/task-status-change.mjs';
import { runTaskListNavigation } from './scenarios/task-list-navigation.mjs';
import { runTaskSummariesEventsRefresh } from './scenarios/task-summaries-events-refresh.mjs';
import { runQueuedPrompts } from './scenarios/queued-prompts.mjs';
import { runProjectAllTasksView } from './scenarios/project-all-tasks-view.mjs';

async function canRun(command, args) {
  const proc = spawn(command, args, { stdio: 'ignore' });
  return await new Promise((resolve) => proc.on('close', (code) => resolve(code === 0)));
}

async function resolvePnpmCommand() {
  if (await canRun('pnpm', ['--version'])) return 'pnpm';
  if (await canRun('pnpm.cmd', ['--version'])) return 'pnpm.cmd';
  return null;
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
	
	    await runSidebarProjectAvatars({ page, baseUrl });
      await runTaskListProjectIcon({ page, baseUrl });
      await runProjectAllTasksView({ page, baseUrl });
	    await runNewTaskModal({ page, baseUrl });
	    await runNewTaskDrafts({ page, baseUrl });
	    await runNewTaskProjectAvatars({ page, baseUrl });
		    await runNewTaskGitProjectWithoutWorkdirs({ page, baseUrl });
		    await runActivityAttachments({ page, baseUrl });
        await runActivityTerminalCommand({ page, baseUrl });
		    await runInboxRead({ page, baseUrl });
		    await runInboxPreviewLine({ page, baseUrl });
			    await runInboxSortStability({ page, baseUrl });
			    await runInboxStatusChange({ page, baseUrl });
		    await runStarFavorites({ page, baseUrl });
    await runNewTaskDefaultProjectFollowsContext({ page, baseUrl });
    await runSettingsPanel({ page, baseUrl });
    await runTaskListNavigation({ page, baseUrl });
    await runTaskStatusChange({ page, baseUrl });
    await runQueuedPrompts({ page, baseUrl });
    await runLatestEventsVisible({ page, baseUrl });
    await runActivityWindowing({ page, baseUrl });
    await runAgentRunnerIcons({ page, baseUrl });
    await runEscapeDoesNotLeakFromChatInput({ page, baseUrl });
    await runAgentRunningEventNoChevron({ page, baseUrl });
    await runNewTaskDoubleSubmitNoDuplicate({ page, baseUrl });
    await runTaskSummariesEventsRefresh({ page, baseUrl });
    await runNoRightSidebar({ page, baseUrl });
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
