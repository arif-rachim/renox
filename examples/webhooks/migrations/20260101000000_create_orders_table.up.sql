CREATE TABLE orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL UNIQUE,
    amount INTEGER NOT NULL,
    status TEXT NOT NULL,
    paid_via TEXT,
    created_at TEXT,
    updated_at TEXT
);
