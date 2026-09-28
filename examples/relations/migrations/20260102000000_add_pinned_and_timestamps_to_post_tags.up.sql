-- Pivot columns: besides the two ids, a link between a post and a tag says
-- whether the post is pinned on the tag's page, and when it was tagged
-- (`Pivot::with_timestamps()` fills created_at and updated_at).
ALTER TABLE post_tags ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE post_tags ADD COLUMN created_at TEXT;
ALTER TABLE post_tags ADD COLUMN updated_at TEXT;
