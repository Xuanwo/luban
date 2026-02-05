import { sleep } from '../lib/utils.mjs';

async function findInboxRowByTitle(page, expectedTitle, timeoutMs = 20_000) {
  const rows = page.locator('[data-testid^="inbox-notification-row-"]');
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const rowCount = await rows.count();
    for (let i = 0; i < Math.min(rowCount, 40); i += 1) {
      const row = page.getByTestId(`inbox-notification-row-${i}`);
      const title = ((await row.getByTestId('inbox-notification-task-title').textContent()) ?? '').trim();
      if (title === expectedTitle) return row;
    }
    await sleep(200);
  }
  throw new Error(`timeout locating inbox row with title ${JSON.stringify(expectedTitle)}`);
}

async function waitForNotRunning(row, timeoutMs = 20_000) {
  const status = row.getByTestId('inbox-notification-task-status-icon');
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const spinning = await status.locator('svg.animate-spin').count();
    if (spinning === 0) return;
    await sleep(200);
  }
  throw new Error('timeout waiting for running icon to clear');
}

async function assertRunningShowsOnlySpinner(row) {
  const status = row.getByTestId('inbox-notification-task-status-icon');
  await status.getByTestId('inbox-notification-runner-spinner').waitFor({ state: 'visible' });
  const taskGlyphs = await status.locator('[class*="icon-[tabler--"]').count();
  if (taskGlyphs !== 0) {
    throw new Error(`expected no task status icon while running, found ${taskGlyphs}`);
  }
}

async function waitForInboxRowGone(page, expectedTitle, timeoutMs = 20_000) {
  const rows = page.locator('[data-testid^="inbox-notification-row-"]');
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const rowCount = await rows.count();
    let found = false;
    for (let i = 0; i < Math.min(rowCount, 40); i += 1) {
      const row = page.getByTestId(`inbox-notification-row-${i}`);
      const title = ((await row.getByTestId('inbox-notification-task-title').textContent()) ?? '').trim();
      if (title === expectedTitle) {
        found = true;
        break;
      }
    }
    if (!found) return;
    await sleep(200);
  }
  throw new Error(`timeout waiting for inbox row to disappear: ${JSON.stringify(expectedTitle)}`);
}

export async function runCancelTaskClearsRunning({ page }) {
  await page.getByTestId('nav-inbox-button').click();
  await page.getByTestId('inbox-view').waitFor({ state: 'visible' });

  const row = await findInboxRowByTitle(page, 'PR: pending');
  await assertRunningShowsOnlySpinner(row);
  await row.click();

  const trigger = page.getByTestId('inbox-task-status-trigger');
  await trigger.waitFor({ state: 'visible' });
  await trigger.click();

  await page.getByTestId('task-status-menu').waitFor({ state: 'visible' });
  await page.getByTestId('task-status-option-canceled').click();

  await waitForInboxRowGone(page, 'PR: pending');
}
