import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';

const require = createRequire(import.meta.url);
const { PNG } = require('pngjs');

import { sleep, waitForDataAttribute, waitForLocatorCount } from '../lib/utils.mjs';

function writeTempPng() {
  const png = new PNG({ width: 2, height: 2 });
  for (let y = 0; y < png.height; y += 1) {
    for (let x = 0; x < png.width; x += 1) {
      const idx = (png.width * y + x) << 2;
      png.data[idx] = 255;
      png.data[idx + 1] = 0;
      png.data[idx + 2] = 0;
      png.data[idx + 3] = 255;
    }
  }

  const out = path.join(os.tmpdir(), `luban-ui-new-task-attachment-${Date.now()}.png`);
  fs.writeFileSync(out, PNG.sync.write(png));
  return out;
}

export async function runNewTaskModal({ page }) {
  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  const projectSelector = page.getByTestId('new-task-project-selector');
  await projectSelector.waitFor({ state: 'visible' });
  await waitForDataAttribute(projectSelector, 'data-selected-project-id', 'mock-project-1', 10_000);

  const workdirSelector = page.getByTestId('new-task-workdir-selector');
  await workdirSelector.waitFor({ state: 'visible' });
  await waitForDataAttribute(workdirSelector, 'data-selected-workdir-id', '1', 10_000);

  // Changing the workdir and stashing should not persist that selection across opens.
  await workdirSelector.click();
  await page.getByRole('menuitem').filter({ hasText: 'feat-ui' }).first().click();
  await waitForDataAttribute(workdirSelector, 'data-selected-workdir-id', '2', 10_000);

  await page.getByTestId('new-task-input').fill('Fix: programmatic agent-browser smoke');
  await page.keyboard.press('Escape');
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });
  await waitForDataAttribute(workdirSelector, 'data-selected-workdir-id', '1', 10_000);

  // Clear stash to avoid interfering with later scenarios.
  await page.getByTestId('new-task-input').fill('');
  await page.keyboard.press('Escape');
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });

  await page.getByTestId('new-task-button').click();
  await page.getByTestId('new-task-modal').waitFor({ state: 'visible' });

  const pngPath = writeTempPng();
  try {
    await page.getByTestId('new-task-attachment-input').setInputFiles(pngPath);
    await page.getByTestId('new-task-attachment-tile').first().waitFor({ state: 'visible' });

    await waitForLocatorCount(page.getByTestId('new-task-attachment-tile'), 1, 10_000);
    await page.getByTestId('new-task-attachment-tile').first().hover();
    await page.getByTestId('new-task-attachment-remove').first().click();
    await waitForLocatorCount(page.getByTestId('new-task-attachment-tile'), 0, 10_000);
  } finally {
    fs.rmSync(pngPath, { force: true });
  }
  await sleep(500);
  const closeButton = page.getByTestId('new-task-close-button');
  await closeButton.waitFor({ state: 'attached', timeout: 10_000 });
  const closeVisible = await closeButton.isVisible();
  if (!closeVisible) {
    const modalVisible = await page.getByTestId('new-task-modal').isVisible().catch(() => false);
    throw new Error(`new-task-close-button not visible (modalVisible=${modalVisible})`);
  }
  await closeButton.click({ timeout: 10_000 });
  await page.getByTestId('new-task-modal').waitFor({ state: 'hidden' });
}
