export async function runSettingsPanel({ page }) {
  await page.getByTestId('workspace-switcher-button').click();
  await page.getByTestId('open-settings-button').click();
  await page.getByTestId('settings-panel').waitFor({ state: 'visible' });
}

