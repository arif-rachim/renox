CREATE TABLE products (
    id BLOB PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    stock INTEGER NOT NULL,
    weight_kg REAL NOT NULL,
    price INTEGER NOT NULL,
    available INTEGER NOT NULL,
    size TEXT NOT NULL,
    colors TEXT NOT NULL,
    opens_at TEXT,
    launch_at TEXT,
    released_on TEXT,
    created_at TEXT,
    updated_at TEXT
);
