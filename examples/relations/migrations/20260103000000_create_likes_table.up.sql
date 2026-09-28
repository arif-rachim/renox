-- Likes belong to a post or a comment (polymorphic): `likeable_type` is the
-- parent's table ("posts" or "comments"), `likeable_id` its id.
CREATE TABLE likes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    likeable_type TEXT NOT NULL,
    likeable_id INTEGER NOT NULL,
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX likes_likeable ON likes (likeable_type, likeable_id);

-- A foreign key can't point at two tables, so triggers remove a parent's
-- likes. They also fire for comments deleted by a post's ON DELETE CASCADE,
-- which a model hook never sees.
CREATE TRIGGER posts_delete_likes AFTER DELETE ON posts BEGIN
    DELETE FROM likes WHERE likeable_type = 'posts' AND likeable_id = OLD.id;
END;
CREATE TRIGGER comments_delete_likes AFTER DELETE ON comments BEGIN
    DELETE FROM likes WHERE likeable_type = 'comments' AND likeable_id = OLD.id;
END;
