export async function runAgentRunnerIcons({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByTestId('task-view-tab-backlog').click();
  await page.getByText('Mock task 2').first().click();

  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  const selector = page.getByTestId('agent-selector');
  const overlay = page.getByTestId('agent-selector-overlay');
  await selector.waitFor({ state: 'visible' });

  if (await overlay.count()) {
    await overlay.click({ position: { x: 1, y: 1 } });
  }

  await selector.locator('[data-provider-id="openai"]').first().waitFor({ state: 'visible' });

  await selector.click();
  await page.getByTestId('agent-runner-option-amp').click();
  await overlay.click({ position: { x: 1, y: 1 } });
  await selector.locator('img[data-agent-runner-icon="amp"]').first().waitFor({ state: 'visible' });

  await selector.click();
  await page.getByTestId('agent-runner-option-claude').click();
  await overlay.click({ position: { x: 1, y: 1 } });
  await selector.locator('[data-provider-id="anthropic"]').first().waitFor({ state: 'visible' });

  await selector.click();
  await page.getByTestId('agent-runner-option-codex').click();
  await overlay.click({ position: { x: 1, y: 1 } });
  await selector.locator('[data-provider-id="openai"]').first().waitFor({ state: 'visible' });
}
