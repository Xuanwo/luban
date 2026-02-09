export async function runActivityAttachments({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByTestId('task-view-tab-backlog').click();
  await page.getByText('Mock task 2').first().click();

  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-tab-agents').click();
  const workspace = page.getByTestId('task-workspace-panel');
  const attachInputCount = await workspace.getByTestId('chat-attach-input').count();
  if (attachInputCount !== 0) {
    throw new Error(`expected no attachment input in agents panel, got ${attachInputCount}`);
  }
}
