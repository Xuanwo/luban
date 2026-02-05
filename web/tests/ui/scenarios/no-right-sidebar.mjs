export async function runNoRightSidebar({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByTestId('task-view-tab-backlog').click();
  await page.getByText('Mock task 2').first().click();

  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  const terminalTabCount = await page.getByTestId('right-sidebar-tab-terminal').count();
  if (terminalTabCount !== 0) {
    throw new Error('expected right sidebar to be removed (terminal tab still present)');
  }

  const changesTabCount = await page.getByTestId('right-sidebar-tab-changes').count();
  if (changesTabCount !== 0) {
    throw new Error('expected right sidebar to be removed (changes tab still present)');
  }

  const resizerCount = await page.getByTitle('Resize terminal').count();
  if (resizerCount !== 0) {
    throw new Error('expected right sidebar to be removed (resizer still present)');
  }
}
