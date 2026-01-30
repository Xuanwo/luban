# Agent Cancel & Resume Interaction

This document defines the expected UI and state behavior for interrupting an in-progress agent turn, as well as resuming after an interruption when a queue exists.

## Concepts

- **Turn**: A single agent execution started by a user message.
- **Queued prompts**: Messages that were submitted while the agent is running and therefore queued for later.
- **Paused queue**: A state where queued prompts exist but the system does not automatically run them until the user explicitly resumes.

## UI States

The UI uses a small state machine for the running card:

- `running`: The agent is streaming.
- `cancelling`: The user clicked **Cancel** and is composing an interruption message (inline editor is visible).
- `paused`: The current run is interrupted and queued prompts exist (inline editor is hidden, **Resume** button is visible).
- `resuming`: The user clicked **Resume** and is composing a resume message (inline editor is visible).

## Rendering Rules

The **Agent Running Card** is attached to the most recent assistant message that has activities, and is rendered when:

- the conversation `run_status` is `running`, or
- the conversation is `queue_paused` and `pending_prompts` is non-empty.

All other assistant messages render the regular **Activity Stream**.

When the running card is expanded, the scroll container should auto-compensate so the card header stays anchored while the activity list above it grows.

## Cancel Flow

1. While the agent is running, the running card header shows a **Cancel** button.
2. Clicking **Cancel** enters `cancelling` and expands an inline editor.
3. Submitting the inline editor:
   - cancels the current turn
   - appends the old turn as a cancelled activity stream (cancel icon + `Cancelled after N steps`)
   - sends the new message as a normal user message, starting a new turn
4. Dismissing the inline editor (Escape key, click outside, or the editor dismiss action):
   - cancels the current turn
   - if queued prompts exist, transitions to `paused`
   - otherwise, the old turn is shown as a cancelled activity stream and the running card is not shown

## Resume Flow

1. When the UI is `paused`, the running card header shows a **Resume** button.
2. Clicking **Resume** enters `resuming` and shows the inline editor.
3. Submitting the inline editor sends the message as a normal user message, starting a new turn.
4. Dismissing the inline editor returns to `paused`.

## Regression Coverage

UI automation coverage:

- `agent-browser` smoke tests cover basic rendering and modal interactions.
- Cancel/resume flows should be validated via domain tests + targeted UI automation as follow-up.
