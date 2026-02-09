export async function runActivityTerminalCommand({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByTestId('task-view-tab-backlog').click();
  await page.getByText('Mock task 2').first().click();

  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-tab-agents').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-tab-terminal').click();
  await page.getByTestId('pty-terminal').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-tab-agents').click();
  const workspace = page.getByTestId('task-workspace-panel');
  const chatInputCount = await workspace.getByTestId('chat-input').count();
  if (chatInputCount !== 0) {
    throw new Error(`expected no chat input in agents panel, got ${chatInputCount}`);
  }
}
