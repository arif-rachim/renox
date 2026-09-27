//! Uploaded files. A form field of type `Upload` (or `Option<Upload>`)
//! receives the file from a `multipart/form-data` post through `Valid<T>`:
//!
//! ```
//! # use renox::prelude::*;
//! # use serde::Deserialize;
//! #[derive(Deserialize)]
//! struct ProdukForm { nama: String, foto: Option<Upload> }
//!
//! impl Validate for ProdukForm {
//!     fn rules(&self, v: &mut Validator) {
//!         v.field("nama", &self.nama).required();
//!         v.field("foto", &self.foto).image().max(2048);     // KB
//!     }
//! }
//!
//! async fn store(State(state): State<AppState>, back: Back, Valid(form): Valid<ProdukForm>) -> Result<Back> {
//!     if let Some(foto) = &form.foto {
//!         let key = foto.store_public(&state.storage, "produk").await?;   // public/produk/…jpg
//!         let url = state.storage.url(&key);
//! #       let _ = url;
//!     }
//!     Ok(back)
//! }
//! ```
//!
//! In the form: `<form method="post" enctype="multipart/form-data">`. The
//! request size limit is `UPLOAD_MAX_SIZE` (megabytes, default 10).

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

use axum::body::Bytes;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Result;
use crate::crypto::random_token;
use crate::storage::Storage;

/// A file posted in a multipart form.
#[derive(Clone)]
pub struct Upload {
    /// The name the browser gave, e.g. `foto kopi.JPG`. Don't trust it for paths.
    pub file_name: String,
    /// The type the browser declared, e.g. `image/jpeg`. Don't trust it for security.
    pub content_type: String,
    pub bytes: Bytes,
}

impl fmt::Debug for Upload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upload")
            .field("file_name", &self.file_name)
            .field("content_type", &self.content_type)
            .field("size", &self.bytes.len())
            .finish()
    }
}

impl Upload {
    pub fn size(&self) -> usize {
        self.bytes.len()
    }

    /// The lowercase extension of the original name, if it is a plain one.
    pub fn extension(&self) -> Option<String> {
        let (_, ext) = self.file_name.rsplit_once('.')?;
        let ext = ext.to_ascii_lowercase();
        (!ext.is_empty() && ext.len() <= 10 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
            .then_some(ext)
    }

    /// The type found in the file's first bytes, for the formats Renox knows.
    pub fn sniffed_type(&self) -> Option<&'static str> {
        let b = &self.bytes[..];
        if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            Some("image/png")
        } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some("image/jpeg")
        } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
            Some("image/gif")
        } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
            Some("image/webp")
        } else if b.starts_with(b"%PDF-") {
            Some("application/pdf")
        } else {
            None
        }
    }

    /// Whether the content really is a PNG, JPEG, GIF or WebP image.
    pub fn is_image(&self) -> bool {
        self.sniffed_type().is_some_and(|t| t.starts_with("image/"))
    }

    /// An extension to store the file under: from the content when it can be
    /// sniffed, otherwise from the original name, otherwise `bin`.
    fn safe_extension(&self) -> String {
        match self.sniffed_type() {
            Some("image/png") => "png".into(),
            Some("image/jpeg") => "jpg".into(),
            Some("image/gif") => "gif".into(),
            Some("image/webp") => "webp".into(),
            Some("application/pdf") => "pdf".into(),
            _ => self.extension().unwrap_or_else(|| "bin".into()),
        }
    }

    fn key(&self, dir: &str) -> String {
        let dir = dir.trim_matches('/');
        let name = format!("{}.{}", &random_token()[..24], self.safe_extension());
        if dir.is_empty() {
            name
        } else {
            format!("{dir}/{name}")
        }
    }

    /// Stores the file privately under `dir` with a random name; returns its key.
    pub async fn store(&self, storage: &Storage, dir: &str) -> Result<String> {
        let key = self.key(dir);
        storage.put(&key, self.bytes.clone()).await?;
        Ok(key)
    }

    /// Stores the file under `public/{dir}` so `storage.url(key)` can link to it.
    pub async fn store_public(&self, storage: &Storage, dir: &str) -> Result<String> {
        let key = self.key(&format!("public/{}", dir.trim_matches('/')));
        storage.put(&key, self.bytes.clone()).await?;
        Ok(key)
    }
}

/// Old input shows the file name; the bytes never go into the session.
impl Serialize for Upload {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.file_name)
    }
}

thread_local! {
    /// Files of the multipart form being deserialized on this thread.
    static UPLOADS: RefCell<HashMap<String, Upload>> = RefCell::new(HashMap::new());
}

impl<'de> Deserialize<'de> for Upload {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let token = String::deserialize(deserializer)?;
        UPLOADS
            .with(|uploads| uploads.borrow().get(&token).cloned())
            .ok_or_else(|| D::Error::custom(NOT_A_FILE))
    }
}

pub(crate) const NOT_A_FILE: &str = "expected an uploaded file";

/// Placeholder put in the form data where a file was.
pub(crate) fn token(index: usize) -> String {
    format!("\u{1}renox-upload:{index}")
}

/// Runs `f` (a synchronous deserialization) with `uploads` resolvable by token.
pub(crate) fn with_uploads<R>(uploads: &HashMap<String, Upload>, f: impl FnOnce() -> R) -> R {
    UPLOADS.with(|cell| *cell.borrow_mut() = uploads.clone());
    let result = f();
    UPLOADS.with(|cell| cell.borrow_mut().clear());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upload(name: &str, bytes: &'static [u8]) -> Upload {
        Upload {
            file_name: name.into(),
            content_type: "application/octet-stream".into(),
            bytes: Bytes::from_static(bytes),
        }
    }

    #[test]
    fn sniffs_content_instead_of_trusting_names() {
        let png = upload("foto.txt", b"\x89PNG\r\n\x1a\nrest");
        assert!(png.is_image());
        assert_eq!(png.safe_extension(), "png");
        let fake = upload("virus.jpg", b"MZ\x90\x00");
        assert!(!fake.is_image());
        assert_eq!(fake.safe_extension(), "jpg");
        assert_eq!(upload("../../etc/passwd", b"x").extension(), None);
        assert_eq!(upload("no-extension", b"x").safe_extension(), "bin");
    }

    #[test]
    fn keys_are_random_and_contained() {
        let a = upload("a.PNG", b"x").key("/produk/");
        assert!(a.starts_with("produk/") && a.ends_with(".png"), "{a}");
        assert_ne!(a, upload("a.png", b"x").key("produk"));
    }
}
