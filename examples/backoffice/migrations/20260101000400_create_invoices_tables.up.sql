CREATE TABLE invoices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- INV-00001: the settings' prefix and the id, given when it's saved.
    number TEXT NOT NULL DEFAULT '',
    customer_id INTEGER NOT NULL REFERENCES customers (id),
    -- draft, issued, paid or void.
    status TEXT NOT NULL DEFAULT 'draft',
    issued_on TEXT NOT NULL,
    due_on TEXT NOT NULL,
    -- In cents (APP_CURRENCY, USD): the lines, the tax on them, both.
    subtotal INTEGER NOT NULL DEFAULT 0,
    tax INTEGER NOT NULL DEFAULT 0,
    total INTEGER NOT NULL DEFAULT 0,
    notes TEXT NOT NULL DEFAULT '',
    -- The payment page the gateway made (Midtrans Snap or a Xendit invoice).
    payment_url TEXT,
    paid_at TEXT,
    paid_via TEXT,
    created_by TEXT NOT NULL DEFAULT '',
    updated_by TEXT NOT NULL DEFAULT '',
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX invoices_customer_id ON invoices (customer_id);
CREATE INDEX invoices_issued_on ON invoices (issued_on);

-- What was sold, at the price on the invoice: products change later.
CREATE TABLE invoice_lines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL REFERENCES invoices (id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products (id),
    description TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    unit_price INTEGER NOT NULL,
    amount INTEGER NOT NULL,
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX invoice_lines_invoice_id ON invoice_lines (invoice_id);
