export async function runTurnDurationEstimate({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByText('Mock: Turn states').first().click();

  const firstTurn = page.getByTestId('agent-turn-card').first();
  await firstTurn.waitFor({ state: 'visible' });
  await firstTurn.scrollIntoViewIfNeeded();

  const toggle = firstTurn.getByTestId('agent-turn-toggle');
  await toggle.waitFor({ state: 'visible' });
  await toggle.click();

  const firstEvent = firstTurn.getByTestId('agent-turn-event').first();
  await firstEvent.waitFor({ state: 'visible' });
  const duration = firstEvent.getByTestId('activity-event-duration');
  await duration.waitFor({ state: 'visible' });

  const durationText = ((await duration.textContent()) ?? '').trim();
  if (durationText === '< 1s') {
    throw new Error('expected completed turn event to have a non-trivial duration estimate (not "< 1s")');
  }
  if (!/^(\d+s|\d+m \d+s|\d+h \d+m \d+s)$/.test(durationText)) {
    throw new Error(`expected duration to look like a formatted duration, got "${durationText}"`);
  }
}
