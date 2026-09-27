CREATE TABLE produk (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nama TEXT NOT NULL,
    harga INTEGER NOT NULL,
    kategori TEXT,
    created_at TEXT,
    updated_at TEXT,
    deleted_at TEXT
);
