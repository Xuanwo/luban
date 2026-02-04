import { waitForLocatorCount } from '../lib/utils.mjs';

export async function runTaskSummariesEventsRefresh({ page, baseUrl }) {
  if (baseUrl) {
    await page.goto(baseUrl, { waitUntil: 'networkidle' });
    await page.getByTestId('nav-sidebar').waitFor({ state: 'visible' });
  }

  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const pill = page.getByTestId('task-agent-pill-2-10');
  await pill.waitFor({ state: 'attached' });
  await pill.scrollIntoViewIfNeeded();
  await pill.waitFor({ state: 'visible' });

  const title = await pill.getAttribute('title');
  if (title !== 'Codex: awaiting_ack') {
    throw new Error(`expected awaiting ack pill title to be Codex: awaiting_ack, got: ${title}`);
  }

  await page.getByText('Todo: awaiting ack').first().click();

  await waitForLocatorCount(pill, 0, 20_000);
}
