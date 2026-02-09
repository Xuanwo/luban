export async function runQueuedPrompts({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock task 1').first().click();

  const scrollContainer = page.getByTestId('chat-scroll-container');
  await scrollContainer.waitFor({ state: 'visible' });
  await scrollContainer.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });

  const runningTurn = page
    .getByTestId('agent-turn-card')
    .filter({ has: page.getByTestId('event-running-icon').first() })
    .first();
  await runningTurn.waitFor({ state: 'visible' });
  await runningTurn.getByTestId('event-running-icon').waitFor({ state: 'visible' });

  const workspace = page.getByTestId('task-workspace-panel');
  const chatInputCount = await workspace.getByTestId('chat-input').count();
  if (chatInputCount !== 0) {
    throw new Error(`expected no chat input in agents panel, got ${chatInputCount}`);
  }
  const queuedSectionCount = await page.getByTestId('queued-prompts').count();
  if (queuedSectionCount !== 0) {
    throw new Error(`expected no queued prompt composer section, got ${queuedSectionCount}`);
  }
}
