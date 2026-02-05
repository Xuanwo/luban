import { sleep } from '../lib/utils.mjs';

async function waitForInputContains(page, testId, needle, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const value = await page.getByTestId(testId).inputValue();
    if (value.includes(needle)) return value;
    await sleep(100);
  }
  const value = await page.getByTestId(testId).inputValue();
  throw new Error(`expected ${testId} to include "${needle}", got: ${value.slice(0, 64)}`);
}

async function waitForInputTrimmedEmpty(page, testId, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const value = await page.getByTestId(testId).inputValue();
    if (value.trim() === '') return value;
    await sleep(100);
  }
  const value = await page.getByTestId(testId).inputValue();
  throw new Error(`expected ${testId} to be empty, got: ${value.slice(0, 64)}`);
}

export async function runNewTaskDrafts({ page }) {
  const title = `Draft smoke ${Date.now()}`;

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  await page.getByTestId('new-task-workdir-selector').click();
  await page.getByRole('menuitem').filter({ hasText: 'Create new...' }).first().click();

  await page.getByTestId('new-task-input').fill(title);
  await page.getByTestId('new-task-save-draft-button').waitFor({ state: 'visible' });
  await page.getByTestId('new-task-save-draft-button').click();

  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('nav-drafts-button').waitFor({ state: 'visible' });
  await page.getByTestId('nav-drafts-button').click();
  await page.getByTestId('new-task-drafts-dialog').waitFor({ state: 'visible' });

  await page.getByTestId('new-task-drafts-open-0').waitFor({ state: 'visible' });
  await page.getByTestId('new-task-drafts-open-0').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  await waitForInputContains(page, 'new-task-input', title, 10_000);

  await page.keyboard.press('Escape');
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  await waitForInputContains(page, 'new-task-input', title, 10_000);

  // Close explicitly discards (clears stash). Use Escape for stashing.
  await page.keyboard.press('Escape');
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('sidebar-project-mock-project-2').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  await waitForInputContains(page, 'new-task-input', title, 10_000);
  await page.getByTestId('new-task-input').fill('');
  await page.getByTestId('new-task-close-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });
  await waitForInputTrimmedEmpty(page, 'new-task-input', 10_000);
  await page.getByTestId('new-task-close-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('nav-drafts-button').click();
  await page.getByTestId('new-task-drafts-dialog').waitFor({ state: 'visible' });

  await page.getByTestId('new-task-drafts-delete-0').waitFor({ state: 'visible' });
  await page.getByTestId('new-task-drafts-delete-0').click();
  await page.getByText('No drafts saved.').waitFor({ state: 'visible' });
  await page.keyboard.press('Escape');
  await page.getByTestId('new-task-drafts-dialog').waitFor({ state: 'hidden' });

  await page.getByTestId('nav-drafts-button').waitFor({ state: 'hidden' });
}
