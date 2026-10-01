CREATE TABLE products (
    id TEXT PRIMARY KEY, -- a ULID, made by the model on insert
    name TEXT NOT NULL UNIQUE,
    price INTEGER NOT NULL,
    created_at TEXT,
    updated_at TEXT
);
