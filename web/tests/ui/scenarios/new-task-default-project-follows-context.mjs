import { sleep, waitForDataAttribute } from '../lib/utils.mjs';

export async function runNewTaskDefaultProjectFollowsContext({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const title = `Context project default ${Math.random().toString(16).slice(2)}`;

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  const projectSelector = page.getByTestId('new-task-project-selector');
  await projectSelector.waitFor({ state: 'visible' });
  await waitForDataAttribute(projectSelector, 'data-selected-project-id', 'mock-project-1', 10_000);

  await projectSelector.click();
  await page.getByText('Mock Local Project').click();
  await waitForDataAttribute(projectSelector, 'data-selected-project-id', 'mock-project-2', 10_000);

  await page.getByTestId('new-task-input').fill(title);
  await sleep(150);
  await page.getByTestId('new-task-submit-button').click();

  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });
  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });
  await projectSelector.waitFor({ state: 'visible' });
  await waitForDataAttribute(projectSelector, 'data-selected-project-id', 'mock-project-1', 10_000);

  await page.getByTestId('new-task-close-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('nav-inbox-button').click();
  await page.getByTestId('inbox-view').waitFor({ state: 'visible' });
}
