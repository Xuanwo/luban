import { sleep } from '../lib/utils.mjs';

async function inboxRowTaskTitle(row) {
  const title = row.getByTestId('inbox-notification-task-title').first();
  await title.waitFor({ state: 'visible' });
  return ((await title.textContent()) ?? '').trim();
}

async function collectInboxTitles({ page, limit = 10 }) {
  const titles = [];
  for (let i = 0; i < limit; i += 1) {
    const row = page.getByTestId(`inbox-notification-row-${i}`);
    if ((await row.count()) === 0) break;
    await row.waitFor({ state: 'visible' });
    titles.push(await inboxRowTaskTitle(row));
  }
  return titles;
}

export async function runInboxSortStability({ page }) {
  await page.getByTestId('nav-inbox-button').click();
  await page.getByTestId('inbox-view').waitFor({ state: 'visible' });

  const row0 = page.getByTestId('inbox-notification-row-0');
  const row1 = page.getByTestId('inbox-notification-row-1');
  await row0.waitFor({ state: 'visible' });
  await row1.waitFor({ state: 'visible' });

  const row0Title = await inboxRowTaskTitle(row0);
  const row1Title = await inboxRowTaskTitle(row1);
  if (!row0Title || !row1Title) {
    throw new Error('expected inbox rows to have non-empty titles');
  }
  if (row0Title === row1Title) {
    throw new Error('expected inbox rows 0 and 1 to have different titles for a stable ordering assertion');
  }

  await row1.click();
  const headerTitle = page.getByTestId('task-header-title').first();
  await headerTitle.waitFor({ state: 'visible' });
  const start = Date.now();
  while (Date.now() - start < 20_000) {
    const text = ((await headerTitle.textContent()) ?? '').trim();
    if (text === row1Title) break;
    await sleep(50);
  }
  const selectedTitle = ((await headerTitle.textContent()) ?? '').trim();
  if (selectedTitle !== row1Title) {
    throw new Error(`expected inbox selection to switch before sending message; got "${selectedTitle}" expected "${row1Title}"`);
  }
  const message = `Inbox reorder stability ${Date.now()}`;
  await page.getByTestId('chat-input').fill(message);
  await page.getByTestId('chat-send').click();

  // Give the app time to process the message and emit app/workdir task events.
  await sleep(500);

  const row0TitleAfterMessage = await inboxRowTaskTitle(row0);
  if (row0TitleAfterMessage !== row0Title) {
    throw new Error(
      `expected inbox ordering to stay stable after message updates; got row0 before="${row0Title}" after="${row0TitleAfterMessage}"`,
    );
  }

  // Reselecting inbox should refresh and resort by updated_at.
  await page.getByTestId('nav-inbox-button').click();
  const startReselect = Date.now();
  while (Date.now() - startReselect < 20_000) {
    const current = await inboxRowTaskTitle(row0);
    if (current === row1Title) break;
    await sleep(100);
  }
  const row0TitleAfterReselect = await inboxRowTaskTitle(row0);
  if (row0TitleAfterReselect !== row1Title) {
    throw new Error(
      `expected inbox to move the updated task ahead after reselect; got row0="${row0TitleAfterReselect}" expected "${row1Title}"`,
    );
  }

  // Leaving and reopening inbox should refresh and resort by updated_at.
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByTestId('nav-inbox-button').click();
  await page.getByTestId('inbox-view').waitFor({ state: 'visible' });

  const reopenedTitles = await collectInboxTitles({ page, limit: 10 });
  const row0Index = reopenedTitles.indexOf(row0Title);
  const row1Index = reopenedTitles.indexOf(row1Title);
  if (row0Index === -1 || row1Index === -1) {
    throw new Error(
      `expected inbox to still contain "${row0Title}" and "${row1Title}" after reopen; got titles=${JSON.stringify(reopenedTitles)}`,
    );
  }
  if (row1Index >= row0Index) {
    throw new Error(
      `expected inbox to move the updated task ahead after reopen; got "${row1Title}" at index=${row1Index} and "${row0Title}" at index=${row0Index}`,
    );
  }

  // Select the newest item so later scenarios have a stable header surface (star button).
  const row0AfterReopen = page.getByTestId('inbox-notification-row-0');
  await row0AfterReopen.waitFor({ state: 'visible' });
  await row0AfterReopen.click();
  await page.getByTestId('task-star-button').waitFor({ state: 'visible' });
}
