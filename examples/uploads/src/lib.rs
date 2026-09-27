//! Example: file uploads. Photos are public (anyone with the link sees
//! them); invoices are private and only reachable through a download link
//! that expires after five minutes.
//!
//! Files go to `STORAGE_PATH/app` on the local disk. For S3, Cloudflare R2
//! or MinIO, enable renox's `s3` feature and set `STORAGE_DISK=s3` and the
//! `S3_*` variables: the code stays the same.

use renox::prelude::*;

mod app;

pub use app::documents::Document;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(app::documents::Documents)
}
