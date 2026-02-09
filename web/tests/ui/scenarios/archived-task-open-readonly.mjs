export async function runArchivedTaskOpenReadonly({ page }) {
  const archivedTaskTitle = 'Done: completed successfully';

  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const archivedBanner = page.getByTestId('archived-task-banner');
  const archivedBannerVisibleBefore = await archivedBanner.isVisible().catch(() => false);
  if (archivedBannerVisibleBefore) {
    throw new Error('expected archived banner to be hidden before opening archived task');
  }

  await page.getByTestId('task-view-tab-all').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const doneGroupToggle = page.getByTestId('task-group-done');
  await doneGroupToggle.waitFor({ state: 'visible' });

  const archivedRow = page
    .getByTestId('task-list-view')
    .locator('div.group', { hasText: archivedTaskTitle })
    .first();

  if (!(await archivedRow.isVisible().catch(() => false))) {
    await doneGroupToggle.click();
    await archivedRow.waitFor({ state: 'visible' });
  }

  await archivedRow.scrollIntoViewIfNeeded();
  await archivedRow.click();

  const bannerVisible = await archivedBanner
    .waitFor({ state: 'visible', timeout: 1500 })
    .then(() => true)
    .catch(() => false);

  if (bannerVisible) return;

  const workspacePanel = page.getByTestId('task-workspace-panel');
  await workspacePanel.waitFor({ state: 'visible' });
  const chatInputCount = await workspacePanel.getByTestId('chat-input').count();
  if (chatInputCount !== 0) {
    throw new Error('expected archived task to be read-only (no editable chat input)');
  }
}
