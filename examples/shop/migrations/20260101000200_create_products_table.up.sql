CREATE TABLE products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER REFERENCES categories (id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    -- In rupiah.
    price INTEGER NOT NULL,
    -- The database refuses to sell what isn't there, even if a check is missed.
    stock INTEGER NOT NULL DEFAULT 0 CHECK (stock >= 0),
    -- Storage key of the photo, e.g. public/products/abc.jpg.
    photo TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX products_category_id ON products (category_id);
