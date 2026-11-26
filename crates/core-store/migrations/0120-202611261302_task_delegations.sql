-- Team delegation links (decision 0128): a delegated child task and the parent task that delegated it.
CREATE TABLE task_delegations (
    child_task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    read_only INTEGER NOT NULL DEFAULT 0 CHECK(read_only IN (0,1)),
    created_at INTEGER NOT NULL
);
CREATE INDEX task_delegations_by_parent ON task_delegations (parent_task_id, created_at);
