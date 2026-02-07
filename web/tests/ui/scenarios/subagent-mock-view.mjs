import { waitForLocatorCount } from '../lib/utils.mjs';

export async function runSubagentMockView({ page }) {
  await page.getByTestId('sidebar-project-mock-project-1').click();
  await page.getByTestId('task-list-view').waitFor({ state: 'visible' });

  await page.getByText('Mock: Subtask delegation demo').first().click();
  const turnCard = page.getByTestId('agent-turn-card').first();
  await turnCard.waitFor({ state: 'visible' });

  await turnCard.getByTestId('agent-turn-toggle').click();

  const subtaskPills = turnCard.getByTestId('activity-subagent-pill');
  await waitForLocatorCount(subtaskPills, 3, 20_000);

  const statusPills = turnCard.getByTestId('activity-subagent-status');
  await waitForLocatorCount(statusPills, 3, 20_000);

  const statusTexts = (await statusPills.allTextContents()).map((v) => v.trim());
  if (!statusTexts.includes('RUNNING')) {
    throw new Error(`expected at least one RUNNING subtask status, got: ${JSON.stringify(statusTexts)}`);
  }
  if (!statusTexts.includes('DONE')) {
    throw new Error(`expected at least one DONE subtask status, got: ${JSON.stringify(statusTexts)}`);
  }

  const completedRow = turnCard.getByTestId('agent-turn-event').filter({ hasText: 'Completed subtask #201' }).first();
  await completedRow.waitFor({ state: 'visible' });
}
