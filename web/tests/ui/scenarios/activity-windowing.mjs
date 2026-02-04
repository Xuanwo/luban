export async function runActivityWindowing({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock: Long conversation').first().click();

  const scrollContainer = page.getByTestId('chat-scroll-container');
  await scrollContainer.waitFor({ state: 'visible' });

  await page.getByText('Assistant message 0').first().waitFor({ state: 'visible' });

  const userCount = await page.getByTestId('activity-user-message-content').count();
  const agentCount = await page.getByTestId('activity-agent-message-content').count();
  const total = userCount + agentCount;
  if (total > 200) {
    throw new Error(`expected windowed activity stream to keep DOM bounded, got ${total} message nodes`);
  }

  await scrollContainer.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });

  await page.getByText('Assistant message 319').first().waitFor({ state: 'visible' });
}

