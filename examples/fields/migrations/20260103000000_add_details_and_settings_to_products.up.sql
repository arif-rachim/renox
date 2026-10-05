-- Rich text (cleaned HTML from the rich text editor) and settings (JSON
-- typed in the code editor), both kept as text.
ALTER TABLE products ADD COLUMN details TEXT;
ALTER TABLE products ADD COLUMN settings TEXT;
