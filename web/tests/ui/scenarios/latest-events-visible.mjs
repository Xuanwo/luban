import { waitForLocatorCount } from '../lib/utils.mjs';

export async function runLatestEventsVisible({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });
  await page.getByText('Mock task 1').first().click();

  const scrollContainer = page.getByTestId('chat-scroll-container');
  await scrollContainer.waitFor({ state: 'visible' });
  const contentWrapper = page.getByTestId('chat-content-wrapper');
  await contentWrapper.waitFor({ state: 'visible' });

  const createdEvent = page.getByTestId('activity-event').filter({ hasText: 'created the task' }).first();
  await createdEvent.scrollIntoViewIfNeeded();
  await createdEvent.waitFor({ state: 'visible' });
  const createdEventText = ((await createdEvent.getByTestId('event-text').textContent()) ?? '').trim();
  if (!createdEventText.startsWith('Luban')) {
    throw new Error(`expected system events to be attributed to Luban, got "${createdEventText}"`);
  }
  const createdAuthor = createdEvent.getByTestId('activity-simple-author');
  await createdAuthor.waitFor({ state: 'visible' });
  const createdAuthorBox = await createdAuthor.boundingBox();
  if (!createdAuthorBox) throw new Error('missing simple event author bounding box');

  const containerMetrics = await scrollContainer.evaluate((el) => {
    const rect = el.getBoundingClientRect();
    return {
      left: rect.left,
      clientLeft: el.clientLeft,
      clientWidth: el.clientWidth,
    };
  });
  const wrapperMetrics = await contentWrapper.evaluate((el) => {
    const rect = el.getBoundingClientRect();
    return { left: rect.left, right: rect.right };
  });
  const containerInnerLeft = containerMetrics.left + containerMetrics.clientLeft;
  const containerInnerRight = containerInnerLeft + containerMetrics.clientWidth;
  const leftInset = wrapperMetrics.left - containerInnerLeft;
  const rightInset = containerInnerRight - wrapperMetrics.right;
  const insetDelta = Math.abs(leftInset - rightInset);
  const insetTolerance = 2;
  if (insetDelta > insetTolerance) {
    throw new Error(
      `expected chat content to be horizontally centered (within ${insetTolerance}px), got leftInset=${leftInset.toFixed(2)}px rightInset=${rightInset.toFixed(2)}px`,
    );
  }
  await scrollContainer.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });

  const latestTurn = page
    .getByTestId('agent-turn-card')
    .filter({ hasText: "I've updated the documentation" })
    .first();
  await latestTurn.waitFor({ state: 'visible' });
  await latestTurn.scrollIntoViewIfNeeded();

  const latestOutput = latestTurn.getByTestId('activity-agent-message-content').first();
  await latestOutput.waitFor({ state: 'visible' });
  const latestOutputText = ((await latestOutput.innerText()) ?? '').trim();
  if (!latestOutputText.includes("I've updated the documentation")) {
    throw new Error(`expected latest turn card to show the latest model output, got "${latestOutputText}"`);
  }

  const agentTurnHeaderText = ((await latestTurn.getByTestId('agent-turn-toggle').textContent()) ?? '').trim();
  if (!agentTurnHeaderText.startsWith('Codex')) {
    throw new Error(`expected agent turns to be attributed to Codex, got "${agentTurnHeaderText}"`);
  }

  const turnAuthor = latestTurn.getByTestId('activity-card-author').first();
  await turnAuthor.waitFor({ state: 'visible' });
  const turnAuthorBox = await turnAuthor.boundingBox();
  if (!turnAuthorBox) throw new Error('missing card event author bounding box');
  const authorDeltaX = Math.abs(createdAuthorBox.x - turnAuthorBox.x);
  const authorTolerance = 1.5;
  if (authorDeltaX > authorTolerance) {
    throw new Error(
      `expected simple/card author names to align within ${authorTolerance}px, got delta=${authorDeltaX.toFixed(2)}px (simple=${createdAuthorBox.x.toFixed(2)}px card=${turnAuthorBox.x.toFixed(2)}px)`,
    );
  }

  const turnAvatarInner = latestTurn.getByTestId('activity-card-avatar-inner').first();
  await turnAvatarInner.waitFor({ state: 'visible' });
  const turnAvatarInnerBox = await turnAvatarInner.boundingBox();
  if (!turnAvatarInnerBox) throw new Error('missing card event avatar bounding box');
  const avatarSizeDelta = Math.max(Math.abs(turnAvatarInnerBox.width - 16), Math.abs(turnAvatarInnerBox.height - 16));
  const avatarSizeTolerance = 1;
  if (avatarSizeDelta > avatarSizeTolerance) {
    throw new Error(
      `expected card event avatar to be 16px (±${avatarSizeTolerance}px), got ${turnAvatarInnerBox.width.toFixed(2)}x${turnAvatarInnerBox.height.toFixed(2)}`,
    );
  }

  await latestTurn.getByTestId('agent-turn-toggle').click();
  const showMoreToggle = latestTurn.getByTestId('agent-turn-show-more');
  if ((await showMoreToggle.count()) > 0) {
    await showMoreToggle.first().click();
  }

  const progressEvents = latestTurn.getByTestId('agent-turn-event').filter({ hasText: 'Progress update' });
  await waitForLocatorCount(progressEvents, 1, 20_000);

  const alignmentRow = progressEvents.first();
  const avatar = latestTurn.getByTestId('agent-turn-avatar');
  await avatar.waitFor({ state: 'visible' });
  const avatarBox = await avatar.boundingBox();
  if (!avatarBox) throw new Error('missing agent avatar bounding box');

  const firstSimpleEventAvatar = page.getByTestId('event-avatar').first();
  await firstSimpleEventAvatar.waitFor({ state: 'visible' });
  const firstSimpleEventAvatarBox = await firstSimpleEventAvatar.boundingBox();
  if (!firstSimpleEventAvatarBox) throw new Error('missing simple event icon bounding box');

  const icon = alignmentRow.getByTestId('activity-event-icon');
  const title = alignmentRow.getByTestId('activity-event-title');
  await icon.waitFor({ state: 'visible' });
  await title.waitFor({ state: 'visible' });
  const iconBox = await icon.boundingBox();
  const titleBox = await title.boundingBox();
  if (!iconBox) throw new Error('missing activity icon bounding box');
  if (!titleBox) throw new Error('missing activity title bounding box');

  const avatarCenterX = avatarBox.x + avatarBox.width / 2;
  const firstSimpleEventCenterX = firstSimpleEventAvatarBox.x + firstSimpleEventAvatarBox.width / 2;
  const simpleEventDeltaX = Math.abs(avatarCenterX - firstSimpleEventCenterX);
  const simpleEventTolerance = 1.5;
  if (simpleEventDeltaX > simpleEventTolerance) {
    throw new Error(
      `expected simple event icon to align with card avatar center within ${simpleEventTolerance}px, got delta=${simpleEventDeltaX}px`,
    );
  }

  const iconCenterX = iconBox.x + iconBox.width / 2;
  const xDelta = Math.abs(avatarCenterX - iconCenterX);
  const xTolerance = 1.5;
  if (xDelta > xTolerance) {
    throw new Error(`expected activity icon to align with avatar center within ${xTolerance}px, got delta=${xDelta}px`);
  }

  const iconCenterY = iconBox.y + iconBox.height / 2;
  const titleCenterY = titleBox.y + titleBox.height / 2;
  const centerDelta = Math.abs(iconCenterY - titleCenterY);
  const centerTolerance = 1.5;
  if (centerDelta > centerTolerance) {
    throw new Error(`expected activity icon/title vertical alignment within ${centerTolerance}px, got delta=${centerDelta}px`);
  }

  const userActivityContent = page.getByTestId('activity-user-message-content');
  const agentActivityContent = page.getByTestId('activity-agent-message-content');
  if ((await userActivityContent.count()) > 0 && (await agentActivityContent.count()) > 0) {
    const lastUser = userActivityContent.last();
    const lastAgent = agentActivityContent.last();
    await lastUser.scrollIntoViewIfNeeded();
    await lastAgent.scrollIntoViewIfNeeded();

    const agentMarkdownRoot = lastAgent.locator(':scope > div').first();
    await agentMarkdownRoot.waitFor({ state: 'visible' });

    const userFontSize = await lastUser.evaluate((el) => getComputedStyle(el).fontSize);
    const agentFontSize = await agentMarkdownRoot.evaluate((el) => getComputedStyle(el).fontSize);
    if (userFontSize !== agentFontSize) {
      throw new Error(`expected user/agent message font size to match, got user=${userFontSize}, agent=${agentFontSize}`);
    }
  }

  const dedupeRows = latestTurn.getByTestId('agent-turn-event').filter({ hasText: 'Dedupe update' });
  await waitForLocatorCount(dedupeRows, 1, 20_000);
  const dedupeRunningIcons = await dedupeRows.first().getByTestId('event-running-icon').count();
  if (dedupeRunningIcons !== 0) {
    throw new Error('expected dedupe update to be done, but it is still running');
  }
  const dedupeDuration = dedupeRows.first().getByTestId('activity-event-duration');
  await dedupeDuration.waitFor({ state: 'visible' });
  const dedupeDurationText = ((await dedupeDuration.textContent()) ?? '').trim();
  if (dedupeDurationText === 'now') {
    throw new Error('expected activity duration to be formatted as a duration (not a relative timestamp like "now")');
  }
  const durationPattern = /^(< 1s|\d+s|\d+m \d+s|\d+h \d+m \d+s)$/;
  if (!durationPattern.test(dedupeDurationText)) {
    throw new Error(`expected activity duration to match ${durationPattern}, got "${dedupeDurationText}"`);
  }

  const runningRow = progressEvents.filter({ hasText: 'Progress update 3' }).first();
  await runningRow.waitFor({ state: 'visible' });
  await runningRow.getByTestId('event-running-icon').waitFor({ state: 'visible' });

  // Only show the cancel affordance when hovering the running icon area (not the entire card).
  const cancelArea = latestTurn.getByTestId('agent-turn-cancel-area');
  await cancelArea.waitFor({ state: 'visible' });
  const cancelButton = cancelArea.getByTestId('agent-turn-cancel');

  const getOpacity = async (locator) => await locator.evaluate((el) => getComputedStyle(el).opacity);

  const waitForOpacity = async (locator, expected, timeoutMs = 1000) => {
    const start = Date.now();
    while (Date.now() - start <= timeoutMs) {
      const opacity = await getOpacity(locator);
      if (opacity === expected) return;
      await new Promise((r) => setTimeout(r, 25));
    }
    const finalOpacity = await getOpacity(locator);
    throw new Error(`expected opacity ${expected}, got ${finalOpacity}`);
  };

  await waitForOpacity(cancelButton, '0');

  await latestTurn.getByTestId('agent-turn-toggle').hover();
  await waitForOpacity(cancelButton, '0');

  await cancelArea.hover();
  await waitForOpacity(cancelButton, '1');
}
