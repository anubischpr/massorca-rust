-- Automation runs (decision 0125): one row per run or folded missed occurrences, numbered per automation and never
-- renumbered. Removing an automation removes its history.
CREATE TABLE automation_runs (
    id TEXT PRIMARY KEY,
    automation_id TEXT NOT NULL REFERENCES automations(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    trigger TEXT NOT NULL CHECK(trigger IN ('schedule','manual')),
    scheduled_for INTEGER NOT NULL,
    occurrences INTEGER NOT NULL DEFAULT 1 CHECK(occurrences >= 1),
    state TEXT NOT NULL CHECK(state IN ('pending','dispatching','dispatched','completed','skipped_precheck','skipped_missed','skipped_unavailable','skipped_needs_interactive_auth','dispatch_failed')),
    outcome TEXT CHECK(outcome IN ('success','failure','cancelled','interrupted')),
    session_id TEXT,
    workspace_id TEXT,
    reason TEXT,
    reason_code TEXT,
    result TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    started_at INTEGER,
    ended_at INTEGER,
    created_at INTEGER NOT NULL,
    UNIQUE (automation_id, number)
);
CREATE INDEX automation_runs_by_session ON automation_runs (session_id);
