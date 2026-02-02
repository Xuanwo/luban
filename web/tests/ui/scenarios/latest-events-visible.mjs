import { waitForLocatorCount } from '../lib/utils.mjs';

export async function runLatestEventsVisible({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock task 1').first().click();

  const scrollContainer = page.getByTestId('chat-scroll-container');
  await scrollContainer.waitFor({ state: 'visible' });
  await scrollContainer.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });

  const pickEventLocator = async () => {
    const activity = page.getByTestId('activity-event');
    if ((await activity.count()) > 0) return activity;
    return page.getByTestId('conversation-event');
  };

  const eventLocator = await pickEventLocator();
  const progressEvents = eventLocator.filter({ hasText: 'Progress update' });
  await waitForLocatorCount(progressEvents, 3, 20_000);

  await eventLocator.filter({ hasText: 'Progress update 1' }).first().waitFor({ state: 'visible' });
  await eventLocator.filter({ hasText: 'Progress update 2' }).first().waitFor({ state: 'visible' });
  const runningRow = eventLocator.filter({ hasText: 'Progress update 3' }).first();
  await runningRow.waitFor({ state: 'visible' });
  await runningRow.getByTestId('event-running-icon').waitFor({ state: 'visible' });
}
