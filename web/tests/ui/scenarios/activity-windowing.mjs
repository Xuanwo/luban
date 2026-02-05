export async function runActivityWindowing({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock: Long conversation').first().click();

  const scrollContainer = page.getByTestId('chat-scroll-container');
  await scrollContainer.waitFor({ state: 'visible' });

  await page.getByText('Assistant message 319').first().waitFor({ state: 'visible' });
  const distanceToBottom = await scrollContainer.evaluate((el) => el.scrollHeight - el.scrollTop - el.clientHeight);
  if (distanceToBottom > 5) {
    throw new Error(`expected to start at the latest activity (near bottom), got distanceToBottom=${distanceToBottom}`);
  }

  const userCount = await page.getByTestId('activity-user-message-content').count();
  const agentCount = await page.getByTestId('activity-agent-message-content').count();
  const total = userCount + agentCount;
  if (total > 200) {
    throw new Error(`expected windowed activity stream to keep DOM bounded, got ${total} message nodes`);
  }
}
