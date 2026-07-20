use async_trait::async_trait;
use rustok_storage::StorageService;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::{
    installation::{sha256_digest, valid_digest},
    ArtifactBlobStore, DurableArtifactBlobStore, ModuleInstallationError, StagedArtifactBlob,
};

/// Durable artifact CAS backed by platform-controlled object storage. Final
/// object names are derived only from a verified SHA-256 digest; temporary
/// uploads stay in a separate private staging prefix.
#[derive(Clone)]
pub struct StorageArtifactBlobStore {
    storage: StorageService,
    prefix: String,
}

impl StorageArtifactBlobStore {
    pub fn new(storage: StorageService) -> Self {
        Self::with_prefix(storage, "module-cas/v1")
            .expect("the built-in artifact CAS prefix is valid")
    }

    pub fn with_prefix(
        storage: StorageService,
        prefix: impl Into<String>,
    ) -> Result<Self, ModuleInstallationError> {
        let prefix = prefix.into().trim_matches('/').to_string();
        if prefix.is_empty()
            || prefix.contains("..")
            || prefix.split('/').any(|segment| segment.is_empty())
        {
            return Err(ModuleInstallationError::Blob(
                "artifact CAS prefix must be a non-empty relative object prefix".into(),
            ));
        }
        Ok(Self { storage, prefix })
    }

    fn stage_path(&self, stage_id: Uuid) -> String {
        format!("{}/staging/{stage_id}", self.prefix)
    }

    fn blob_path(&self, digest: &str) -> Result<String, ModuleInstallationError> {
        if !valid_digest(digest) {
            return Err(ModuleInstallationError::Blob(
                "artifact CAS digest must be a sha256 digest".into(),
            ));
        }
        Ok(format!("{}/blobs/sha256/{}", self.prefix, &digest[7..]))
    }

    fn storage_error(error: rustok_storage::StorageError) -> ModuleInstallationError {
        ModuleInstallationError::Blob(error.to_string())
    }

    fn verify_bytes(digest: &str, bytes: &[u8]) -> Result<(), ModuleInstallationError> {
        let actual = sha256_digest(bytes);
        if actual != digest {
            return Err(ModuleInstallationError::PayloadDigestMismatch {
                expected: digest.to_string(),
                actual,
            });
        }
        Ok(())
    }

    async fn verify_file(
        digest: &str,
        source: &std::path::Path,
    ) -> Result<u64, ModuleInstallationError> {
        let mut file = tokio::fs::File::open(source)
            .await
            .map_err(|error| ModuleInstallationError::Blob(error.to_string()))?;
        let mut hasher = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|error| ModuleInstallationError::Blob(error.to_string()))?;
            if read == 0 {
                break;
            }
            size += read as u64;
            hasher.update(&buffer[..read]);
        }
        let actual = format!("sha256:{}", hex::encode(hasher.finalize()));
        if actual != digest {
            return Err(ModuleInstallationError::PayloadDigestMismatch {
                expected: digest.to_string(),
                actual,
            });
        }
        Ok(size)
    }

    async fn publish_verified(
        &self,
        digest: &str,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<(), ModuleInstallationError> {
        Self::verify_bytes(digest, bytes)?;
        let path = self.blob_path(digest)?;
        let created = self
            .storage
            .store_if_absent(&path, bytes::Bytes::copy_from_slice(bytes), media_type)
            .await
            .map_err(Self::storage_error)?;
        if !created {
            self.get_verified(digest).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl ArtifactBlobStore for StorageArtifactBlobStore {
    async fn put_verified(
        &self,
        digest: &str,
        bytes: &[u8],
    ) -> Result<(), ModuleInstallationError> {
        self.publish_verified(digest, bytes, "application/octet-stream")
            .await
    }

    async fn get_verified(&self, digest: &str) -> Result<Vec<u8>, ModuleInstallationError> {
        let path = self.blob_path(digest)?;
        let bytes = self
            .storage
            .read(&path)
            .await
            .map_err(|error| match error {
                rustok_storage::StorageError::NotFound(_) => {
                    ModuleInstallationError::BlobNotFound(digest.to_string())
                }
                error => Self::storage_error(error),
            })?;
        Self::verify_bytes(digest, &bytes)?;
        Ok(bytes.to_vec())
    }
}

#[async_trait]
impl DurableArtifactBlobStore for StorageArtifactBlobStore {
    async fn stage(
        &self,
        expected_digest: &str,
        expected_media_type: &str,
        bytes: &[u8],
    ) -> Result<StagedArtifactBlob, ModuleInstallationError> {
        if expected_media_type.trim().is_empty() {
            return Err(ModuleInstallationError::Blob(
                "artifact media type is empty".into(),
            ));
        }
        Self::verify_bytes(expected_digest, bytes)?;
        let stage = StagedArtifactBlob {
            stage_id: Uuid::new_v4(),
            digest: expected_digest.to_string(),
            media_type: expected_media_type.to_string(),
            size_bytes: bytes.len() as u64,
        };
        self.storage
            .store(
                &self.stage_path(stage.stage_id),
                bytes::Bytes::copy_from_slice(bytes),
                expected_media_type,
            )
            .await
            .map_err(Self::storage_error)?;
        Ok(stage)
    }

    async fn stage_file(
        &self,
        expected_digest: &str,
        expected_media_type: &str,
        source: &std::path::Path,
    ) -> Result<StagedArtifactBlob, ModuleInstallationError> {
        if expected_media_type.trim().is_empty() {
            return Err(ModuleInstallationError::Blob(
                "artifact media type is empty".into(),
            ));
        }
        let size_bytes = Self::verify_file(expected_digest, source).await?;
        let stage = StagedArtifactBlob {
            stage_id: Uuid::new_v4(),
            digest: expected_digest.to_string(),
            media_type: expected_media_type.to_string(),
            size_bytes,
        };
        self.storage
            .store_file(
                &self.stage_path(stage.stage_id),
                source,
                expected_media_type,
            )
            .await
            .map_err(Self::storage_error)?;
        Ok(stage)
    }

    async fn publish(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError> {
        let stage_path = self.stage_path(staged.stage_id);
        let bytes = self
            .storage
            .read(&stage_path)
            .await
            .map_err(Self::storage_error)?;
        if bytes.len() as u64 != staged.size_bytes {
            return Err(ModuleInstallationError::Blob(
                "staged artifact size does not match admission metadata".into(),
            ));
        }
        Self::verify_bytes(&staged.digest, &bytes)?;
        self.publish_verified(&staged.digest, &bytes, &staged.media_type)
            .await?;
        self.storage
            .delete(&stage_path)
            .await
            .map_err(Self::storage_error)
    }

    async fn discard(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError> {
        self.storage
            .delete(&self.stage_path(staged.stage_id))
            .await
            .map_err(Self::storage_error)
    }

    async fn published_digests(&self) -> Result<Vec<String>, ModuleInstallationError> {
        let prefix = format!("{}/blobs/sha256", self.prefix);
        let objects = self
            .storage
            .list(&prefix)
            .await
            .map_err(Self::storage_error)?;
        let expected_prefix = format!("{prefix}/");
        objects
            .into_iter()
            .map(|object| {
                let hex = object.path.strip_prefix(&expected_prefix).ok_or_else(|| {
                    ModuleInstallationError::Blob(
                        "artifact CAS returned an object outside its prefix".into(),
                    )
                })?;
                let digest = format!("sha256:{hex}");
                if !valid_digest(&digest) || hex.contains('/') {
                    return Err(ModuleInstallationError::Blob(
                        "artifact CAS contains an invalid content-addressed object key".into(),
                    ));
                }
                Ok(digest)
            })
            .collect()
    }

    async fn delete(&self, digest: &str) -> Result<(), ModuleInstallationError> {
        let path = self.blob_path(digest)?;
        self.storage
            .delete(&path)
            .await
            .map_err(Self::storage_error)
    }
}

#[cfg(test)]
mod tests {
    use rustok_storage::{local::LocalStorage, StorageService};
    use uuid::Uuid;

    use super::*;

    fn store() -> (StorageArtifactBlobStore, std::path::PathBuf) {
        let directory =
            std::env::temp_dir().join(format!("rustok-artifact-cas-{}", Uuid::new_v4()));
        let storage = StorageService::new(LocalStorage::new(&directory, "/private"));
        (StorageArtifactBlobStore::new(storage), directory)
    }

    #[tokio::test]
    async fn publishes_digest_derived_object_and_rechecks_the_read() {
        let (store, directory) = store();
        let payload = b"durable artifact payload";
        let digest = sha256_digest(payload);
        let staged = store
            .stage(&digest, "application/vnd.rustok.rhai.source.v1", payload)
            .await
            .expect("stage artifact");
        store.publish(&staged).await.expect("publish artifact");

        assert_eq!(
            store.get_verified(&digest).await.expect("read artifact"),
            payload
        );
        assert_eq!(
            store.published_digests().await.expect("list published"),
            vec![digest]
        );
        if directory.exists() {
            tokio::fs::remove_dir_all(directory)
                .await
                .expect("remove test storage");
        }
    }

    #[tokio::test]
    async fn rejects_payload_before_it_can_enter_staging() {
        let (store, directory) = store();
        let error = store
            .stage(
                "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                "application/vnd.rustok.rhai.source.v1",
                b"tampered",
            )
            .await
            .expect_err("digest mismatch must fail");

        assert!(matches!(
            error,
            ModuleInstallationError::PayloadDigestMismatch { .. }
        ));
        if directory.exists() {
            tokio::fs::remove_dir_all(directory)
                .await
                .expect("remove test storage");
        }
    }
}
