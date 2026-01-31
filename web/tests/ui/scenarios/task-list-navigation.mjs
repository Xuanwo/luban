import { sleep, waitForDataAttribute } from '../lib/utils.mjs';

export async function runTaskListNavigation({ page }) {
  const settingsPanel = page.getByTestId('settings-panel');
  if (await settingsPanel.isVisible()) {
    await settingsPanel.getByText('Back').click();
    await settingsPanel.waitFor({ state: 'hidden' });
  }

  const title = `Persist task after navigation ${Math.random().toString(16).slice(2)}`;

  // Anchor the active workdir so the new task modal defaults are stable.
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock task 1').first().click();
  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  const projectSelector = page.getByTestId('new-task-project-selector');
  await projectSelector.waitFor({ state: 'visible' });
  await waitForDataAttribute(projectSelector, 'data-selected-project-id', 'mock-project-1', 10_000);

  const workdirSelector = page.getByTestId('new-task-workdir-selector');
  await workdirSelector.waitFor({ state: 'visible' });
  {
    const deadline = Date.now() + 10_000;
    // Wait for the selector to finish initializing (non-empty id).
    while (Date.now() < deadline) {
      const value = await workdirSelector.getAttribute('data-selected-workdir-id');
      if (value != null && value.trim().length > 0) break;
      await sleep(100);
    }
  }

  await page.getByTestId('new-task-input').fill(title);
  await sleep(150);
  await page.getByTestId('new-task-submit-button').click();

  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });
  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  await page.getByTestId('nav-inbox-button').click();
  await page.getByTestId('inbox-view').waitFor({ state: 'visible' });

  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const row = page.getByText(title).first();
  await row.waitFor({ state: 'attached' });
  await row.scrollIntoViewIfNeeded();
  await row.waitFor({ state: 'visible' });
}
