-- `customer` by default; `rnx shop:make-admin EMAIL` promotes someone.
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'customer';
