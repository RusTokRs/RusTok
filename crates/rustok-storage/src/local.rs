use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tracing::instrument;

use crate::{
    backend::{StorageBackend, UploadedObject},
    error::{Result, StorageError},
};

/// Local-filesystem storage driver.
///
/// Files are stored under `base_dir/<path>`. Public URLs are constructed as
/// `<base_url>/<path>` — configure `base_url` to point at a static-file server
/// route (for example `/media`).
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

    fn validated_relative_path(&self, path: &str, allow_empty: bool) -> Result<PathBuf> {
        if path.contains('\\') || path.contains('\0') || path.chars().any(char::is_control) {
            return Err(StorageError::InvalidPath(path.to_string()));
        }

        let path = path.trim_matches('/');
        if path.is_empty() {
            return if allow_empty {
                Ok(PathBuf::new())
            } else {
                Err(StorageError::InvalidPath(path.to_string()))
            };
        }

        let candidate = Path::new(path);
        if candidate.is_absolute() {
            return Err(StorageError::InvalidPath(path.to_string()));
        }

        let mut relative = PathBuf::new();
        for component in candidate.components() {
            match component {
                Component::Normal(segment) if !segment.is_empty() => relative.push(segment),
                Component::CurDir
                | Component::ParentDir
                | Component::RootDir
                | Component::Prefix(_) => {
                    return Err(StorageError::InvalidPath(path.to_string()));
                }
                Component::Normal(_) => return Err(StorageError::InvalidPath(path.to_string())),
            }
        }

        if relative.as_os_str().is_empty() && !allow_empty {
            return Err(StorageError::InvalidPath(path.to_string()));
        }
        Ok(relative)
    }

    async fn canonical_root(&self) -> Result<PathBuf> {
        tokio::fs::create_dir_all(&self.base_dir).await?;
        tokio::fs::canonicalize(&self.base_dir)
            .await
            .map_err(StorageError::Io)
    }

    async fn prepare_destination(&self, path: &str) -> Result<(PathBuf, PathBuf)> {
        let relative = self.validated_relative_path(path, false)?;
        let root = self.canonical_root().await?;
        let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
        let mut current = root.clone();

        for component in parent_relative.components() {
            let Component::Normal(segment) = component else {
                return Err(StorageError::InvalidPath(path.to_string()));
            };
            current.push(segment);
            match tokio::fs::symlink_metadata(&current).await {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(StorageError::InvalidPath(path.to_string()));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    tokio::fs::create_dir(&current).await?;
                }
                Err(error) => return Err(StorageError::Io(error)),
            }
            let canonical = tokio::fs::canonicalize(&current).await?;
            if !canonical.starts_with(&root) {
                return Err(StorageError::InvalidPath(path.to_string()));
            }
            current = canonical;
        }

        let destination = root.join(&relative);
        if let Ok(metadata) = tokio::fs::symlink_metadata(&destination).await {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(StorageError::InvalidPath(path.to_string()));
            }
        }
        Ok((root, destination))
    }

    async fn resolve_existing(&self, path: &str) -> Result<PathBuf> {
        let relative = self.validated_relative_path(path, false)?;
        let root = self.canonical_root().await?;
        let unresolved = root.join(relative);
        let resolved = match tokio::fs::canonicalize(&unresolved).await {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(StorageError::NotFound(path.to_string()));
            }
            Err(error) => return Err(StorageError::Io(error)),
        };
        if !resolved.starts_with(&root) {
            return Err(StorageError::InvalidPath(path.to_string()));
        }
        Ok(resolved)
    }

    async fn list_paths(&self, prefix: &str) -> Result<Vec<crate::StoredObject>> {
        let relative = self.validated_relative_path(prefix, true)?;
        let root = self.canonical_root().await?;
        let unresolved = root.join(relative);
        let list_root = match tokio::fs::canonicalize(&unresolved).await {
            Ok(path) if path.starts_with(&root) => path,
            Ok(_) => return Err(StorageError::InvalidPath(prefix.to_string())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(StorageError::Io(error)),
        };

        let mut pending = vec![list_root];
        let mut objects = Vec::new();
        while let Some(directory) = pending.pop() {
            let mut entries = match tokio::fs::read_dir(&directory).await {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(StorageError::Io(error)),
            };
            while let Some(entry) = entries.next_entry().await? {
                let file_type = entry.file_type().await?;
                if file_type.is_symlink() {
                    tracing::warn!(path = %entry.path().display(), "Skipping symlink in local storage tree");
                    continue;
                }
                if file_type.is_dir() {
                    let canonical = tokio::fs::canonicalize(entry.path()).await?;
                    if canonical.starts_with(&root) {
                        pending.push(canonical);
                    }
                } else if file_type.is_file() {
                    let metadata = entry.metadata().await?;
                    let relative = entry
                        .path()
                        .strip_prefix(&root)
                        .map_err(|_| StorageError::InvalidPath(prefix.to_string()))?
                        .to_string_lossy()
                        .replace('\\', "/");
                    objects.push(crate::StoredObject {
                        path: relative,
                        size: metadata.len(),
                    });
                }
            }
        }
        Ok(objects)
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
        let (_root, destination) = self.prepare_destination(path).await?;
        let parent = destination
            .parent()
            .ok_or_else(|| StorageError::InvalidPath(path.to_string()))?;
        let temporary = parent.join(format!(".upload-{}.tmp", uuid::Uuid::new_v4()));
        let size = data.len() as u64;

        if let Err(error) = tokio::fs::write(&temporary, &data).await {
            return Err(StorageError::Io(error));
        }
        if let Err(error) = tokio::fs::rename(&temporary, &destination).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(StorageError::Io(error));
        }

        Ok(UploadedObject {
            path: path.to_string(),
            public_url: self.public_url(path),
            size,
        })
    }

    async fn store_if_absent(
        &self,
        path: &str,
        data: bytes::Bytes,
        _content_type: &str,
    ) -> Result<bool> {
        let (_root, destination) = self.prepare_destination(path).await?;
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .await
        {
            Ok(mut file) => {
                file.write_all(&data).await?;
                file.flush().await?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(StorageError::Io(error)),
        }
    }

    #[instrument(skip(self), fields(path))]
    async fn delete(&self, path: &str) -> Result<()> {
        let destination = match self.resolve_existing(path).await {
            Ok(path) => path,
            Err(StorageError::NotFound(_)) => return Ok(()),
            Err(error) => return Err(error),
        };
        match tokio::fs::remove_file(&destination).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StorageError::Io(error)),
        }
    }

    #[instrument(skip(self), fields(path))]
    async fn read(&self, path: &str) -> Result<bytes::Bytes> {
        let destination = self.resolve_existing(path).await?;
        match tokio::fs::read(&destination).await {
            Ok(bytes) => Ok(bytes::Bytes::from(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(StorageError::NotFound(path.to_string()))
            }
            Err(error) => Err(StorageError::Io(error)),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<crate::StoredObject>> {
        self.list_paths(prefix).await
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

#[cfg(test)]
mod tests {
    use super::LocalStorage;

    #[test]
    fn local_storage_path_validation_rejects_traversal_and_backslashes() {
        let storage = LocalStorage::new("storage/media", "/media");
        assert!(storage.validated_relative_path("tenant/file.png", false).is_ok());
        assert!(storage.validated_relative_path("../secret", false).is_err());
        assert!(storage.validated_relative_path("tenant\\..\\secret", false).is_err());
        assert!(storage.validated_relative_path("/absolute", false).is_ok());
    }
}