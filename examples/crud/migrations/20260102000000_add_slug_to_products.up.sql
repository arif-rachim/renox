-- Filled by the `saving` hook (src/app/products/model.rs), never by forms.
ALTER TABLE products ADD COLUMN slug TEXT NOT NULL DEFAULT '';
