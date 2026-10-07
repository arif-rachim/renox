CREATE TABLE products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sku TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    -- In rupiah.
    price INTEGER NOT NULL DEFAULT 0,
    -- Kept equal to the sum of the product's stock_movements.
    stock INTEGER NOT NULL DEFAULT 0,
    -- Below this the dashboard lists it.
    min_stock INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT,
    updated_at TEXT
);

-- The stock ledger: every change to products.stock, never edited.
CREATE TABLE stock_movements (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products (id) ON DELETE CASCADE,
    -- Positive in, negative out.
    quantity INTEGER NOT NULL,
    -- received, sold, returned, counted or imported.
    reason TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    invoice_id INTEGER,
    user_name TEXT NOT NULL DEFAULT '',
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX stock_movements_product_id ON stock_movements (product_id);
