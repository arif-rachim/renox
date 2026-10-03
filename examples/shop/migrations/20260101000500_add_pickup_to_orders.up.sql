-- Orders picked up at the store: their mails say "ready", not "on its way".
ALTER TABLE orders ADD COLUMN pickup INTEGER NOT NULL DEFAULT 0;
