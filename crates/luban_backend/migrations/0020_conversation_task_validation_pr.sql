ALTER TABLE conversations
  ADD COLUMN task_validation_pr_number INTEGER;

ALTER TABLE conversations
  ADD COLUMN task_validation_pr_url TEXT;
