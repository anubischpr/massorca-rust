-- Conference room routing (AUTO-13): a person's line waiting for its routing query, and that query's run.
CREATE TABLE team_room_routes (
    message_id TEXT PRIMARY KEY REFERENCES team_room_messages(id) ON DELETE CASCADE,
    team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    session TEXT,
    state TEXT NOT NULL DEFAULT 'open' CHECK(state IN ('open','settled')),
    start INTEGER NOT NULL DEFAULT 1 CHECK(start IN (0,1)),
    admin INTEGER NOT NULL DEFAULT 0 CHECK(admin IN (0,1)),
    created_at INTEGER NOT NULL
);
CREATE INDEX team_room_routes_by_session ON team_room_routes (session);
