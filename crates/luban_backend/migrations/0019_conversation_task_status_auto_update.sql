-- Track whether the auto task status system task has analyzed the latest messages.
--
-- `task_status_last_analyzed_message_seq` stores the latest `conversation_entries.seq` value
-- among user/agent message entries at the time we last ran the system task.
-- This lets the server backfill analyses for Iterating/Validating threads that have new
-- messages but have never been analyzed (or are stale).

ALTER TABLE conversations
    ADD COLUMN task_status_last_analyzed_message_seq INTEGER NOT NULL DEFAULT 0;

