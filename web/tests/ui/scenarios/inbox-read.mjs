import { waitForDataAttribute } from '../lib/utils.mjs';

export async function runInboxRead({ page }) {
  await page.getByTestId('nav-inbox-button').click();
  await page.getByTestId('inbox-view').waitFor({ state: 'visible' });

  const row0 = page.getByTestId('inbox-notification-row-0');
  await row0.waitFor({ state: 'visible' });
  await waitForDataAttribute(row0, 'data-read', 'false', 20_000);

  await row0.click();
  await row0.waitFor({ state: 'visible' });
  await waitForDataAttribute(row0, 'data-read', 'true', 20_000);
}

