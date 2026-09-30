//! File storage on the local disk, or on S3-compatible storage (AWS S3,
//! Cloudflare R2, MinIO) with the `s3` feature.
//!
//! Keys are relative paths like `produk/abc.jpg`. Keys under `public/` are
//! public: `storage.url(key)` links to them (served from `/storage/...` for the
//! local disk, or `STORAGE_URL` for S3). Other keys are private; share them
//! with `storage.temporary_url(key, ttl)`, a link that expires.
//!
//! `STORAGE_DISK=local` (default) keeps files in `STORAGE_PATH/app`.
//! `STORAGE_DISK=s3` needs `S3_BUCKET`, `S3_REGION`, `S3_ACCESS_KEY_ID`,
//! `S3_SECRET_ACCESS_KEY`, optionally `S3_ENDPOINT` (R2/MinIO) and
//! `STORAGE_URL` (public base URL of the bucket or its CDN).

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path as UrlPath, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::signed::ValidSignature;
use crate::{AppState, Config, Error, Result};

#[derive(Clone)]
enum Disk {
    Local {
        root: PathBuf,
    },
    #[cfg(feature = "s3")]
    S3 {
        store: std::sync::Arc<object_store::aws::AmazonS3>,
        public_url: Option<String>,
    },
}

/// The app's file storage; `state.storage`.
#[derive(Clone)]
pub struct Storage {
    disk: Disk,
}

/// Rejects keys that could escape the storage root.
fn check_key(key: &str) -> Result<&str> {
    let key = key.trim_start_matches('/');
    let clean = !key.is_empty()
        && Path::new(key)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && !key.contains('\\');
    if clean && !key.contains('\0') {
        Ok(key)
    } else {
        Err(Error::BadRequest(format!(
            "invalid storage key `{}`",
            key.escape_debug()
        )))
    }
}

impl Storage {
    pub(crate) fn from_config(config: &Config) -> anyhow::Result<Self> {
        let disk = match config.storage.disk.as_str() {
            "local" => Disk::Local {
                root: config.storage_path.join("app"),
            },
            #[cfg(feature = "s3")]
            "s3" => s3(&config.storage)?,
            #[cfg(not(feature = "s3"))]
            "s3" => bail!(
                "STORAGE_DISK=s3 needs Renox's `s3` feature: renox = {{ features = [\"s3\"] }}"
            ),
            other => bail!("STORAGE_DISK must be local or s3, got `{other}`"),
        };
        Ok(Self { disk })
    }

    /// Writes `bytes` to `key`, replacing any file there.
    pub async fn put(&self, key: &str, bytes: Bytes) -> Result {
        let key = check_key(key)?;
        match &self.disk {
            Disk::Local { root } => {
                let path = root.join(key);
                if let Some(dir) = path.parent() {
                    tokio::fs::create_dir_all(dir)
                        .await
                        .with_context(|| format!("could not create {}", dir.display()))?;
                }
                tokio::fs::write(&path, &bytes)
                    .await
                    .with_context(|| format!("could not write {}", path.display()))?;
            }
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                store
                    .put(&object_store::path::Path::from(key), bytes.into())
                    .await
                    .map_err(anyhow::Error::from)
                    .with_context(|| format!("could not store `{key}`"))?;
            }
        }
        Ok(())
    }

    /// The file's bytes, or `None` if there is no such key.
    pub async fn get(&self, key: &str) -> Result<Option<Bytes>> {
        let key = check_key(key)?;
        match &self.disk {
            Disk::Local { root } => match tokio::fs::read(root.join(key)).await {
                Ok(bytes) => Ok(Some(bytes.into())),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(anyhow::Error::from(err)
                    .context(format!("could not read {}", root.join(key).display()))
                    .into()),
            },
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                match store.get(&object_store::path::Path::from(key)).await {
                    Ok(result) => Ok(Some(result.bytes().await.map_err(anyhow::Error::from)?)),
                    Err(object_store::Error::NotFound { .. }) => Ok(None),
                    Err(err) => Err(anyhow::Error::from(err)
                        .context(format!("could not read `{key}`"))
                        .into()),
                }
            }
        }
    }

    /// Whether a file exists at `key`.
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let key = check_key(key)?;
        match &self.disk {
            Disk::Local { root } => Ok(tokio::fs::try_exists(root.join(key)).await?),
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                match store.head(&object_store::path::Path::from(key)).await {
                    Ok(_) => Ok(true),
                    Err(object_store::Error::NotFound { .. }) => Ok(false),
                    Err(err) => Err(anyhow::Error::from(err).into()),
                }
            }
        }
    }

    /// Deletes the file; deleting a missing key is not an error.
    pub async fn delete(&self, key: &str) -> Result {
        let key = check_key(key)?;
        match &self.disk {
            Disk::Local { root } => match tokio::fs::remove_file(root.join(key)).await {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(anyhow::Error::from(err)
                    .context(format!("could not delete {}", root.join(key).display()))
                    .into()),
            },
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                match store.delete(&object_store::path::Path::from(key)).await {
                    Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
                    Err(err) => Err(anyhow::Error::from(err).into()),
                }
            }
        }
    }

    /// The URL of a public file (a key under `public/`).
    /// The files under `prefix` (a folder like `invoices/2026`, or `""`
    /// for all), at any depth, sorted by key.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # async fn demo(state: AppState) -> Result {
    /// for file in state.storage.list("invoices/2026").await? {
    ///     println!("{} ({} bytes)", file.key, file.size);
    /// }
    /// state.storage.copy("public/logo.png", "public/logo-old.png").await?;
    /// state.storage.rename("uploads/tmp/a.pdf", "invoices/2026/a.pdf").await?;
    /// let removed = state.storage.delete_all("uploads/tmp").await?;
    /// # let _ = removed; Ok(()) }
    /// ```
    pub async fn list(&self, prefix: &str) -> Result<Vec<FileInfo>> {
        let prefix = check_prefix(prefix)?;
        let mut files = match &self.disk {
            Disk::Local { root } => {
                let mut files = Vec::new();
                let start = if prefix.is_empty() {
                    root.clone()
                } else {
                    root.join(prefix)
                };
                let mut dirs = vec![start];
                while let Some(dir) = dirs.pop() {
                    let mut entries = match tokio::fs::read_dir(&dir).await {
                        Ok(entries) => entries,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(err) if err.kind() == std::io::ErrorKind::NotADirectory => continue,
                        Err(err) => {
                            return Err(anyhow::Error::from(err)
                                .context(format!("could not list {}", dir.display()))
                                .into());
                        }
                    };
                    while let Some(entry) = entries.next_entry().await? {
                        let meta = entry.metadata().await?;
                        if meta.is_dir() {
                            dirs.push(entry.path());
                        } else if meta.is_file() {
                            let relative = entry.path();
                            let relative = relative.strip_prefix(root).unwrap_or(&relative);
                            let key = relative
                                .components()
                                .map(|c| c.as_os_str().to_string_lossy())
                                .collect::<Vec<_>>()
                                .join("/");
                            files.push(FileInfo {
                                key,
                                size: meta.len(),
                                modified: meta.modified().ok().map(Into::into),
                            });
                        }
                    }
                }
                files
            }
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use futures_util::TryStreamExt;
                use object_store::ObjectStore;
                let path = (!prefix.is_empty()).then(|| object_store::path::Path::from(prefix));
                store
                    .list(path.as_ref())
                    .map_ok(|meta| FileInfo {
                        key: meta.location.to_string(),
                        size: meta.size,
                        modified: Some(meta.last_modified),
                    })
                    .try_collect()
                    .await
                    .map_err(anyhow::Error::from)?
            }
        };
        files.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(files)
    }

    /// The size of a file in bytes, if it exists.
    pub async fn size(&self, key: &str) -> Result<Option<u64>> {
        let key = check_key(key)?;
        match &self.disk {
            Disk::Local { root } => match tokio::fs::metadata(root.join(key)).await {
                Ok(meta) if meta.is_file() => Ok(Some(meta.len())),
                Ok(_) => Ok(None),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(anyhow::Error::from(err).into()),
            },
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                match store.head(&object_store::path::Path::from(key)).await {
                    Ok(meta) => Ok(Some(meta.size)),
                    Err(object_store::Error::NotFound { .. }) => Ok(None),
                    Err(err) => Err(anyhow::Error::from(err).into()),
                }
            }
        }
    }

    /// Copies a file, replacing `to` if it exists. A missing `from` is a
    /// 404.
    pub async fn copy(&self, from: &str, to: &str) -> Result {
        let (from, to) = (check_key(from)?, check_key(to)?);
        match &self.disk {
            Disk::Local { root } => {
                let target = root.join(to);
                create_parent(&target).await?;
                match tokio::fs::copy(root.join(from), &target).await {
                    Ok(_) => Ok(()),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(Error::NotFound),
                    Err(err) => Err(anyhow::Error::from(err)
                        .context(format!("could not copy `{from}` to `{to}`"))
                        .into()),
                }
            }
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                let (from, to) = (
                    object_store::path::Path::from(from),
                    object_store::path::Path::from(to),
                );
                match store.copy(&from, &to).await {
                    Ok(()) => Ok(()),
                    Err(object_store::Error::NotFound { .. }) => Err(Error::NotFound),
                    Err(err) => Err(anyhow::Error::from(err).into()),
                }
            }
        }
    }

    /// Moves a file (Laravel's `move`), replacing `to` if it exists. A
    /// missing `from` is a 404.
    pub async fn rename(&self, from: &str, to: &str) -> Result {
        let (from, to) = (check_key(from)?, check_key(to)?);
        match &self.disk {
            Disk::Local { root } => {
                let target = root.join(to);
                create_parent(&target).await?;
                match tokio::fs::rename(root.join(from), &target).await {
                    Ok(()) => Ok(()),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(Error::NotFound),
                    Err(err) => Err(anyhow::Error::from(err)
                        .context(format!("could not move `{from}` to `{to}`"))
                        .into()),
                }
            }
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                let (from, to) = (
                    object_store::path::Path::from(from),
                    object_store::path::Path::from(to),
                );
                match store.rename(&from, &to).await {
                    Ok(()) => Ok(()),
                    Err(object_store::Error::NotFound { .. }) => Err(Error::NotFound),
                    Err(err) => Err(anyhow::Error::from(err).into()),
                }
            }
        }
    }

    /// Deletes every file under `prefix` (not `""`: that would be all of
    /// them); returns how many.
    pub async fn delete_all(&self, prefix: &str) -> Result<usize> {
        if check_prefix(prefix)?.is_empty() {
            return Err(Error::BadRequest(
                "delete_all needs a folder, not the whole disk".into(),
            ));
        }
        let files = self.list(prefix).await?;
        for file in &files {
            self.delete(&file.key).await?;
        }
        Ok(files.len())
    }

    /// The public URL of `key` (under `public/`): `/storage/…` on the local
    /// disk, or under `STORAGE_URL` on S3 when set.
    pub fn url(&self, key: &str) -> String {
        let key = key.trim_start_matches('/');
        let rest = key.strip_prefix("public/").unwrap_or(key);
        match &self.disk {
            Disk::Local { .. } => format!("/storage/{}", encode_path(rest)),
            #[cfg(feature = "s3")]
            Disk::S3 { public_url, .. } => match public_url {
                Some(base) => format!("{}/{}", base.trim_end_matches('/'), encode_path(key)),
                None => format!("/storage/{}", encode_path(rest)),
            },
        }
    }

    /// A link to any file that works for `ttl`: signed by Renox for the local
    /// disk, presigned by S3 otherwise (for at most 7 days).
    pub async fn temporary_url(
        &self,
        state: &AppState,
        key: &str,
        ttl: Duration,
    ) -> Result<String> {
        let key = check_key(key)?;
        match &self.disk {
            Disk::Local { .. } => {
                state.sign_path(&format!("/_renox/files/{}", encode_path(key)), ttl)
            }
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::signer::Signer;
                // S3 presigns for at most 7 days.
                let ttl = ttl.min(Duration::from_secs(7 * 24 * 60 * 60));
                let url = store
                    .signed_url(
                        axum::http::Method::GET,
                        &object_store::path::Path::from(key),
                        ttl,
                    )
                    .await
                    .map_err(anyhow::Error::from)?;
                Ok(url.to_string())
            }
        }
    }

    /// Where public local files live, for serving `/storage`.
    pub(crate) fn public_root(&self) -> Option<PathBuf> {
        match &self.disk {
            Disk::Local { root } => Some(root.join("public")),
            #[cfg(feature = "s3")]
            Disk::S3 { .. } => None,
        }
    }
}

/// A folder prefix: `""` or a clean relative path.
fn check_prefix(prefix: &str) -> Result<&str> {
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() {
        return Ok(prefix);
    }
    check_key(prefix)
}

async fn create_parent(path: &Path) -> Result {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .with_context(|| format!("could not create {}", dir.display()))?;
    }
    Ok(())
}

/// A stored file; from [`Storage::list`].
#[derive(Debug, Clone, serde::Serialize)]
#[non_exhaustive]
pub struct FileInfo {
    /// Its key, e.g. `invoices/2026/a.pdf`.
    pub key: String,
    /// Its size in bytes.
    pub size: u64,
    /// When it last changed, if the disk says.
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
}

fn encode_path(key: &str) -> String {
    let mut out = String::new();
    crate::routing::encode(&mut out, key, true);
    out
}

/// Settings for `state.storage`, from `STORAGE_DISK` and `S3_*`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StorageConfig {
    /// `local` or `s3`.
    pub disk: String,
    /// The S3 bucket, from `S3_BUCKET`.
    pub bucket: Option<String>,
    /// The S3 region, from `S3_REGION`.
    pub region: Option<String>,
    /// An S3-compatible endpoint, from `S3_ENDPOINT` (AWS when unset).
    pub endpoint: Option<String>,
    /// The S3 access key id, from `S3_ACCESS_KEY_ID`.
    pub access_key_id: Option<String>,
    /// The S3 secret key, from `S3_SECRET_ACCESS_KEY`.
    pub secret_access_key: Option<String>,
    /// Public base URL for keys under `public/` (bucket URL or CDN), for S3.
    pub url: Option<String>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            disk: "local".into(),
            bucket: None,
            region: None,
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            url: None,
        }
    }
}

#[cfg(feature = "s3")]
fn s3(config: &StorageConfig) -> anyhow::Result<Disk> {
    use anyhow::Context;
    let mut builder = object_store::aws::AmazonS3Builder::new()
        .with_bucket_name(
            config
                .bucket
                .clone()
                .context("S3_BUCKET is required for STORAGE_DISK=s3")?,
        )
        .with_region(config.region.clone().unwrap_or_else(|| "auto".into()));
    if let Some(endpoint) = &config.endpoint {
        // A local MinIO or SeaweedFS usually speaks plain http.
        builder = builder
            .with_endpoint(endpoint)
            .with_allow_http(endpoint.starts_with("http://"))
            .with_virtual_hosted_style_request(false);
    }
    if let (Some(id), Some(secret)) = (&config.access_key_id, &config.secret_access_key) {
        builder = builder
            .with_access_key_id(id)
            .with_secret_access_key(secret);
    }
    Ok(Disk::S3 {
        store: std::sync::Arc::new(builder.build()?),
        public_url: config.url.clone(),
    })
}

/// `/_renox/files/{key}?expires=…&signature=…` for local temporary URLs.
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/_renox/files/{*key}", get(private_file))
        .layer(axum::middleware::map_response(
            crate::app::user_file_headers,
        ))
}

async fn private_file(
    _: ValidSignature,
    State(state): State<AppState>,
    UrlPath(key): UrlPath<String>,
) -> Result<Response> {
    let bytes = state.storage.get(&key).await?.ok_or(Error::NotFound)?;
    let upload = crate::upload::Upload {
        file_name: key.clone(),
        content_type: String::new(),
        bytes: bytes.clone(),
    };
    let content_type = upload.sniffed_type().unwrap_or("application/octet-stream");
    Ok((
        [
            (CONTENT_TYPE, content_type),
            (CACHE_CONTROL, "private, max-age=0"),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_cannot_escape_the_root() {
        assert!(check_key("produk/a.jpg").is_ok());
        assert!(check_key("/produk/a.jpg").is_ok());
        for bad in ["../etc/passwd", "produk/../../x", "", "a\\..\\b", "./x"] {
            assert!(check_key(bad).is_err(), "{bad}");
        }
    }
}
