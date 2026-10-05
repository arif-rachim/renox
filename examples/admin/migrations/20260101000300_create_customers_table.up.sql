CREATE TABLE customers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    phone TEXT,
    city TEXT,
    tier TEXT NOT NULL DEFAULT 'regular',
    newsletter INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    created_at TEXT,
    updated_at TEXT
);
