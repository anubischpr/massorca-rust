-- Automation destinations (decision 0127): where each run happens. Old rows read as the project's own place.
ALTER TABLE automations ADD COLUMN destination TEXT NOT NULL DEFAULT 'project_default' CHECK(destination IN ('project_default','project_worktree_per_run','existing_workspace'));
ALTER TABLE automations ADD COLUMN destination_workspace_id TEXT;
ALTER TABLE automations ADD COLUMN destination_path TEXT;
ALTER TABLE automations ADD COLUMN base_branch TEXT;
