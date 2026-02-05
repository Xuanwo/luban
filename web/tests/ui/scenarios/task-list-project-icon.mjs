export async function runTaskListProjectIcon({ page }) {
  const settingsPanel = page.getByTestId('settings-panel');
  if (await settingsPanel.isVisible()) {
    await settingsPanel.getByText('Back').click();
    await settingsPanel.waitFor({ state: 'hidden' });
  }

  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  const icon = page.getByTestId('task-list-project-icon');
  await icon.waitFor({ state: 'visible' });

  const tagName = await icon.evaluate((el) => el.tagName);
  if (tagName !== 'IMG') {
    throw new Error(`expected task list project icon to be an IMG, got: ${tagName}`);
  }

  const box = await icon.boundingBox();
  if (!box) throw new Error('missing task list project icon bounding box');
  if (Math.round(box.width) !== 14 || Math.round(box.height) !== 14) {
    throw new Error(`expected task list project icon to be 14x14, got ${box.width}x${box.height}`);
  }
}

