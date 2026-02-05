import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';

const require = createRequire(import.meta.url);
const { PNG } = require('pngjs');

function writeTempPng() {
  const png = new PNG({ width: 2, height: 2 });
  for (let y = 0; y < png.height; y += 1) {
    for (let x = 0; x < png.width; x += 1) {
      const idx = (png.width * y + x) << 2;
      png.data[idx] = 0;
      png.data[idx + 1] = 128;
      png.data[idx + 2] = 255;
      png.data[idx + 3] = 255;
    }
  }

  const out = path.join(os.tmpdir(), `luban-ui-activity-attachment-${Date.now()}.png`);
  fs.writeFileSync(out, PNG.sync.write(png));
  return out;
}

export async function runActivityAttachments({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByTestId('task-view-tab-backlog').click();
  await page.getByText('Mock task 2').first().click();

  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });

  const pngPath = writeTempPng();
  try {
    await page.getByTestId('chat-attach-input').setInputFiles(pngPath);
    await page.getByTestId('chat-attachment-tile').first().waitFor({ state: 'visible' });

    const message = `Attachment smoke ${Date.now()}`;
    await page.getByTestId('chat-input').fill(message);
    await page.getByTestId('chat-send').click();

    await page.getByTestId('activity-user-message-content').filter({ hasText: message }).first().waitFor({ state: 'visible' });
    await page.getByTestId('activity-user-attachment').first().waitFor({ state: 'visible' });
  } finally {
    fs.rmSync(pngPath, { force: true });
  }
}
