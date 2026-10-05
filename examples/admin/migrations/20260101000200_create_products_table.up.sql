CREATE TABLE products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER REFERENCES categories (id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    sku TEXT NOT NULL UNIQUE,
    price INTEGER NOT NULL DEFAULT 0,
    stock INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    featured INTEGER NOT NULL DEFAULT 0,
    released_on TEXT,
    description TEXT,
    created_at TEXT,
    updated_at TEXT,
    deleted_at TEXT
);
CREATE INDEX products_category_id ON products (category_id);
