-- Whether a workspace is the project's own checkout (decision 0112): only forgotten on removal, never deleted.
ALTER TABLE workspaces ADD COLUMN in_place INTEGER NOT NULL DEFAULT 0 CHECK(in_place IN (0,1));
