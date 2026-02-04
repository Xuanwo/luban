export async function runRightSidebarNoContextTab({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock task 2').first().click();
  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  await page.getByTestId('right-sidebar-tab-terminal').waitFor({ state: 'visible' });
  await page.getByTestId('right-sidebar-tab-changes').waitFor({ state: 'visible' });

  const ctxCount = await page.getByTestId('right-sidebar-tab-context').count();
  if (ctxCount !== 0) {
    throw new Error('expected Context tab to be removed from the right sidebar');
  }
}

