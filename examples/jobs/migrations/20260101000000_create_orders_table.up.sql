CREATE TABLE orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_email TEXT NOT NULL,
    item TEXT NOT NULL,
    total INTEGER NOT NULL,
    created_at TEXT,
    updated_at TEXT
);
