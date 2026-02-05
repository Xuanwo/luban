export async function runNewTaskDoubleSubmitNoDuplicate({ page }) {
  const title = `E2E no duplicate task ${Date.now()}`;

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  await page.getByTestId('new-task-input').fill(title);
  await page.getByTestId('new-task-submit-button').click({ clickCount: 2 });

  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByTestId('task-view-tab-all').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const matches = page.getByTestId('task-list-view').locator('div.group', { hasText: title });
  await matches.first().waitFor({ state: 'visible' });

  const count = await matches.count();
  if (count !== 1) {
    throw new Error(`expected exactly one task row for "${title}", saw ${count}`);
  }
}
