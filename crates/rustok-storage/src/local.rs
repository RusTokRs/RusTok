use std::path::PathBuf;

use async_trait::async_trait;
use tracing::instrument;

use crate::{
    backend::{StorageBackend, UploadedObject},
    error::{Result, StorageError},
};

/// Local-filesystem storage driver.
///
/// Files are stored under `base_dir/<path>`.  Public URLs are constructed as
/// `<base_url>/<path>` — configure `base_url` to point at a static-file
/// server route (e.g. `/media`).
#[derive(Clone, Debug)]
pub struct LocalStorage {
    base_dir: PathBuf,
    base_url: String,
}

impl LocalStorage {
    pub fn new(base_dir: impl Into<PathBuf>, base_url: impl Into<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        }
    }

    fn resolve(&self, path: &str) -> Result<PathBuf> {
        // Guard against path traversal
        if path.contains("..") {
            return Err(StorageError::InvalidPath(path.to_string()));
        }
        Ok(self.base_dir.join(path.trim_start_matches('/')))
    }
}

#[async_trait]
impl StorageBackend for LocalStorage {
    #[instrument(skip(self, data), fields(path, size = data.len()))]
    async fn store(
        &self,
        path: &str,
        data: bytes::Bytes,
        _content_type: &str,
    ) -> Result<UploadedObject> {
        let dest = self.resolve(path)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let size = data.len() as u64;
        tokio::fs::write(&dest, &data).await?;
        Ok(UploadedObject {
            path: path.to_string(),
            public_url: self.public_url(path),
            size,
        })
    }

    #[instrument(skip(self), fields(path))]
    async fn delete(&self, path: &str) -> Result<()> {
        let dest = self.resolve(path)?;
        match tokio::fs::remove_file(&dest).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError::Io(e)),
        }
    }

    #[instrument(skip(self), fields(path))]
    async fn read(&self, path: &str) -> Result<bytes::Bytes> {
        let dest = self.resolve(path)?;
        match tokio::fs::read(&dest).await {
            Ok(bytes) => Ok(bytes::Bytes::from(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(StorageError::NotFound(path.to_string()))
            }
            Err(error) => Err(StorageError::Io(error)),
        }
    }

    async fn private_download_url(
        &self,
        _path: &str,
        _expires_in: std::time::Duration,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    fn public_url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn backend_name(&self) -> &'static str {
        "local"
    }
}

/// Config for `LocalStorage`, suitable for YAML/TOML deserialization.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LocalStorageConfig {
    /// Directory on disk where uploads are stored.
    pub base_dir: String,
    /// URL prefix exposed to clients (e.g. `/media` or `https://cdn.example.com/media`).
    pub base_url: String,
}

impl Default for LocalStorageConfig {
    fn default() -> Self {
        Self {
            base_dir: "storage/media".into(),
            base_url: "/media".into(),
        }
    }
}

impl LocalStorageConfig {
    pub fn build(&self) -> LocalStorage {
        LocalStorage::new(&self.base_dir, &self.base_url)
    }
}
