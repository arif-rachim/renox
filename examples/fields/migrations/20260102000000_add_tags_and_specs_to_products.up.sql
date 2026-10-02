-- A tags field (a JSON list) and a key-value field (a JSON object).
ALTER TABLE products ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';
ALTER TABLE products ADD COLUMN specs TEXT NOT NULL DEFAULT '{}';
