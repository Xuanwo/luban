# Codex CLI Integration

This document describes how Luban runs the Codex CLI and maps streamed events into the chat UI.

## Executable discovery

- Default: `codex` in `PATH`.
- Override: set `LUBAN_CODEX_BIN` to an absolute path.

If the executable is missing or not runnable, Luban surfaces a user-visible error and does not
attempt fallbacks.

## Streaming protocol

Luban runs Codex in streaming mode and reads stdout line-by-line.

- Each line is either JSON (an event object) or noise.
- Noise lines are ignored (but may be logged in debug builds).
- Unknown event types are ignored.

### "Thinking" mapping

Codex may emit:

```json
{"type":"item.completed","item":{"type":"reasoning","text":"..."}}
```

Luban treats `item.type == "reasoning"` as a "thinking" entry and renders it separately from normal
assistant messages.

### Assistant message mapping

Codex may emit:

```json
{"type":"item.completed","item":{"type":"agent_message","text":"..."}}
```

Luban treats `item.type == "agent_message"` as a normal assistant message in the conversation.

## Tolerance and compatibility

The parser is intentionally tolerant:

- Unknown fields are ignored.
- Older legacy JSON event shapes are accepted when possible.
- Invalid JSON is treated as noise.

The goal is to keep the UI responsive even if Codex changes minor details of its event payloads.

## Images and attachments

- User messages persist plain text plus structured attachments (`attachments: Vec<AttachmentRef>`).
- Image attachments are passed to the Codex CLI via `--image <absolute_path>` arguments in a stable
  order (resolved server-side).
- The web UI never embeds absolute paths in user-visible text.

See `docs/persistence.md` for attachment storage layout.
