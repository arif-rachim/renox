CREATE TABLE products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    price INTEGER NOT NULL,
    created_at TEXT,
    updated_at TEXT,
    deleted_at TEXT
);
CREATE INDEX products_user_id ON products (user_id);
