//! File I/O helpers for import/export operations.

use rustok_content_portability_api::PortabilityError;
use std::path::Path;
use tokio::fs;

/// Read file contents as bytes.
pub async fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>, PortabilityError> {
    let path = path.as_ref();
    fs::read(path)
        .await
        .map_err(|e| PortabilityError::io(format!("failed to read {}: {}", path.display(), e)))
}

/// Write bytes to file.
pub async fn write_file(path: impl AsRef<Path>, data: &[u8]) -> Result<(), PortabilityError> {
    let path = path.as_ref();
    fs::write(path, data)
        .await
        .map_err(|e| PortabilityError::io(format!("failed to write {}: {}", path.display(), e)))
}

/// Check if file exists.
pub async fn file_exists(path: impl AsRef<Path>) -> bool {
    fs::metadata(path.as_ref()).await.is_ok()
}

/// Get file size in bytes.
pub async fn file_size(path: impl AsRef<Path>) -> Result<u64, PortabilityError> {
    let path = path.as_ref();
    let metadata = fs::metadata(path)
        .await
        .map_err(|e| PortabilityError::io(format!("failed to stat {}: {}", path.display(), e)))?;
    Ok(metadata.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn read_and_write_file() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let data = b"hello world";
        write_file(path, data).await.unwrap();

        let read_data = read_file(path).await.unwrap();
        assert_eq!(read_data, data);
    }

    #[tokio::test]
    async fn file_exists_check() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        assert!(file_exists(path).await);
        assert!(!file_exists("/nonexistent/file.txt").await);
    }

    #[tokio::test]
    async fn file_size_check() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let data = b"hello world";
        write_file(path, data).await.unwrap();

        let size = file_size(path).await.unwrap();
        assert_eq!(size, data.len() as u64);
    }
}
