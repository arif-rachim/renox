-- SQLite (the plain file); PostgreSQL uses the .postgres.up.sql next to it.
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    done INTEGER NOT NULL DEFAULT 0,
    due_on TEXT,
    created_at TEXT,
    updated_at TEXT
);
