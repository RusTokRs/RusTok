use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "s3")]
use crate::s3::{S3Storage, S3StorageConfig};
use crate::{
    backend::{StorageBackend, StoredObject, UploadedObject},
    error::{Result, StorageError},
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
        let ext = sanitized_extension(original_name);
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
        validate_storage_path(path, false, false)?;
        self.0.store(path, data, content_type).await
    }

    pub async fn store_if_absent(
        &self,
        path: &str,
        data: bytes::Bytes,
        content_type: &str,
    ) -> Result<bool> {
        validate_storage_path(path, false, false)?;
        self.0.store_if_absent(path, data, content_type).await
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        validate_storage_path(path, false, false)?;
        self.0.delete(path).await
    }

    pub async fn read(&self, path: &str) -> Result<bytes::Bytes> {
        validate_storage_path(path, false, false)?;
        self.0.read(path).await
    }

    pub async fn list(&self, prefix: &str) -> Result<Vec<StoredObject>> {
        validate_storage_path(prefix, true, true)?;
        let normalized = prefix.strip_suffix('/').unwrap_or(prefix);
        self.0.list(normalized).await
    }

    pub async fn private_download_url(
        &self,
        path: &str,
        expires_in: std::time::Duration,
    ) -> Result<Option<String>> {
        validate_storage_path(path, false, false)?;
        self.0.private_download_url(path, expires_in).await
    }

    pub fn public_url(&self, path: &str) -> String {
        self.0.public_url(path)
    }

    pub fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }
}

fn validate_storage_path(
    path: &str,
    allow_empty: bool,
    allow_trailing_slash: bool,
) -> Result<()> {
    if path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains('\0')
        || path.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidPath(path.to_string()));
    }

    let normalized = if allow_trailing_slash {
        path.strip_suffix('/').unwrap_or(path)
    } else {
        if path.ends_with('/') {
            return Err(StorageError::InvalidPath(path.to_string()));
        }
        path
    };

    if normalized.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(StorageError::InvalidPath(path.to_string()))
        };
    }

    if normalized
        .split('/')
        .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(StorageError::InvalidPath(path.to_string()));
    }

    Ok(())
}

fn sanitized_extension(original_name: &str) -> String {
    let extension = std::path::Path::new(original_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>()
        .to_ascii_lowercase();

    if extension.is_empty() {
        "bin".to_string()
    } else {
        extension
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

#[cfg(test)]
mod tests {
    use super::{sanitized_extension, validate_storage_path};

    #[test]
    fn object_extension_is_bounded_and_path_safe() {
        assert_eq!(sanitized_extension("photo.JPEG"), "jpeg");
        assert_eq!(sanitized_extension("payload.sh;curl"), "shcurl");
        assert_eq!(sanitized_extension("no-extension"), "bin");
        assert_eq!(
            sanitized_extension("asset.abcdefghijklmnopqrstuvwxyz"),
            "abcdefghijklmnop"
        );
    }

    #[test]
    fn service_boundary_rejects_absolute_and_traversal_paths() {
        assert!(validate_storage_path("tenant/file.png", false, false).is_ok());
        assert!(validate_storage_path("tenant/", true, true).is_ok());
        assert!(validate_storage_path("", true, true).is_ok());
        assert!(validate_storage_path("/absolute", false, false).is_err());
        assert!(validate_storage_path("../secret", false, false).is_err());
        assert!(validate_storage_path("tenant//file", false, false).is_err());
        assert!(validate_storage_path("tenant\\file", false, false).is_err());
    }
}
