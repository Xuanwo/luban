ALTER TABLE conversations
    ADD COLUMN task_status TEXT NOT NULL DEFAULT 'todo';

UPDATE conversations
SET task_status = 'backlog'
WHERE NOT EXISTS (
    SELECT 1
    FROM conversation_entries
    WHERE conversation_entries.project_slug = conversations.project_slug
      AND conversation_entries.workspace_name = conversations.workspace_name
      AND conversation_entries.thread_local_id = conversations.thread_local_id
      AND conversation_entries.kind = 'user_message'
);

