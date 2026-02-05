export async function runProjectAllTasksView({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const archivedTaskTitle = 'Done: completed successfully';
  const backlogTaskTitle = 'Mock task 2';
  const activeTaskTitle = 'Iterating: queue paused';
  if ((await page.getByText(archivedTaskTitle).count()) !== 0) {
    throw new Error('expected done tasks not to appear in active view');
  }
  if ((await page.getByText(backlogTaskTitle).count()) !== 0) {
    throw new Error('expected backlog tasks not to appear in active view');
  }

  await page.getByTestId('task-view-tab-backlog').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByText(backlogTaskTitle).first().waitFor({ state: 'visible' });
  if ((await page.getByText(activeTaskTitle).count()) !== 0) {
    throw new Error('expected active tasks not to appear in backlog view');
  }

  await page.getByTestId('task-view-tab-all').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByTestId('task-group-done').click();

  const row = page.getByText(archivedTaskTitle).first();
  await row.waitFor({ state: 'attached' });
  await row.scrollIntoViewIfNeeded();
  await row.waitFor({ state: 'visible' });

  await page.getByTestId('task-view-tab-active').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  if ((await page.getByText(archivedTaskTitle).count()) !== 0) {
    throw new Error('expected done tasks not to appear after switching back to active view');
  }
  if ((await page.getByText(backlogTaskTitle).count()) !== 0) {
    throw new Error('expected backlog tasks not to appear after switching back to active view');
  }
}
