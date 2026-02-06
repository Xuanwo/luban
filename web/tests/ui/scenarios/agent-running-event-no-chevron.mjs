export async function runAgentRunningEventNoChevron({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock task 1').first().click();

  const scrollContainer = page.getByTestId('chat-scroll-container');
  await scrollContainer.waitFor({ state: 'visible' });
  await scrollContainer.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });

  const latestTurn = page
    .getByTestId('agent-turn-card')
    .filter({ has: page.getByTestId('event-running-icon').first() })
    .first();
  await latestTurn.waitFor({ state: 'visible' });
  await latestTurn.scrollIntoViewIfNeeded();

  await latestTurn.getByTestId('agent-turn-toggle').click();

  const runningRow = latestTurn.getByTestId('agent-turn-event').filter({ hasText: 'Progress update 3' }).first();
  await runningRow.waitFor({ state: 'visible' });
  await runningRow.getByTestId('event-running-icon').waitFor({ state: 'visible' });

  const eventButton = runningRow.locator('button').first();
  await eventButton.waitFor({ state: 'visible' });

  const lastChildTestId = await eventButton.evaluate((el) => el.lastElementChild?.getAttribute('data-testid') ?? null);
  if (lastChildTestId !== 'activity-event-trailing') {
    throw new Error(`expected running event row to end with activity-event-trailing, got ${JSON.stringify(lastChildTestId)}`);
  }

  const trailing = eventButton.getByTestId('activity-event-trailing');
  const trailingSvgCount = await trailing.locator('svg').count();
  if (trailingSvgCount !== 0) {
    throw new Error(`expected activity-event-trailing to contain no svg, got ${trailingSvgCount}`);
  }
}
