CREATE TABLE customers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT,
    phone TEXT,
    city TEXT,
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX customers_name ON customers (name);
