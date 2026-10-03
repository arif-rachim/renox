# examples/uploads

File uploads, public and private. Photos are public: anyone with the link sees them. Invoices
are private: they are reached only through a signed link that expires after five minutes, or
sent by the app itself. Read it when your app takes files from users.

```bash
cd examples/uploads
cp .env.example .env    # optional: the settings this example reads
cargo run -- migrate
cargo run                        # http://127.0.0.1:3000
```

Try it: drop two images on the photo form (a toast says "2 photos uploaded."), upload a PDF as
an invoice, open it with View or Download (a signed link that dies after five minutes), then
delete a document from the list: the confirmation sheet asks first, and the file leaves storage
with the row.

Files go to `STORAGE_PATH/app` on the local disk (`storage/app` by default).

## On S3

The same code runs on S3 or any store that speaks its API (Cloudflare R2, DigitalOcean
Spaces, MinIO, SeaweedFS). The example's `s3` feature turns on renox's:

```bash
STORAGE_DISK=s3 S3_BUCKET=toko-uploads S3_REGION=ap-southeast-1 \
  S3_ACCESS_KEY_ID=… S3_SECRET_ACCESS_KEY=… STORAGE_URL=https://cdn.example.com \
  cargo run --features s3
```

(or put them in `.env`; `.env.example` lists them, with `S3_ENDPOINT` for stores other than
AWS). Photos are linked at `STORAGE_URL` + their key: the bucket's public address or a CDN in
front of it. Invoices stay private: Download redirects to a URL presigned by S3 that works for
five minutes, and View streams the object through the app. Deleting a document deletes the
object.

The test `on_s3::photos_and_invoices_live_on_s3` in [tests/documents.rs](tests/documents.rs)
does all of that against a real server when `TEST_S3_ENDPOINT` is set; CI's `s3` job runs it
on SeaweedFS (the commands are at the top of `crates/renox/tests/it/s3.rs`):

```bash
TEST_S3_ENDPOINT=http://127.0.0.1:8333 TEST_S3_BUCKET=renox-test \
  TEST_S3_ACCESS_KEY_ID=renox TEST_S3_SECRET_ACCESS_KEY=renox-secret \
  cargo test -p uploads --features s3
```

## What's where

| Feature | Where |
|---|---|
| Wiring | [src/lib.rs](src/lib.rs) |
| The model, routes, upload validation, public and private storing, the expiring link, the inline download, deleting a document with its file, toasts after each | [src/app/documents/mod.rs](src/app/documents/mod.rs) |
| The page with both forms on the UI kit's `file` field (a drop zone listing the chosen files, photos previewed; the photo form posts with htmx), and the uploaded documents as a kit `table` (photos as thumbnails, invoices as View/Download links, Delete behind `confirm`) or an `empty` state | [resources/views/documents/index.html](resources/views/documents/index.html) |
| The layout: the kit (`renox_ui()`), a navigation bar, `toasts()` | [resources/views/layouts/app.html](resources/views/layouts/app.html), [public/app.css](public/app.css) |
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
- **A toast survives an `HX-Redirect`.** `store_photo` returns `(Toast, htmx.redirect(…))`:
  a 303 for a plain post, `HX-Redirect` for htmx. Both load a new page, so the toast waits in
  the session and the layout's `{{ toasts() }}` shows it there.
- **Delete the file with the row.** `destroy` deletes the row, then
  `state.storage.delete(&document.file_key)` (a missing file is not an error).
- **Keep the original file name.** The table stores `file_key` (where it is) and `file_name`
  (what it was called) separately.

## Tests

```bash
cargo test -p uploads
```

[tests/documents.rs](tests/documents.rs) checks photos by content and serves them publicly,
keeps invoices behind expiring links, enforces the limits, shows the toast after each upload
(plain and htmx), and deletes a photo and an invoice together with their files.
