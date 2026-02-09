import { sleep } from '../lib/utils.mjs';

export async function runEscapeDoesNotLeakFromChatInput({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('PR: pending').first().click();

  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  const hint = page.getByTestId('esc-cancel-hint');
  const beforeCount = await hint.count();
  if (beforeCount !== 0) {
    throw new Error(`expected no esc cancel hint before test, got ${beforeCount}`);
  }

  const workspace = page.getByTestId('task-workspace-panel');
  const chatInputCount = await workspace.getByTestId('chat-input').count();
  if (chatInputCount !== 0) {
    throw new Error(`expected no chat input in agents panel, got ${chatInputCount}`);
  }
  await page.getByTestId('chat-scroll-container').click();
  await page.keyboard.press('Escape');

  await sleep(150);

  const afterCount = await hint.count();
  if (afterCount !== 0) {
    throw new Error(`expected escape in chat input not to show esc cancel hint, got ${afterCount}`);
  }
}
