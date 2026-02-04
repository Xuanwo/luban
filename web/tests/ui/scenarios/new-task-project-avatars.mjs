export async function runNewTaskProjectAvatars({ page }) {
  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  const projectSelector = page.getByTestId('new-task-project-selector');
  await projectSelector.waitFor({ state: 'visible' });
  await projectSelector.click();

  const gitProject = page.getByTestId('new-task-project-option-mock-project-1');
  await gitProject.waitFor({ state: 'visible' });

  const img = gitProject.locator('img').first();
  await img.waitFor({ state: 'visible' });

  const src = await img.getAttribute('src');
  if (!src) throw new Error('missing project avatar src');
  if (!src.startsWith('data:image/svg+xml,')) {
    throw new Error(`expected mock avatar data URL, got: ${src.slice(0, 64)}`);
  }

  // Close the dropdown before clicking the modal close button (Radix captures pointer events while open).
  await gitProject.click();

  await page.getByTestId('new-task-close-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });
}
