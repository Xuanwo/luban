export async function runProjectArchiveMenu({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const archivedTaskTitle = 'Done: completed successfully';
  if ((await page.getByText(archivedTaskTitle).count()) !== 0) {
    throw new Error('expected archived tasks not to appear in active view');
  }

  await page.getByTestId('sidebar-project-mock-project-1-menu').click();
  await page.getByTestId('sidebar-project-mock-project-1-open-archive').click();

  await page.getByTestId('task-view-tab-archive').waitFor({ state: 'visible' });

  const row = page.getByText(archivedTaskTitle).first();
  await row.waitFor({ state: 'attached' });
  await row.scrollIntoViewIfNeeded();
  await row.waitFor({ state: 'visible' });

  await page.getByTestId('task-view-tab-active').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  if ((await page.getByText(archivedTaskTitle).count()) !== 0) {
    throw new Error('expected archived tasks not to appear after switching back to active view');
  }
}
