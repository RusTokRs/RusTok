use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "s3")]
use crate::s3::{S3Storage, S3StorageConfig};
use crate::{
    backend::{StorageBackend, UploadedObject},
    error::Result,
    local::LocalStorageConfig,
};

/// High-level storage service wrapping a concrete backend.
#[derive(Clone)]
pub struct StorageService(Arc<dyn StorageBackend>);

impl StorageService {
    /// Build from a config struct.
    pub async fn from_config(config: &StorageConfig) -> Result<Self> {
        match &config.driver {
            StorageDriver::Local => Ok(Self::new(config.local.build())),
            #[cfg(feature = "s3")]
            StorageDriver::S3 => Ok(Self::new(S3Storage::from_config(&config.s3).await?)),
        }
    }

    pub fn new(backend: impl StorageBackend + 'static) -> Self {
        Self(Arc::new(backend))
    }

    /// Generate a tenant-scoped storage path for a new upload.
    ///
    /// Format: `<tenant_id>/<year>/<month>/<random_id>.<ext>`
    pub fn generate_path(tenant_id: Uuid, original_name: &str) -> String {
        let ext = std::path::Path::new(original_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        let now = chrono::Utc::now();
        format!(
            "{}/{}/{}/{}.{}",
            tenant_id,
            now.format("%Y"),
            now.format("%m"),
            Uuid::new_v4(),
            ext
        )
    }

    pub async fn store(
        &self,
        path: &str,
        data: bytes::Bytes,
        content_type: &str,
    ) -> Result<UploadedObject> {
        self.0.store(path, data, content_type).await
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        self.0.delete(path).await
    }

    pub async fn read(&self, path: &str) -> Result<bytes::Bytes> {
        self.0.read(path).await
    }

    pub async fn private_download_url(
        &self,
        path: &str,
        expires_in: std::time::Duration,
    ) -> Result<Option<String>> {
        self.0.private_download_url(path, expires_in).await
    }

    pub fn public_url(&self, path: &str) -> String {
        self.0.public_url(path)
    }

    pub fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StorageDriver {
    #[default]
    Local,
    #[cfg(feature = "s3")]
    S3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default)]
    pub driver: StorageDriver,
    #[serde(default)]
    pub local: LocalStorageConfig,
    #[cfg(feature = "s3")]
    #[serde(default)]
    pub s3: S3StorageConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            driver: StorageDriver::Local,
            local: LocalStorageConfig::default(),
            #[cfg(feature = "s3")]
            s3: S3StorageConfig::default(),
        }
    }
}
