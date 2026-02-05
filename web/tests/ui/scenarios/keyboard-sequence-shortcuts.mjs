import { waitForDataAttribute } from '../lib/utils.mjs';

export async function runKeyboardSequenceShortcuts({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  // Ensure the task list container owns focus so key presses have a stable target.
  await page.getByTestId('task-list-view').click();

  // C: open new task modal.
  await page.keyboard.press('c');
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });
  await page.keyboard.press('Escape');
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  // G -> I: go to inbox.
  await page.keyboard.press('g');
  await page.keyboard.press('i');
  await page.getByTestId('inbox-view').waitFor({ state: 'visible' });

  // G -> A: go back to active tasks.
  await page.keyboard.press('g');
  await page.keyboard.press('a');
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  // G -> B: switch to backlog view.
  await page.keyboard.press('g');
  await page.keyboard.press('b');
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock task 2').first().waitFor({ state: 'visible' });

  // G -> E: switch to all tasks view.
  await page.keyboard.press('g');
  await page.keyboard.press('e');
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  // S: open status menu for hovered task, then numeric selection.
  const trigger = page.getByTestId('task-status-selector-1-1');
  await trigger.waitFor({ state: 'visible' });

  const row = page.getByTestId('task-list-view').locator('div.group', { hasText: 'Mock task 1' }).first();
  await row.waitFor({ state: 'visible' });
  await row.hover();
  await page.getByTestId('task-list-view').focus();

  await page.keyboard.press('s');
  await page.getByTestId('task-status-command-menu').waitFor({ state: 'visible' });
  await page.keyboard.press('3'); // Iterating

  await page.getByTestId('task-status-command-menu').waitFor({ state: 'hidden' });
  await waitForDataAttribute(trigger, 'title', 'Status: Iterating', 20_000);
}
