import { sleep } from '../lib/utils.mjs';

export async function runNewTaskModal({ page }) {
  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });
  await page.getByTestId('new-task-input').fill('Fix: programmatic agent-browser smoke');
  await sleep(500);
  await page.keyboard.press('Escape');
}

