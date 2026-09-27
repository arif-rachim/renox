CREATE TABLE documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,
    file_key TEXT NOT NULL,
    file_name TEXT NOT NULL,
    created_at TEXT,
    updated_at TEXT
);
