//! Canonical `SourceObjectStore` for media-neutral source object CAS.
//!
//! Under the repository release-safety contract:
//! - `rustok-modules` preparation owns the single `SourceObjectStore`.
//! - Blobs are media-type neutral and globally deduplicated by SHA-256 digest (`source_digest`).
//! - Each publication records an owner/RLS-scoped `SourceReceipt` over `(preparation_id, source_digest, media_type, byte_length, manifest_digest)`.
//! - Same-request / same-preparation idempotency converges without overwriting existing blobs.
//! - Durable retention holds protect active sources from premature GC.
//!
//! Storage properties:
//! - Blobs live in a two-level fan-out layout,
//!   `<cas_root>/<hex[0..2]>/<hex[2..4]>/<hex>`, so directory fan-out stays
//!   bounded as the CAS grows. The layout is not migrated in place:
//!   pre-fan-out CAS directories must be discarded and republished.
//! - All file I/O on async paths is non-blocking and streamed in bounded
//!   chunks; objects larger than [`MAX_SOURCE_OBJECT_BYTES`] are rejected.
//! - Existing blobs are content-verified (streamed SHA-256) on every publish
//!   and on every read that claims verification; a corrupt blob surfaces as
//!   [`SourceObjectError::DigestMismatch`] and is never silently overwritten.
//! - Receipt reads require an explicit calling scope; there is no
//!   scope-less read path that skips the tenant boundary.
//! - Blobs are published before their receipt (a receipt must never point at
//!   a missing blob). A failed receipt insert removes a freshly created,
//!   still-unreferenced blob; blobs orphaned by process crashes are owned by
//!   retention GC (see the source-CAS adapter design in `gc.rs`, which is
//!   accepted-unfinished and not yet wired).

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::{ModuleInstallationScope, installation::valid_digest};

static SOURCE_UPLOAD_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Defensive ceiling on a single source object. Build preparation enforces
/// stricter archive limits before publication; this bound exists so no
/// publish or read path can be forced into unbounded memory or disk work.
pub const MAX_SOURCE_OBJECT_BYTES: u64 = 64 * 1024 * 1024;

const STREAM_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum SourceObjectError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Database error: {0}")]
    Storage(String),
    #[error("Digest mismatch: expected `{expected}`, actual `{actual}`")]
    DigestMismatch { expected: String, actual: String },
    #[error("Invalid digest `{0}`")]
    InvalidDigest(String),
    #[error("Source object with digest `{0}` not found")]
    NotFound(String),
    #[error("Source object of {actual} bytes exceeds the {limit}-byte limit")]
    TooLarge { actual: u64, limit: u64 },
    #[error(
        "Unauthorized tenant `{target_tenant}` for source receipt `{receipt_id}` belonging to another tenant"
    )]
    UnauthorizedTenant {
        receipt_id: Uuid,
        target_tenant: Uuid,
    },
    #[error("Source reference `{0}` violates path or CAS constraints")]
    InvalidReference(String),
}

fn io_error(error: std::io::Error) -> SourceObjectError {
    SourceObjectError::Io(error.to_string())
}

/// Immutable receipt for an admitted source object within an owner/preparation domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceReceipt {
    pub source_receipt_id: Uuid,
    pub preparation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub source_digest: String,
    pub media_type: String,
    pub byte_length: u64,
    pub manifest_digest: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Durable retention hold preventing garbage collection of a source object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceObjectRetentionHold {
    pub hold_id: Uuid,
    pub source_digest: String,
    pub held_by: String,
    pub reason: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct SourceObjectStore {
    db: DatabaseConnection,
    cas_root: PathBuf,
}

impl SourceObjectStore {
    pub fn new(db: DatabaseConnection, cas_root: PathBuf) -> Result<Self, SourceObjectError> {
        if !cas_root.is_absolute() {
            return Err(SourceObjectError::Io(
                "Source CAS root path must be absolute".to_string(),
            ));
        }
        if !cas_root.exists() {
            fs::create_dir_all(&cas_root).map_err(|e| SourceObjectError::Io(e.to_string()))?;
        }
        let canonical_root =
            fs::canonicalize(&cas_root).map_err(|e| SourceObjectError::Io(e.to_string()))?;
        Ok(Self {
            db,
            cas_root: canonical_root,
        })
    }

    pub fn cas_root(&self) -> &Path {
        &self.cas_root
    }

    /// Extracts hex string from a valid `sha256:<hex>` digest string.
    fn extract_hex<'a>(&self, digest: &'a str) -> Result<&'a str, SourceObjectError> {
        if !valid_digest(digest) {
            return Err(SourceObjectError::InvalidDigest(digest.to_string()));
        }
        Ok(&digest[7..])
    }

    /// Returns the canonical destination path for a given digest in the CAS
    /// root: two-level fan-out `<cas_root>/<hex[0..2]>/<hex[2..4]>/<hex>`,
    /// without extension. This computes a path only; it does not assert that
    /// the blob exists or is intact (use [`Self::get_source_path`] for that).
    pub fn destination_path(&self, source_digest: &str) -> Result<PathBuf, SourceObjectError> {
        let hex = self.extract_hex(source_digest)?;
        // `valid_digest` guarantees exactly 64 lowercase hex characters.
        Ok(self.cas_root.join(&hex[0..2]).join(&hex[2..4]).join(hex))
    }

    /// Atomically publishes a raw in-memory byte slice as a canonical source
    /// object. The slice is streamed to disk in bounded chunks while its
    /// SHA-256 is computed; nothing is committed on digest mismatch.
    pub async fn publish_source_blob(
        &self,
        preparation_id: Uuid,
        scope: &ModuleInstallationScope,
        media_type: &str,
        expected_digest: &str,
        bytes: &[u8],
        manifest_digest: Option<&str>,
    ) -> Result<SourceReceipt, SourceObjectError> {
        self.validate_digests(expected_digest, manifest_digest)?;
        let byte_length = bytes.len() as u64;
        if byte_length > MAX_SOURCE_OBJECT_BYTES {
            return Err(SourceObjectError::TooLarge {
                actual: byte_length,
                limit: MAX_SOURCE_OBJECT_BYTES,
            });
        }

        if let Some(existing) = self
            .find_receipt_by_preparation(preparation_id, expected_digest, scope)
            .await?
        {
            return Ok(existing);
        }

        let destination = self.destination_path(expected_digest)?;
        let created = self
            .commit_blob_from_bytes(&destination, expected_digest, bytes)
            .await?;
        self.finish_publish(
            preparation_id,
            scope,
            media_type,
            expected_digest,
            byte_length,
            manifest_digest,
            &destination,
            created,
        )
        .await
    }

    /// Publishes a source file from an existing path into the source CAS,
    /// streaming it in bounded chunks (the file is never loaded whole into
    /// memory) and rejecting files above [`MAX_SOURCE_OBJECT_BYTES`].
    pub async fn publish_source_file(
        &self,
        preparation_id: Uuid,
        scope: &ModuleInstallationScope,
        media_type: &str,
        expected_digest: &str,
        file_path: &Path,
        manifest_digest: Option<&str>,
    ) -> Result<SourceReceipt, SourceObjectError> {
        self.validate_digests(expected_digest, manifest_digest)?;

        let metadata = tokio::fs::metadata(file_path).await.map_err(io_error)?;
        if metadata.len() > MAX_SOURCE_OBJECT_BYTES {
            return Err(SourceObjectError::TooLarge {
                actual: metadata.len(),
                limit: MAX_SOURCE_OBJECT_BYTES,
            });
        }

        if let Some(existing) = self
            .find_receipt_by_preparation(preparation_id, expected_digest, scope)
            .await?
        {
            // The receipt asserts the blob is published; verify the source
            // file really is the expected content before converging.
            let (actual_digest, _) = Self::hash_file(file_path).await?;
            if actual_digest != expected_digest {
                return Err(SourceObjectError::DigestMismatch {
                    expected: expected_digest.to_string(),
                    actual: actual_digest,
                });
            }
            return Ok(existing);
        }

        let destination = self.destination_path(expected_digest)?;
        let (created, byte_length) = self
            .commit_blob_from_file(&destination, expected_digest, file_path)
            .await?;
        self.finish_publish(
            preparation_id,
            scope,
            media_type,
            expected_digest,
            byte_length,
            manifest_digest,
            &destination,
            created,
        )
        .await
    }

    fn validate_digests(
        &self,
        expected_digest: &str,
        manifest_digest: Option<&str>,
    ) -> Result<(), SourceObjectError> {
        if !valid_digest(expected_digest) {
            return Err(SourceObjectError::InvalidDigest(
                expected_digest.to_string(),
            ));
        }
        if let Some(m_digest) = manifest_digest
            && !valid_digest(m_digest)
        {
            return Err(SourceObjectError::InvalidDigest(m_digest.to_string()));
        }
        Ok(())
    }

    /// Inserts the receipt after the blob is committed. If the insert fails
    /// and this call created the blob, the still-unreferenced blob is removed
    /// so a handled failure does not orphan it; cleanup failures are reported
    /// together with the original error, never suppressed.
    #[allow(clippy::too_many_arguments)]
    async fn finish_publish(
        &self,
        preparation_id: Uuid,
        scope: &ModuleInstallationScope,
        media_type: &str,
        source_digest: &str,
        byte_length: u64,
        manifest_digest: Option<&str>,
        destination: &Path,
        created: bool,
    ) -> Result<SourceReceipt, SourceObjectError> {
        match self
            .insert_receipt(
                preparation_id,
                scope,
                media_type,
                source_digest,
                byte_length,
                manifest_digest,
            )
            .await
        {
            Ok(receipt) => Ok(receipt),
            Err(error) => {
                if created
                    && let Err(cleanup) = self
                        .remove_unreferenced_blob(destination, source_digest)
                        .await
                {
                    return Err(SourceObjectError::Storage(format!(
                        "{error}; additionally failed to remove orphaned blob \
                         {source_digest}: {cleanup}"
                    )));
                }
                Err(error)
            }
        }
    }

    /// Removes a blob only when no receipt references its digest, so a
    /// concurrent publication of the same content keeps its blob.
    async fn remove_unreferenced_blob(
        &self,
        destination: &Path,
        source_digest: &str,
    ) -> Result<(), SourceObjectError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT COUNT(*) AS refs FROM module_source_object_receipts \
                     WHERE source_digest = {}",
                    backend.placeholder(1),
                ),
                vec![source_digest.into()],
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?
            .ok_or_else(|| {
                SourceObjectError::Storage("receipt reference count query returned no row".into())
            })?;
        let refs: i64 = row
            .try_get("", "refs")
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        if refs == 0 {
            tokio::fs::remove_file(destination)
                .await
                .map_err(io_error)?;
        }
        Ok(())
    }

    /// Writes `bytes` to a temp file in the destination shard while hashing,
    /// then atomically links it into place. If the destination already
    /// exists, its content is verified instead. Returns whether this call
    /// created the blob.
    async fn commit_blob_from_bytes(
        &self,
        destination: &Path,
        expected_digest: &str,
        bytes: &[u8],
    ) -> Result<bool, SourceObjectError> {
        let shard_dir = destination.parent().ok_or_else(|| {
            SourceObjectError::Io("CAS destination has no parent directory".to_string())
        })?;
        tokio::fs::create_dir_all(shard_dir)
            .await
            .map_err(io_error)?;

        if tokio::fs::try_exists(destination).await.map_err(io_error)? {
            Self::verify_blob(destination, expected_digest).await?;
            return Ok(false);
        }

        let temp_path = self.temp_path(shard_dir);
        let write_result = async {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .await
                .map_err(io_error)?;
            let mut hasher = Sha256::new();
            for chunk in bytes.chunks(STREAM_CHUNK_BYTES) {
                file.write_all(chunk).await.map_err(io_error)?;
                hasher.update(chunk);
            }
            file.sync_all().await.map_err(io_error)?;
            Ok::<String, SourceObjectError>(format!("sha256:{}", hex::encode(hasher.finalize())))
        }
        .await;

        let computed_digest = match write_result {
            Ok(digest) => digest,
            Err(error) => {
                tokio::fs::remove_file(&temp_path).await.map_err(io_error)?;
                return Err(error);
            }
        };
        if computed_digest != expected_digest {
            tokio::fs::remove_file(&temp_path).await.map_err(io_error)?;
            return Err(SourceObjectError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: computed_digest,
            });
        }

        self.link_into_place(&temp_path, destination, expected_digest)
            .await
    }

    /// Streams a source file into a temp file in the destination shard while
    /// hashing, then atomically links it into place. Returns whether this
    /// call created the blob and the streamed byte length.
    async fn commit_blob_from_file(
        &self,
        destination: &Path,
        expected_digest: &str,
        file_path: &Path,
    ) -> Result<(bool, u64), SourceObjectError> {
        let shard_dir = destination.parent().ok_or_else(|| {
            SourceObjectError::Io("CAS destination has no parent directory".to_string())
        })?;
        tokio::fs::create_dir_all(shard_dir)
            .await
            .map_err(io_error)?;

        if tokio::fs::try_exists(destination).await.map_err(io_error)? {
            let byte_length = Self::verify_blob(destination, expected_digest).await?;
            // Converging on the existing blob still requires the offered
            // file to be the expected content.
            let (actual_digest, _) = Self::hash_file(file_path).await?;
            if actual_digest != expected_digest {
                return Err(SourceObjectError::DigestMismatch {
                    expected: expected_digest.to_string(),
                    actual: actual_digest,
                });
            }
            return Ok((false, byte_length));
        }

        let temp_path = self.temp_path(shard_dir);
        let write_result = async {
            let mut source = tokio::fs::File::open(file_path).await.map_err(io_error)?;
            let mut temp = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .await
                .map_err(io_error)?;
            let mut hasher = Sha256::new();
            let mut total: u64 = 0;
            let mut buffer = vec![0u8; STREAM_CHUNK_BYTES];
            loop {
                let read = source.read(&mut buffer).await.map_err(io_error)?;
                if read == 0 {
                    break;
                }
                total += read as u64;
                if total > MAX_SOURCE_OBJECT_BYTES {
                    return Err(SourceObjectError::TooLarge {
                        actual: total,
                        limit: MAX_SOURCE_OBJECT_BYTES,
                    });
                }
                temp.write_all(&buffer[..read]).await.map_err(io_error)?;
                hasher.update(&buffer[..read]);
            }
            temp.sync_all().await.map_err(io_error)?;
            Ok::<(String, u64), SourceObjectError>((
                format!("sha256:{}", hex::encode(hasher.finalize())),
                total,
            ))
        }
        .await;

        let (computed_digest, byte_length) = match write_result {
            Ok(result) => result,
            Err(error) => {
                tokio::fs::remove_file(&temp_path).await.map_err(io_error)?;
                return Err(error);
            }
        };
        if computed_digest != expected_digest {
            tokio::fs::remove_file(&temp_path).await.map_err(io_error)?;
            return Err(SourceObjectError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: computed_digest,
            });
        }

        let created = self
            .link_into_place(&temp_path, destination, expected_digest)
            .await?;
        Ok((created, byte_length))
    }

    /// Atomically moves a verified temp blob to its destination via hard
    /// link (rename fallback). Losing a race against a concurrent publisher
    /// converges on the winner's blob after verifying it.
    async fn link_into_place(
        &self,
        temp_path: &Path,
        destination: &Path,
        expected_digest: &str,
    ) -> Result<bool, SourceObjectError> {
        match tokio::fs::hard_link(temp_path, destination).await {
            Ok(()) => {
                tokio::fs::remove_file(temp_path).await.map_err(io_error)?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                tokio::fs::remove_file(temp_path).await.map_err(io_error)?;
                Self::verify_blob(destination, expected_digest).await?;
                Ok(false)
            }
            Err(_) => match tokio::fs::rename(temp_path, destination).await {
                Ok(()) => Ok(true),
                Err(rename_error) => {
                    tokio::fs::remove_file(temp_path).await.map_err(io_error)?;
                    if tokio::fs::try_exists(destination).await.map_err(io_error)? {
                        Self::verify_blob(destination, expected_digest).await?;
                        Ok(false)
                    } else {
                        Err(io_error(rename_error))
                    }
                }
            },
        }
    }

    fn temp_path(&self, shard_dir: &Path) -> PathBuf {
        let seq = SOURCE_UPLOAD_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        shard_dir.join(format!(".upload-{}-{seq}.tmp", std::process::id()))
    }

    /// Streams a file through SHA-256, enforcing the size ceiling. Returns
    /// the `sha256:<hex>` digest and the byte length.
    async fn hash_file(path: &Path) -> Result<(String, u64), SourceObjectError> {
        let mut file = tokio::fs::File::open(path).await.map_err(io_error)?;
        let mut hasher = Sha256::new();
        let mut total: u64 = 0;
        let mut buffer = vec![0u8; STREAM_CHUNK_BYTES];
        loop {
            let read = file.read(&mut buffer).await.map_err(io_error)?;
            if read == 0 {
                break;
            }
            total += read as u64;
            if total > MAX_SOURCE_OBJECT_BYTES {
                return Err(SourceObjectError::TooLarge {
                    actual: total,
                    limit: MAX_SOURCE_OBJECT_BYTES,
                });
            }
            hasher.update(&buffer[..read]);
        }
        Ok((format!("sha256:{}", hex::encode(hasher.finalize())), total))
    }

    /// Verifies that the blob at `path` hashes to `expected_digest`.
    async fn verify_blob(path: &Path, expected_digest: &str) -> Result<u64, SourceObjectError> {
        let (actual_digest, byte_length) = Self::hash_file(path).await?;
        if actual_digest != expected_digest {
            return Err(SourceObjectError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: actual_digest,
            });
        }
        Ok(byte_length)
    }

    async fn insert_receipt(
        &self,
        preparation_id: Uuid,
        scope: &ModuleInstallationScope,
        media_type: &str,
        source_digest: &str,
        byte_length: u64,
        manifest_digest: Option<&str>,
    ) -> Result<SourceReceipt, SourceObjectError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let receipt_id = Uuid::new_v4();
        let now = Utc::now();

        let (scope_kind, scope_tenant_key) = match scope {
            ModuleInstallationScope::Platform => ("platform", "".to_string()),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
        };

        let insert_sql = format!(
            "INSERT INTO module_source_object_receipts (\
                source_receipt_id, preparation_id, scope_kind, scope_tenant_key, \
                source_digest, media_type, byte_length, manifest_digest, created_at\
             ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {})\
             ON CONFLICT (preparation_id, source_digest) DO NOTHING",
            backend.placeholder(1),
            backend.placeholder(2),
            backend.placeholder(3),
            backend.placeholder(4),
            backend.placeholder(5),
            backend.placeholder(6),
            backend.placeholder(7),
            backend.placeholder(8),
            backend.placeholder(9),
        );

        let manifest_val = match manifest_digest {
            Some(d) => Value::from(d.to_string()),
            None => Value::from(None::<String>),
        };

        let values = vec![
            backend.uuid_value(receipt_id),
            backend.uuid_value(preparation_id),
            scope_kind.into(),
            scope_tenant_key.into(),
            source_digest.into(),
            media_type.into(),
            (byte_length as i64).into(),
            manifest_val,
            now.to_rfc3339().into(),
        ];

        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                insert_sql,
                values,
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        // Re-read to guarantee we return the canonical receipt (whether inserted or pre-existing)
        let receipt = self
            .find_receipt_by_preparation(preparation_id, source_digest, scope)
            .await?
            .ok_or_else(|| {
                SourceObjectError::Storage("Failed to read back inserted receipt".to_string())
            })?;

        Ok(receipt)
    }

    /// Reads back full content bytes of a source object after streamed
    /// digest verification. Objects above [`MAX_SOURCE_OBJECT_BYTES`] are
    /// rejected before buffering.
    pub async fn get_source_blob(&self, source_digest: &str) -> Result<Vec<u8>, SourceObjectError> {
        let path = self.destination_path(source_digest)?;
        let mut file = match tokio::fs::File::open(&path).await {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(SourceObjectError::NotFound(source_digest.to_string()));
            }
            Err(error) => return Err(io_error(error)),
        };
        let metadata = file.metadata().await.map_err(io_error)?;
        if metadata.len() > MAX_SOURCE_OBJECT_BYTES {
            return Err(SourceObjectError::TooLarge {
                actual: metadata.len(),
                limit: MAX_SOURCE_OBJECT_BYTES,
            });
        }

        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; STREAM_CHUNK_BYTES];
        loop {
            let read = file.read(&mut buffer).await.map_err(io_error)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            bytes.extend_from_slice(&buffer[..read]);
        }
        let computed = format!("sha256:{}", hex::encode(hasher.finalize()));
        if computed != source_digest {
            return Err(SourceObjectError::DigestMismatch {
                expected: source_digest.to_string(),
                actual: computed,
            });
        }

        Ok(bytes)
    }

    /// Returns the path of a source object only after streamed digest
    /// verification of the blob on disk. Callers that merely need the
    /// canonical location without integrity guarantees can use
    /// [`Self::destination_path`].
    pub async fn get_source_path(&self, source_digest: &str) -> Result<PathBuf, SourceObjectError> {
        let path = self.destination_path(source_digest)?;
        if !tokio::fs::try_exists(&path).await.map_err(io_error)? {
            return Err(SourceObjectError::NotFound(source_digest.to_string()));
        }
        Self::verify_blob(&path, source_digest).await?;
        Ok(path)
    }

    /// Finds a receipt by `(preparation_id, source_digest)`. The calling
    /// scope is mandatory and the tenant boundary is always enforced:
    /// a tenant scope may read platform receipts and its own, never another
    /// tenant's.
    pub async fn find_receipt_by_preparation(
        &self,
        preparation_id: Uuid,
        source_digest: &str,
        calling_scope: &ModuleInstallationScope,
    ) -> Result<Option<SourceReceipt>, SourceObjectError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let query_sql = format!(
            "SELECT source_receipt_id, preparation_id, scope_kind, scope_tenant_key, \
                    source_digest, media_type, byte_length, manifest_digest, created_at \
             FROM module_source_object_receipts \
             WHERE preparation_id = {} AND source_digest = {}",
            backend.placeholder(1),
            backend.placeholder(2)
        );

        let values = vec![backend.uuid_value(preparation_id), source_digest.into()];

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                query_sql,
                values,
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        match row {
            Some(r) => {
                let receipt = self.parse_receipt_row(r, backend)?;
                self.enforce_rls_boundary(&receipt, calling_scope)?;
                Ok(Some(receipt))
            }
            None => Ok(None),
        }
    }

    /// Finds a receipt by `source_receipt_id` under the same mandatory
    /// calling-scope boundary as [`Self::find_receipt_by_preparation`].
    pub async fn get_receipt(
        &self,
        source_receipt_id: Uuid,
        calling_scope: &ModuleInstallationScope,
    ) -> Result<Option<SourceReceipt>, SourceObjectError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let query_sql = format!(
            "SELECT source_receipt_id, preparation_id, scope_kind, scope_tenant_key, \
                    source_digest, media_type, byte_length, manifest_digest, created_at \
             FROM module_source_object_receipts \
             WHERE source_receipt_id = {}",
            backend.placeholder(1)
        );

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                query_sql,
                vec![backend.uuid_value(source_receipt_id)],
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        match row {
            Some(r) => {
                let receipt = self.parse_receipt_row(r, backend)?;
                self.enforce_rls_boundary(&receipt, calling_scope)?;
                Ok(Some(receipt))
            }
            None => Ok(None),
        }
    }

    fn enforce_rls_boundary(
        &self,
        receipt: &SourceReceipt,
        calling_scope: &ModuleInstallationScope,
    ) -> Result<(), SourceObjectError> {
        if let (
            ModuleInstallationScope::Tenant { tenant_id },
            ModuleInstallationScope::Tenant {
                tenant_id: owner_id,
            },
        ) = (calling_scope, &receipt.scope)
            && tenant_id != owner_id
        {
            return Err(SourceObjectError::UnauthorizedTenant {
                receipt_id: receipt.source_receipt_id,
                target_tenant: *tenant_id,
            });
        }
        Ok(())
    }

    fn parse_receipt_row(
        &self,
        row: sea_orm::QueryResult,
        backend: SupportedBackend,
    ) -> Result<SourceReceipt, SourceObjectError> {
        let source_receipt_id: Uuid = match backend {
            SupportedBackend::Sqlite => {
                let s: String = row
                    .try_get("", "source_receipt_id")
                    .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
                Uuid::parse_str(&s).map_err(|e| SourceObjectError::Storage(e.to_string()))?
            }
            SupportedBackend::Postgres => row
                .try_get("", "source_receipt_id")
                .map_err(|e| SourceObjectError::Storage(e.to_string()))?,
        };

        let preparation_id: Uuid = match backend {
            SupportedBackend::Sqlite => {
                let s: String = row
                    .try_get("", "preparation_id")
                    .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
                Uuid::parse_str(&s).map_err(|e| SourceObjectError::Storage(e.to_string()))?
            }
            SupportedBackend::Postgres => row
                .try_get("", "preparation_id")
                .map_err(|e| SourceObjectError::Storage(e.to_string()))?,
        };

        let scope_kind: String = row
            .try_get("", "scope_kind")
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let scope_tenant_key: String = row
            .try_get("", "scope_tenant_key")
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        let scope = if scope_kind == "platform" {
            ModuleInstallationScope::Platform
        } else {
            let tenant_id = Uuid::parse_str(&scope_tenant_key)
                .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
            ModuleInstallationScope::Tenant { tenant_id }
        };

        let source_digest: String = row
            .try_get("", "source_digest")
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let media_type: String = row
            .try_get("", "media_type")
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let byte_length: i64 = row
            .try_get("", "byte_length")
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let manifest_digest: Option<String> = row
            .try_get("", "manifest_digest")
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        let created_at: DateTime<Utc> = match backend {
            SupportedBackend::Sqlite => {
                let s: String = row
                    .try_get("", "created_at")
                    .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
                crate::backend::parse_sqlite_datetime(&s)
                    .map_err(|e| SourceObjectError::Storage(e.to_string()))?
            }
            SupportedBackend::Postgres => row
                .try_get("", "created_at")
                .map_err(|e| SourceObjectError::Storage(e.to_string()))?,
        };

        Ok(SourceReceipt {
            source_receipt_id,
            preparation_id,
            scope,
            source_digest,
            media_type,
            byte_length: byte_length as u64,
            manifest_digest,
            created_at,
        })
    }

    /// Adds a durable retention hold for a source object.
    pub async fn add_retention_hold(
        &self,
        source_digest: &str,
        held_by: &str,
        reason: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Uuid, SourceObjectError> {
        if !valid_digest(source_digest) {
            return Err(SourceObjectError::InvalidDigest(source_digest.to_string()));
        }

        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let hold_id = Uuid::new_v4();
        let now = Utc::now();

        let insert_sql = format!(
            "INSERT INTO module_source_object_retention_holds (\
                hold_id, source_digest, held_by, reason, expires_at, created_at\
             ) VALUES ({}, {}, {}, {}, {}, {})",
            backend.placeholder(1),
            backend.placeholder(2),
            backend.placeholder(3),
            backend.placeholder(4),
            backend.placeholder(5),
            backend.placeholder(6),
        );

        let exp_val = match expires_at {
            Some(exp) => Value::from(exp.to_rfc3339()),
            None => Value::from(None::<String>),
        };

        let values = vec![
            backend.uuid_value(hold_id),
            source_digest.into(),
            held_by.into(),
            reason.into(),
            exp_val,
            now.to_rfc3339().into(),
        ];

        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                insert_sql,
                values,
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        Ok(hold_id)
    }

    /// Releases an existing retention hold by ID.
    pub async fn release_retention_hold(&self, hold_id: Uuid) -> Result<(), SourceObjectError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let delete_sql = format!(
            "DELETE FROM module_source_object_retention_holds WHERE hold_id = {}",
            backend.placeholder(1)
        );

        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                delete_sql,
                vec![backend.uuid_value(hold_id)],
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Checks if a source object is currently protected by an active retention hold.
    pub async fn is_held(&self, source_digest: &str) -> Result<bool, SourceObjectError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
        let query_sql = format!(
            "SELECT 1 FROM module_source_object_retention_holds \
             WHERE source_digest = {} \
               AND (expires_at IS NULL OR expires_at > {}) \
             LIMIT 1",
            backend.placeholder(1),
            backend.placeholder(2),
        );

        let now = Utc::now().to_rfc3339();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                query_sql,
                vec![source_digest.into(), now.into()],
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        Ok(row.is_some())
    }
}
