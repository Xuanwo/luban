export async function runNoRightSidebar({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByTestId('task-view-tab-backlog').click();
  await page.getByText('Mock task 2').first().click();

  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-panel').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-tab-agents').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-tab-changes').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-tab-preview').waitFor({ state: 'visible' });
  await page.getByTestId('task-workspace-tab-terminal').waitFor({ state: 'visible' });
}
