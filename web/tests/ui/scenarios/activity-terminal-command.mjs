export async function runActivityTerminalCommand({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByTestId('task-view-tab-backlog').click();
  await page.getByText('Mock task 2').first().click();

  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  await page.getByTestId('chat-mode-toggle').click();

  await page.getByTestId('shell-composer').waitFor({ state: 'visible' });
  await page.getByTestId('pty-terminal').waitFor({ state: 'visible' });

  await page.getByTestId('chat-mode-toggle').click();
  await page.getByTestId('chat-input').waitFor({ state: 'visible' });
}
