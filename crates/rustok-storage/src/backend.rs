use async_trait::async_trait;

use crate::error::Result;

/// Metadata returned after a successful upload.
#[derive(Debug, Clone)]
pub struct UploadedObject {
    /// Relative storage path (driver-specific).
    pub path: String,
    /// Public URL for serving the file (may be empty for private buckets).
    pub public_url: String,
    /// Final size in bytes as stored.
    pub size: u64,
}

/// Contract every storage driver must implement.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store `data` at the given relative `path`.
    ///
    /// Returns the stored path (may differ if the backend normalises it).
    async fn store(
        &self,
        path: &str,
        data: bytes::Bytes,
        content_type: &str,
    ) -> Result<UploadedObject>;

    /// Remove the object at `path`.  Idempotent — missing objects return `Ok`.
    async fn delete(&self, path: &str) -> Result<()>;

    /// Read the raw object bytes for private download or validation flows.
    async fn read(&self, path: &str) -> Result<bytes::Bytes>;

    /// Resolve a private download URL when the backend supports native presigning.
    async fn private_download_url(
        &self,
        path: &str,
        expires_in: std::time::Duration,
    ) -> Result<Option<String>>;

    /// Resolve the public URL for a stored path.
    fn public_url(&self, path: &str) -> String;

    /// Stable backend identifier for diagnostics and persisted metadata.
    fn backend_name(&self) -> &'static str;
}
