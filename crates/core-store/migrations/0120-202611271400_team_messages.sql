-- Messages between the bots of a delegation (AUTO-18, decision 0129).
CREATE TABLE team_messages (
    id TEXT PRIMARY KEY,
    from_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    to_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    from_bot_id TEXT REFERENCES bots(id) ON DELETE SET NULL,
    to_bot_id TEXT REFERENCES bots(id) ON DELETE SET NULL,
    kind TEXT NOT NULL,
    priority TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    reply_to TEXT,
    outcome TEXT,
    settles_task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    from_session_id TEXT,
    delivery TEXT NOT NULL DEFAULT 'inbox',
    read_at INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX team_messages_by_recipient ON team_messages (to_task_id, read_at, created_at);
CREATE INDEX team_messages_by_sender ON team_messages (from_task_id, to_task_id, kind, created_at);
-- One settlement per run: a reopened task's new run settles it again.
CREATE UNIQUE INDEX team_messages_settles ON team_messages (settles_task_id, from_session_id) WHERE settles_task_id IS NOT NULL;
