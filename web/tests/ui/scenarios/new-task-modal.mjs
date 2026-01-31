import { sleep, waitForDataAttribute } from '../lib/utils.mjs';

export async function runNewTaskModal({ page }) {
  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  const projectSelector = page.getByTestId('new-task-project-selector');
  await projectSelector.waitFor({ state: 'visible' });
  await waitForDataAttribute(projectSelector, 'data-selected-project-id', 'mock-project-1', 10_000);

  const workdirSelector = page.getByTestId('new-task-workdir-selector');
  await workdirSelector.waitFor({ state: 'visible' });
  await waitForDataAttribute(workdirSelector, 'data-selected-workdir-id', '1', 10_000);

  await page.getByTestId('new-task-input').fill('Fix: programmatic agent-browser smoke');
  await sleep(500);
  await page.keyboard.press('Escape');
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });
}
