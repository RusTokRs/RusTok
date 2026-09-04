//! Canonical `SourceObjectStore` for media-neutral source object CAS.
//!
//! Under the repository release-safety contract:
//! - `rustok-modules` preparation owns the single `SourceObjectStore`.
//! - Blobs are media-type neutral and globally deduplicated by SHA-256 digest (`source_digest`).
//! - Each publication records an owner/RLS-scoped `SourceReceipt` over `(preparation_id, source_digest, media_type, byte_length, manifest_digest)`.
//! - Same-request / same-preparation idempotency converges without overwriting existing blobs.
//! - Durable retention holds protect active sources from premature GC.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ModuleInstallationScope,
    data::{placeholder, uuid_value},
    installation::{sha256_digest, valid_digest},
};

static SOURCE_UPLOAD_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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
        let canonical_root = fs::canonicalize(&cas_root).map_err(|e| SourceObjectError::Io(e.to_string()))?;
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

    /// Returns the canonical destination path for a given digest in the CAS root.
    /// Notice: media-neutral layout directly under `<cas_root>/<digest_hex>`, without extension.
    pub fn destination_path(&self, source_digest: &str) -> Result<PathBuf, SourceObjectError> {
        let hex = self.extract_hex(source_digest)?;
        Ok(self.cas_root.join(hex))
    }

    /// Atomically publishes a raw in-memory byte slice as a canonical source object.
    pub async fn publish_source_blob(
        &self,
        preparation_id: Uuid,
        scope: &ModuleInstallationScope,
        media_type: &str,
        expected_digest: &str,
        bytes: &[u8],
        manifest_digest: Option<&str>,
    ) -> Result<SourceReceipt, SourceObjectError> {
        if !valid_digest(expected_digest) {
            return Err(SourceObjectError::InvalidDigest(expected_digest.to_string()));
        }
        if let Some(m_digest) = manifest_digest {
            if !valid_digest(m_digest) {
                return Err(SourceObjectError::InvalidDigest(m_digest.to_string()));
            }
        }

        let computed_digest = sha256_digest(bytes);
        if computed_digest != expected_digest {
            return Err(SourceObjectError::DigestMismatch {
                expected: expected_digest.to_string(),
                actual: computed_digest,
            });
        }

        // Check if an existing receipt already exists for this preparation + digest (idempotency)
        if let Some(existing) = self
            .find_receipt_by_preparation(preparation_id, expected_digest, Some(scope))
            .await?
        {
            return Ok(existing);
        }

        let destination = self.destination_path(expected_digest)?;

        // If blob does not already exist on disk, commit atomically via hard link or rename
        if !destination.exists() {
            let seq = SOURCE_UPLOAD_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temp_filename = format!(".upload-{}-{}-{}.tmp", std::process::id(), preparation_id, seq);
            let temp_path = self.cas_root.join(temp_filename);

            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .map_err(|e| SourceObjectError::Io(e.to_string()))?;

            if let Err(e) = file.write_all(bytes) {
                let _ = fs::remove_file(&temp_path);
                return Err(SourceObjectError::Io(e.to_string()));
            }

            if let Err(e) = file.sync_all() {
                let _ = fs::remove_file(&temp_path);
                return Err(SourceObjectError::Io(e.to_string()));
            }
            drop(file);

            match fs::hard_link(&temp_path, &destination) {
                Ok(()) => {
                    let _ = fs::remove_file(&temp_path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let _ = fs::remove_file(&temp_path);
                }
                Err(_) => {
                    // Fallback to rename if hard-linking fails across filesystem boundaries
                    if let Err(rename_err) = fs::rename(&temp_path, &destination) {
                        let _ = fs::remove_file(&temp_path);
                        if !destination.exists() {
                            return Err(SourceObjectError::Io(rename_err.to_string()));
                        }
                    }
                }
            }
        }

        self.insert_receipt(
            preparation_id,
            scope,
            media_type,
            expected_digest,
            bytes.len() as u64,
            manifest_digest,
        )
        .await
    }

    /// Publishes a source file from an existing path into the source CAS.
    pub async fn publish_source_file(
        &self,
        preparation_id: Uuid,
        scope: &ModuleInstallationScope,
        media_type: &str,
        expected_digest: &str,
        file_path: &Path,
        manifest_digest: Option<&str>,
    ) -> Result<SourceReceipt, SourceObjectError> {
        let bytes = fs::read(file_path).map_err(|e| SourceObjectError::Io(e.to_string()))?;
        self.publish_source_blob(
            preparation_id,
            scope,
            media_type,
            expected_digest,
            &bytes,
            manifest_digest,
        )
        .await
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
        let backend = self.db.get_database_backend();
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
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
            placeholder(backend, 4),
            placeholder(backend, 5),
            placeholder(backend, 6),
            placeholder(backend, 7),
            placeholder(backend, 8),
            placeholder(backend, 9),
        );

        let manifest_val = match manifest_digest {
            Some(d) => Value::from(d.to_string()),
            None => Value::from(None::<String>),
        };

        let values = vec![
            uuid_value(receipt_id, backend),
            uuid_value(preparation_id, backend),
            scope_kind.into(),
            scope_tenant_key.into(),
            source_digest.into(),
            media_type.into(),
            (byte_length as i64).into(),
            manifest_val,
            now.to_rfc3339().into(),
        ];

        self.db
            .execute_raw(Statement::from_sql_and_values(backend, insert_sql, values))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        // Re-read to guarantee we return the canonical receipt (whether inserted or pre-existing)
        let receipt = self
            .find_receipt_by_preparation(preparation_id, source_digest, Some(scope))
            .await?
            .ok_or_else(|| {
                SourceObjectError::Storage("Failed to read back inserted receipt".to_string())
            })?;

        Ok(receipt)
    }

    /// Reads back full content bytes of a verified source object.
    pub async fn get_source_blob(&self, source_digest: &str) -> Result<Vec<u8>, SourceObjectError> {
        let path = self.destination_path(source_digest)?;
        if !path.exists() {
            return Err(SourceObjectError::NotFound(source_digest.to_string()));
        }

        let bytes = fs::read(&path).map_err(|e| SourceObjectError::Io(e.to_string()))?;
        let computed = sha256_digest(&bytes);
        if computed != source_digest {
            return Err(SourceObjectError::DigestMismatch {
                expected: source_digest.to_string(),
                actual: computed,
            });
        }

        Ok(bytes)
    }

    /// Returns path to the verified source file on disk if it exists.
    pub fn get_source_path(&self, source_digest: &str) -> Result<PathBuf, SourceObjectError> {
        let path = self.destination_path(source_digest)?;
        if !path.exists() {
            return Err(SourceObjectError::NotFound(source_digest.to_string()));
        }
        Ok(path)
    }

    /// Finds a receipt by `(preparation_id, source_digest)` with RLS boundary check.
    pub async fn find_receipt_by_preparation(
        &self,
        preparation_id: Uuid,
        source_digest: &str,
        calling_scope: Option<&ModuleInstallationScope>,
    ) -> Result<Option<SourceReceipt>, SourceObjectError> {
        let backend = self.db.get_database_backend();
        let query_sql = format!(
            "SELECT source_receipt_id, preparation_id, scope_kind, scope_tenant_key, \
                    source_digest, media_type, byte_length, manifest_digest, created_at \
             FROM module_source_object_receipts \
             WHERE preparation_id = {} AND source_digest = {}",
            placeholder(backend, 1),
            placeholder(backend, 2)
        );

        let values = vec![
            uuid_value(preparation_id, backend),
            source_digest.into(),
        ];

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(backend, query_sql, values))
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

    /// Finds a receipt by `source_receipt_id` with RLS boundary check.
    pub async fn get_receipt(
        &self,
        source_receipt_id: Uuid,
        calling_scope: Option<&ModuleInstallationScope>,
    ) -> Result<Option<SourceReceipt>, SourceObjectError> {
        let backend = self.db.get_database_backend();
        let query_sql = format!(
            "SELECT source_receipt_id, preparation_id, scope_kind, scope_tenant_key, \
                    source_digest, media_type, byte_length, manifest_digest, created_at \
             FROM module_source_object_receipts \
             WHERE source_receipt_id = {}",
            placeholder(backend, 1)
        );

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                query_sql,
                vec![uuid_value(source_receipt_id, backend)],
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
        calling_scope: Option<&ModuleInstallationScope>,
    ) -> Result<(), SourceObjectError> {
        if let Some(scope) = calling_scope {
            match (scope, &receipt.scope) {
                (ModuleInstallationScope::Tenant { tenant_id }, ModuleInstallationScope::Tenant { tenant_id: owner_id }) => {
                    if tenant_id != owner_id {
                        return Err(SourceObjectError::UnauthorizedTenant {
                            receipt_id: receipt.source_receipt_id,
                            target_tenant: *tenant_id,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_receipt_row(
        &self,
        row: sea_orm::QueryResult,
        backend: DbBackend,
    ) -> Result<SourceReceipt, SourceObjectError> {
        let source_receipt_id: Uuid = match backend {
            DbBackend::Sqlite => {
                let s: String = row
                    .try_get("", "source_receipt_id")
                    .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
                Uuid::parse_str(&s).map_err(|e| SourceObjectError::Storage(e.to_string()))?
            }
            _ => row
                .try_get("", "source_receipt_id")
                .map_err(|e| SourceObjectError::Storage(e.to_string()))?,
        };

        let preparation_id: Uuid = match backend {
            DbBackend::Sqlite => {
                let s: String = row
                    .try_get("", "preparation_id")
                    .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
                Uuid::parse_str(&s).map_err(|e| SourceObjectError::Storage(e.to_string()))?
            }
            _ => row
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
            DbBackend::Sqlite => {
                let s: String = row
                    .try_get("", "created_at")
                    .map_err(|e| SourceObjectError::Storage(e.to_string()))?;
                DateTime::parse_from_rfc3339(&s)
                    .map_err(|e| SourceObjectError::Storage(e.to_string()))?
                    .with_timezone(&Utc)
            }
            _ => row
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

        let backend = self.db.get_database_backend();
        let hold_id = Uuid::new_v4();
        let now = Utc::now();

        let insert_sql = format!(
            "INSERT INTO module_source_object_retention_holds (\
                hold_id, source_digest, held_by, reason, expires_at, created_at\
             ) VALUES ({}, {}, {}, {}, {}, {})",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
            placeholder(backend, 4),
            placeholder(backend, 5),
            placeholder(backend, 6),
        );

        let exp_val = match expires_at {
            Some(exp) => Value::from(exp.to_rfc3339()),
            None => Value::from(None::<String>),
        };

        let values = vec![
            uuid_value(hold_id, backend),
            source_digest.into(),
            held_by.into(),
            reason.into(),
            exp_val,
            now.to_rfc3339().into(),
        ];

        self.db
            .execute_raw(Statement::from_sql_and_values(backend, insert_sql, values))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        Ok(hold_id)
    }

    /// Releases an existing retention hold by ID.
    pub async fn release_retention_hold(&self, hold_id: Uuid) -> Result<(), SourceObjectError> {
        let backend = self.db.get_database_backend();
        let delete_sql = format!(
            "DELETE FROM module_source_object_retention_holds WHERE hold_id = {}",
            placeholder(backend, 1)
        );

        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                delete_sql,
                vec![uuid_value(hold_id, backend)],
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Checks if a source object is currently protected by an active retention hold.
    pub async fn is_held(&self, source_digest: &str) -> Result<bool, SourceObjectError> {
        let backend = self.db.get_database_backend();
        let query_sql = format!(
            "SELECT 1 FROM module_source_object_retention_holds \
             WHERE source_digest = {} \
               AND (expires_at IS NULL OR expires_at > {}) \
             LIMIT 1",
            placeholder(backend, 1),
            placeholder(backend, 2),
        );

        let now = Utc::now().to_rfc3339();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                query_sql,
                vec![source_digest.into(), now.into()],
            ))
            .await
            .map_err(|e| SourceObjectError::Storage(e.to_string()))?;

        Ok(row.is_some())
    }
}
