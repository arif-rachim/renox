# examples/uploads

File uploads, public and private. Photos are public: anyone with the link sees them. Invoices
are private: they are reached only through a signed link that expires after five minutes, or
sent by the app itself. Read it when your app takes files from users.

```bash
cd examples/uploads
cargo run -- migrate
cargo run                        # http://127.0.0.1:3000
```

Files go to `STORAGE_PATH/app` on the local disk (`storage/app` by default). For S3,
Cloudflare R2 or MinIO, enable renox's `s3` feature and set `STORAGE_DISK=s3` and the `S3_*`
variables; the code stays the same.

## What's where

| Feature | Where |
|---|---|
| Wiring | [src/lib.rs](src/lib.rs) |
| The model, routes, upload validation, public and private storing, the expiring link, the inline download | [src/app/documents/mod.rs](src/app/documents/mod.rs) |
| The page with both forms on the UI kit's `file` field (a drop zone listing the chosen files, photos previewed; the photo form posts with htmx) | [resources/views/documents/index.html](resources/views/documents/index.html) |
| The table | [migrations](migrations) |

## Things worth copying

- **Files are checked by content, not by name.** `photo.image().max(2048)` refuses a text
  file called `x.png`; the invoice must be a PDF (`.mimes(&["pdf"])`, at most 5 MB).
- **Several files in one field.** `photos: Vec<Upload>` takes `<input type="file" multiple>`;
  `v.each(...)` checks every file, and errors come back per file (`photos.0`, `photos.1`, ...).
- **Public vs private is one call.** `store_public` puts the photo under `/storage/...` and the
  view links it with `storage_url(...)`; `store` keeps the invoice out of reach of any
  guessable URL.
- **Private files through a checked route.** `/invoices/{id}/download` redirects to
  `storage.temporary_url(...)` (five minutes); `/invoices/{id}` sends the file with
  `Download::from_storage(...).inline()` under its original name.
- **Keep the original file name.** The table stores `file_key` (where it is) and `file_name`
  (what it was called) separately.

## Tests

```bash
cargo test -p uploads
```
