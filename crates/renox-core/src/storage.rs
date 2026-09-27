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

use anyhow::{anyhow, bail};
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
    if clean {
        Ok(key)
    } else {
        Err(anyhow!("invalid storage key `{key}`").into())
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

    pub async fn put(&self, key: &str, bytes: Bytes) -> Result {
        let key = check_key(key)?;
        match &self.disk {
            Disk::Local { root } => {
                let path = root.join(key);
                if let Some(dir) = path.parent() {
                    tokio::fs::create_dir_all(dir).await?;
                }
                tokio::fs::write(path, &bytes).await?;
            }
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                store
                    .put(&object_store::path::Path::from(key), bytes.into())
                    .await
                    .map_err(anyhow::Error::from)?;
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
                Err(err) => Err(err.into()),
            },
            #[cfg(feature = "s3")]
            Disk::S3 { store, .. } => {
                use object_store::ObjectStoreExt;
                match store.get(&object_store::path::Path::from(key)).await {
                    Ok(result) => Ok(Some(result.bytes().await.map_err(anyhow::Error::from)?)),
                    Err(object_store::Error::NotFound { .. }) => Ok(None),
                    Err(err) => Err(anyhow::Error::from(err).into()),
                }
            }
        }
    }

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
                Err(err) => Err(err.into()),
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
    /// disk, presigned by S3 otherwise.
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

fn encode_path(key: &str) -> String {
    let mut out = String::new();
    crate::routing::encode(&mut out, key, true);
    out
}

/// Settings for `state.storage`, from `STORAGE_DISK` and `S3_*`.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// `local` or `s3`.
    pub disk: String,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub access_key_id: Option<String>,
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
        builder = builder
            .with_endpoint(endpoint)
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
