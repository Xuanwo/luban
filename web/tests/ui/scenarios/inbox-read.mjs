import { sleep, waitForDataAttribute } from '../lib/utils.mjs';

async function waitForLocatorCount(locator, expected, timeoutMs = 20_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const count = await locator.count();
    if (count === expected) return;
    await sleep(200);
  }
  const last = await locator.count();
  throw new Error(`timeout waiting for locator count; expected=${expected} got=${last}`);
}

export async function runInboxRead({ page }) {
  await page.getByTestId('nav-inbox-button').click();
  await page.getByTestId('inbox-view').waitFor({ state: 'visible' });

  const unreadRows = page.locator('[data-testid^="inbox-notification-row-"][data-read="false"]');
  const start = Date.now();
  while (Date.now() - start < 20_000) {
    if ((await unreadRows.count()) > 0) break;
    await sleep(250);
  }
  const unreadCount = await unreadRows.count();
  if (unreadCount === 0) {
    const allRows = page.locator('[data-testid^="inbox-notification-row-"]');
    const allCount = await allRows.count();
    const sample = [];
    for (let i = 0; i < Math.min(allCount, 5); i += 1) {
      const row = allRows.nth(i);
      sample.push({
        idx: i,
        read: await row.getAttribute('data-read'),
        text: ((await row.innerText()) ?? '').trim().replace(/\s+/g, ' ').slice(0, 200),
      });
    }
    throw new Error(`expected at least one unread inbox notification; rows=${allCount} sample=${JSON.stringify(sample)}`);
  }

  const rowTestId = await unreadRows.first().getAttribute('data-testid');
  if (!rowTestId) throw new Error('expected inbox notification row to have a data-testid');
  const row = page.getByTestId(rowTestId);
  await row.waitFor({ state: 'visible' });
  await waitForDataAttribute(row, 'data-read', 'false', 20_000);

  const unreadIndicator = row.locator('[data-testid="inbox-notification-unread-indicator"]');
  await waitForLocatorCount(unreadIndicator, 1, 20_000);

  await row.click();
  await row.waitFor({ state: 'visible' });
  await waitForDataAttribute(row, 'data-read', 'true', 20_000);
  await waitForLocatorCount(unreadIndicator, 0, 20_000);
}
