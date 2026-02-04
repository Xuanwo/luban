-- Clean up implicitly created "Thread 1" placeholder tasks that have no content.
--
-- Criteria:
-- - thread_local_id = 1
-- - title = "Thread 1"
-- - status is backlog and has no remote thread id
-- - only 1 entry exists and it is a system_event (the auto-inserted TaskCreated)
-- - no queued prompts, no run metadata/config
-- - the workspace has at least one other thread (so thread 1 is clearly the extra placeholder)

CREATE TEMP TABLE luban_cleanup_candidates (
  project_slug   TEXT NOT NULL,
  workspace_name TEXT NOT NULL,
  PRIMARY KEY (project_slug, workspace_name)
);

INSERT OR IGNORE INTO luban_cleanup_candidates (project_slug, workspace_name)
SELECT c.project_slug, c.workspace_name
FROM conversations c
WHERE c.thread_local_id = 1
  AND COALESCE(c.title, '') = 'Thread 1'
  AND COALESCE(c.task_status, '') = 'backlog'
  AND c.thread_id IS NULL
  AND COALESCE(c.queue_paused, 0) = 0
  AND c.run_started_at_unix_ms IS NULL
  AND c.run_finished_at_unix_ms IS NULL
  AND c.agent_runner IS NULL
  AND c.agent_model_id IS NULL
  AND c.thinking_effort IS NULL
  AND c.amp_mode IS NULL
  AND EXISTS (
    SELECT 1
    FROM conversations c2
    WHERE c2.project_slug = c.project_slug
      AND c2.workspace_name = c.workspace_name
      AND c2.thread_local_id <> 1
  )
  AND NOT EXISTS (
    SELECT 1
    FROM conversation_queued_prompts qp
    WHERE qp.project_slug = c.project_slug
      AND qp.workspace_name = c.workspace_name
      AND qp.thread_local_id = 1
  )
  AND (
    SELECT COUNT(*)
    FROM conversation_entries e
    WHERE e.project_slug = c.project_slug
      AND e.workspace_name = c.workspace_name
      AND e.thread_local_id = 1
  ) = 1
  AND EXISTS (
    SELECT 1
    FROM conversation_entries e
    WHERE e.project_slug = c.project_slug
      AND e.workspace_name = c.workspace_name
      AND e.thread_local_id = 1
      AND e.seq = 1
      AND e.kind = 'system_event'
  );

DELETE FROM conversation_queued_prompts
WHERE thread_local_id = 1
  AND EXISTS (
    SELECT 1
    FROM luban_cleanup_candidates x
    WHERE x.project_slug = conversation_queued_prompts.project_slug
      AND x.workspace_name = conversation_queued_prompts.workspace_name
  );

DELETE FROM conversation_entries
WHERE thread_local_id = 1
  AND EXISTS (
    SELECT 1
    FROM luban_cleanup_candidates x
    WHERE x.project_slug = conversation_entries.project_slug
      AND x.workspace_name = conversation_entries.workspace_name
  );

DELETE FROM conversations
WHERE thread_local_id = 1
  AND EXISTS (
    SELECT 1
    FROM luban_cleanup_candidates x
    WHERE x.project_slug = conversations.project_slug
      AND x.workspace_name = conversations.workspace_name
  );

DROP TABLE luban_cleanup_candidates;
