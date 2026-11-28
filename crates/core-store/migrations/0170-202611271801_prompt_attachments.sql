-- The files a queued prompt sends with it (decision 0172): a JSON array of the paths the caller gave, checked again
-- before the prompt is typed. A prompt queued before this step sends no files.
ALTER TABLE agent_prompt_queue ADD COLUMN attachments_json TEXT NOT NULL DEFAULT '[]';
