CREATE TABLE catatan (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    produk_id INTEGER REFERENCES produk (id) ON DELETE CASCADE,
    isi TEXT NOT NULL
);
CREATE INDEX catatan_produk_id ON catatan (produk_id);
