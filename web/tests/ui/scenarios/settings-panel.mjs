export async function runSettingsPanel({ page }) {
  await page.getByTestId('workspace-switcher-button').click();
  await page.getByTestId('open-settings-button').click();
  await page.getByTestId('settings-panel').waitFor({ state: 'visible' });

  await page.getByTestId('settings-terminal-font').fill('Primary Mono Test, Fallback NF Test');
  const terminalPreviewFontFamily = await page
    .getByTestId('settings-terminal-font-preview')
    .evaluate((el) => el.style.fontFamily);
  if (!/"Primary Mono Test"\s*,\s*"Fallback NF Test"\s*,\s*monospace/i.test(terminalPreviewFontFamily)) {
    throw new Error(`expected terminal preview font fallback chain, got "${terminalPreviewFontFamily}"`);
  }

  await page.getByRole('button', { name: 'Integrations' }).click();
  await page.getByRole('button', { name: 'Telegram' }).click();

  await page.getByTestId('telegram-bot-token-input').fill('mock_token');
  await page.getByTestId('telegram-bot-token-save').click();

  await page.getByTestId('telegram-pair-generate').click();
  const url = (await page.getByTestId('telegram-pair-url').inputValue()).trim();
  if (!url.startsWith('https://t.me/')) {
    throw new Error(`expected telegram pair url to start with https://t.me/, got "${url}"`);
  }

  await page.getByRole('button', { name: 'Back' }).click();
  await page.getByTestId('settings-panel').waitFor({ state: 'hidden' });
}
