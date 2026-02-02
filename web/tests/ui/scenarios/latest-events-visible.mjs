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

  const progressUpdate1 = eventLocator.filter({ hasText: 'Progress update 1' }).first();
  await progressUpdate1.waitFor({ state: 'visible' });
  await progressUpdate1.scrollIntoViewIfNeeded();

  const avatar = progressUpdate1.getByTestId('event-avatar');
  const text = progressUpdate1.getByTestId('event-text');
  await avatar.waitFor({ state: 'visible' });
  await text.waitFor({ state: 'visible' });

  const avatarBox = await avatar.boundingBox();
  const textBox = await text.boundingBox();
  if (!avatarBox) throw new Error('missing avatar bounding box');
  if (!textBox) throw new Error('missing text bounding box');

  const avatarCenterY = avatarBox.y + avatarBox.height / 2;
  const textCenterY = textBox.y + textBox.height / 2;
  const delta = Math.abs(avatarCenterY - textCenterY);
  const tolerance = 1.5;
  if (delta > tolerance) {
    throw new Error(`expected avatar/text vertical alignment within ${tolerance}px, got delta=${delta}px`);
  }

  await eventLocator.filter({ hasText: 'Progress update 2' }).first().waitFor({ state: 'visible' });
  const runningRow = eventLocator.filter({ hasText: 'Progress update 3' }).first();
  await runningRow.waitFor({ state: 'visible' });
  await runningRow.getByTestId('event-running-icon').waitFor({ state: 'visible' });
  await waitForLocatorCount(runningRow.getByTestId('event-timestamp'), 0, 5_000);
}
