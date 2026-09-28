CREATE TABLE orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id),
    -- pending, paid, shipped or cancelled (`OrderStatus`).
    status TEXT NOT NULL DEFAULT 'pending',
    total INTEGER NOT NULL,
    address TEXT NOT NULL,
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX orders_user_id ON orders (user_id);

-- What was bought, at the price paid: products change later, orders don't.
CREATE TABLE order_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id INTEGER NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    product_id INTEGER REFERENCES products (id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    price INTEGER NOT NULL,
    quantity INTEGER NOT NULL,
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX order_items_order_id ON order_items (order_id);
