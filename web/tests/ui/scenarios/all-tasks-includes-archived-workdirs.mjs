import { waitForDataAttribute } from '../lib/utils.mjs';

async function createTaskInNewWorkdir(page, { title }) {
  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  const workdirSelector = page.getByTestId('new-task-workdir-selector');
  await workdirSelector.waitFor({ state: 'visible' });
  await workdirSelector.click();
  await page.getByRole('menuitem').filter({ hasText: 'Create new...' }).first().click();
  await waitForDataAttribute(workdirSelector, 'data-selected-workdir-id', '-1', 10_000);

  await page.getByTestId('new-task-input').fill(title);
  await page.getByTestId('new-task-submit-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });
}

async function setTaskStatusToDoneInAllTasksView(page, { title }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByTestId('task-view-tab-all').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const doneGroupToggle = page.getByTestId('task-group-done');
  await doneGroupToggle.waitFor({ state: 'visible' });

  const knownDoneRow = page
    .getByTestId('task-list-view')
    .locator('div.group', { hasText: 'Done: completed successfully' })
    .first();
  if (!(await knownDoneRow.isVisible().catch(() => false))) {
    await doneGroupToggle.click();
    await knownDoneRow.waitFor({ state: 'visible' });
  }

  const row = page.getByTestId('task-list-view').locator('div.group', { hasText: title }).first();
  await row.waitFor({ state: 'visible' });
  await row.scrollIntoViewIfNeeded();

  const trigger = row.locator('button[title^="Status:"]').first();
  await trigger.waitFor({ state: 'visible' });
  await trigger.click();
  await page.getByTestId('task-status-option-done').click();

  const doneRow = page.getByTestId('task-list-view').locator('div.group', { hasText: title }).first();
  await doneRow.waitFor({ state: 'visible' });
  const doneTrigger = doneRow.locator('button[title^="Status:"]').first();
  await waitForDataAttribute(doneTrigger, 'title', 'Status: Done', 20_000);
}

export async function runAllTasksIncludesArchivedWorkdirs({ page }) {
  const title = 'Smoke: archived workdir stays visible';

  await createTaskInNewWorkdir(page, { title });
  await setTaskStatusToDoneInAllTasksView(page, { title });
}
