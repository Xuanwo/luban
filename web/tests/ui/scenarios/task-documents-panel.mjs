export async function runTaskDocumentsPanel({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByText('Mock task 1').first().click();
  await page.getByTestId('chat-scroll-container').waitFor({ state: 'visible' });
  const panel = page.getByTestId('task-document-panel');
  await panel.waitFor({ state: 'visible' });
  await panel.getByTestId('task-document-section-task').waitFor({ state: 'visible' });
  await panel.getByTestId('task-document-section-plan').waitFor({ state: 'visible' });
  await panel.getByTestId('task-document-section-memory').waitFor({ state: 'visible' });
  await panel.getByTestId('task-document-rendered-task').waitFor({ state: 'visible' });
  await panel.getByTestId('task-document-fixed-comment').waitFor({ state: 'visible' });
  const fixedCommentInput = panel.getByTestId('chat-input');
  await fixedCommentInput.waitFor({ state: 'visible' });
  const placeholder = await fixedCommentInput.getAttribute('placeholder');
  if (placeholder !== "Review comment or instruction to agent...") {
    throw new Error(`unexpected task document comment placeholder: ${placeholder ?? '<null>'}`);
  }
  await panel.getByTestId('chat-attach').waitFor({ state: 'visible' });
  await panel.getByTestId('agent-selector').waitFor({ state: 'visible' });
  const shellToggleCount = await panel.getByTestId('chat-mode-toggle').count();
  if (shellToggleCount !== 0) {
    throw new Error(`expected no left-side shell toggle in task documents panel, got ${shellToggleCount}`);
  }

  // TipTap editor is always visible (seamless WYSIWYG) — click to focus, then select all
  const taskEditor = panel.getByTestId('task-document-editor-task');
  await taskEditor.waitFor({ state: 'visible' });
  await taskEditor.click();
  await page.keyboard.type('Draft task notes for inline comment smoke test.');
  await taskEditor.press('ControlOrMeta+A');
  await panel.getByTestId('task-document-selection-toolbar').waitFor({ state: 'visible' });
  await panel.getByTestId('task-document-selection-toolbar-comment').click();
  await panel.getByTestId('task-document-inline-comment').waitFor({ state: 'visible' });
  await panel.getByTestId('task-document-inline-comment-input').fill('Please clarify this section.');
  await panel.getByTestId('task-document-inline-comment-submit').click();

  await page.getByTestId('task-workspace-tab-agents').click();
  const workspace = page.getByTestId('task-workspace-panel');
  const scrollContainer = workspace.getByTestId('chat-scroll-container');
  await scrollContainer.waitFor({ state: 'visible' });
  const overflowY = await scrollContainer.evaluate((el) => window.getComputedStyle(el).overflowY);
  if (!(overflowY === 'auto' || overflowY === 'scroll')) {
    throw new Error(`expected agents panel to be scrollable, got overflowY=${overflowY}`);
  }
  const chatInputCount = await workspace.getByTestId('chat-input').count();
  if (chatInputCount !== 0) {
    throw new Error(`expected no right-side chat input in agents activity, got ${chatInputCount}`);
  }
}
