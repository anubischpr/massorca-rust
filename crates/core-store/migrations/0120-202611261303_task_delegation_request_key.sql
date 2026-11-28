-- A request key per parent task (decision 0128), so a retried delegation returns the first child.
ALTER TABLE task_delegations ADD COLUMN request_id TEXT;
CREATE UNIQUE INDEX task_delegations_by_request ON task_delegations (parent_task_id, request_id) WHERE request_id IS NOT NULL;
