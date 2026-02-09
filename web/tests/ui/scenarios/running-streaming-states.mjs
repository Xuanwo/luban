async function openTaskByTitle(page, title) {
  const projectButton = page.getByTestId('sidebar-project-mock-project-1');
  await projectButton.waitFor({ state: 'visible' });
  await projectButton.click();

  const taskList = page.getByTestId('task-list-view');
  await taskList.waitFor({ state: 'visible' });

  const task = taskList.getByText(title).first();
  await task.waitFor({ state: 'visible' });
  await task.click();

  const scrollContainer = page.getByTestId('chat-scroll-container');
  await scrollContainer.waitFor({ state: 'visible' });
  await scrollContainer.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });
}

export async function runRunningStreamingStates({ page, baseUrl }) {
  await page.goto(baseUrl, { waitUntil: 'networkidle' });
  await page.evaluate(() => {
    if (typeof window.__LUBAN_MOCK_RESET__ !== 'function') {
      throw new Error('mock reset hook is not available');
    }
    window.__LUBAN_MOCK_RESET__();
  });
  await page.goto(baseUrl, { waitUntil: 'networkidle' });
  await page.getByTestId('nav-sidebar').waitFor({ state: 'visible' });
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await openTaskByTitle(page, 'Mock: Running state (no output)');

  const noOutputCard = page.getByTestId('agent-turn-card').first();
  await noOutputCard.waitFor({ state: 'visible' });

  const noOutputContentCount = await noOutputCard.getByTestId('activity-agent-message-content').count();
  if (noOutputContentCount !== 0) {
    throw new Error(`expected no output for running state without message, got ${noOutputContentCount}`);
  }

  const noOutputSummary = ((await noOutputCard.getByTestId('agent-turn-toggle').textContent()) ?? '').trim();
  if (!noOutputSummary.includes('just test-fast')) {
    throw new Error(`expected running summary to show latest tool progress, got "${noOutputSummary}"`);
  }

  await noOutputCard.getByTestId('agent-turn-toggle').click();
  const noOutputRunningEvent = noOutputCard.getByTestId('agent-turn-event').filter({ hasText: 'just test-fast' }).first();
  await noOutputRunningEvent.waitFor({ state: 'visible' });
  await noOutputRunningEvent.getByTestId('event-running-icon').waitFor({ state: 'visible' });

  await openTaskByTitle(page, 'Mock: Running state (streaming output)');

  const streamingCard = page.getByTestId('agent-turn-card').first();
  await streamingCard.waitFor({ state: 'visible' });

  const streamingSummary = ((await streamingCard.getByTestId('agent-turn-toggle').textContent()) ?? '').trim();
  if (!streamingSummary.includes('Collecting diagnostics')) {
    throw new Error(`expected streaming summary to show latest event, got "${streamingSummary}"`);
  }

  const streamingOutput = streamingCard.getByTestId('activity-agent-message-content').first();
  await streamingOutput.waitFor({ state: 'visible' });
  const streamingOutputText = ((await streamingOutput.innerText()) ?? '').trim();
  if (!streamingOutputText.includes('Almost done. Preparing the final consolidated response and final verification notes.')) {
    throw new Error(`expected running card body to show latest assistant message, got "${streamingOutputText}"`);
  }

  await streamingCard.getByTestId('agent-turn-toggle').click();

  const streamingMessageEvents = streamingCard.getByTestId('agent-turn-message-event');
  const streamingMessageCount = await streamingMessageEvents.count();
  if (streamingMessageCount < 5) {
    throw new Error(`expected expanded timeline to include all independent streaming message events, got ${streamingMessageCount}`);
  }
  const finalMessageRows = streamingMessageEvents.filter({
    hasText: 'Almost done. Preparing the final consolidated response and final verification notes.',
  });
  const finalMessageCount = await finalMessageRows.count();
  if (finalMessageCount !== 1) {
    throw new Error(`expected repeated updates for the same assistant message id to be deduped, got ${finalMessageCount}`);
  }

  const reasoningEvent = streamingCard
    .getByTestId('agent-turn-event')
    .filter({ hasText: 'Preparing the response body and validating examples.' })
    .first();
  await reasoningEvent.waitFor({ state: 'visible' });

  const streamingToolTail = streamingCard
    .getByTestId('agent-turn-event')
    .filter({ hasText: 'Collecting diagnostics' })
    .first();
  await streamingToolTail.waitFor({ state: 'visible' });

  const timelineRows = streamingCard.locator('[data-testid="agent-turn-message-event"], [data-testid="agent-turn-event"]');
  const timelineTexts = (await timelineRows.allTextContents())
    .map((value) => value.trim())
    .filter((value) => value.length > 0);

  const firstMessageIndex = timelineTexts.findIndex((title) =>
    title.includes('Collected context from the relevant files and started drafting the response structure.'),
  );
  const secondMessageIndex = timelineTexts.findIndex(
    (title, idx) =>
      idx > firstMessageIndex &&
      title.includes('Validated the timeline ordering path and confirmed assistant message events are rendered as independent rows.'),
  );
  const toolTailIndex = timelineTexts.findIndex((title) => title.includes('Collecting diagnostics'));

  if (firstMessageIndex < 0 || secondMessageIndex < 0 || toolTailIndex < 0) {
    throw new Error(`expected message and tool events in expanded timeline, got [${timelineTexts.join(' | ')}]`);
  }

  if (!(firstMessageIndex < secondMessageIndex && secondMessageIndex < toolTailIndex)) {
    throw new Error(
      `expected expanded events to keep chronological order for independent messages and tool tail, got [${timelineTexts.join(' | ')}]`,
    );
  }

  await openTaskByTitle(page, 'Mock: Running state (completed output)');

  const completedCard = page.getByTestId('agent-turn-card').first();
  await completedCard.waitFor({ state: 'visible' });

  const completedOutput = completedCard.getByTestId('activity-agent-message-content').first();
  await completedOutput.waitFor({ state: 'visible' });
  const completedText = ((await completedOutput.innerText()) ?? '').trim();
  if (!completedText.includes('Completed run with full output.')) {
    throw new Error(`expected completed state to show final assistant output, got "${completedText}"`);
  }

  await completedCard.getByTestId('agent-turn-toggle').click();
  const completedMessageEvent = completedCard
    .getByTestId('agent-turn-message-event')
    .filter({ hasText: 'Completed run with full output.' })
    .first();
  await completedMessageEvent.waitFor({ state: 'visible' });
  await completedCard.getByTestId('agent-turn-event').filter({ hasText: 'Turn duration:' }).first().waitFor({ state: 'visible' });
}
