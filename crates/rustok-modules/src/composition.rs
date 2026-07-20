//! Durable owner operations for the platform module-composition projection.
//!
//! The host may adapt a typed manifest or release model at its boundary, but
//! it must not write `platform_state` directly. This owner service keeps the
//! active release pointer consistent with the durable composition snapshot.

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait,
    Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::data::{now_expression, placeholder};
use rustok_api::manifest_hash::{canonical_manifest_snapshot_json, hash_manifest_snapshot};

/// Stable identity of the single active platform composition projection.
pub const ACTIVE_MODULE_COMPOSITION_ID: &str = "active";

/// Immutable durable view of the platform's active module composition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleCompositionSnapshot {
    pub revision: i64,
    pub manifest_hash: String,
    pub manifest: Value,
}

/// Revision-guarded replacement of the immutable active composition snapshot.
/// The owner canonicalizes the manifest and derives the persisted digest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleCompositionUpdate {
    pub expected_revision: Option<i64>,
    pub manifest: Value,
    pub updated_by: Option<String>,
}

/// Host adapter that enqueues a build using the composition owner's open
/// transaction. The owner controls the transaction boundary; the host only
/// adapts its build persistence contract.
#[async_trait]
pub trait ModuleCompositionBuildEnqueuer: Send + Sync {
    type Output: Send;

    async fn enqueue(
        &self,
        transaction: &DatabaseTransaction,
        snapshot: &ModuleCompositionSnapshot,
    ) -> Result<Self::Output, String>;
}

/// Owner-side database adapter for module-composition state.
#[derive(Clone)]
pub struct SeaOrmModuleCompositionService {
    db: DatabaseConnection,
}

impl SeaOrmModuleCompositionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Loads the already admitted active composition snapshot.
    pub async fn active_snapshot(
        &self,
    ) -> Result<ModuleCompositionSnapshot, ModuleCompositionError> {
        Self::active_snapshot_on(&self.db).await
    }

    /// Ensures the active composition projection exists from a host-loaded
    /// bootstrap snapshot. The owner canonicalizes and hashes the snapshot;
    /// the host never supplies a precomputed identity.
    pub async fn ensure_active_snapshot(
        &self,
        bootstrap_manifest: &Value,
        bootstrap_actor: &str,
    ) -> Result<ModuleCompositionSnapshot, ModuleCompositionError> {
        Self::ensure_active_snapshot_on(&self.db, bootstrap_manifest, bootstrap_actor).await
    }

    pub async fn replace_active_snapshot(
        &self,
        update: ModuleCompositionUpdate,
    ) -> Result<ModuleCompositionSnapshot, ModuleCompositionError> {
        Self::replace_active_snapshot_on(&self.db, update).await
    }

    /// Replaces the immutable composition snapshot and requests a build in one
    /// transaction. A failed enqueue rolls the CAS update back; a caller must
    /// publish non-transactional notifications only after this method returns.
    pub async fn replace_active_snapshot_and_enqueue<E>(
        &self,
        update: ModuleCompositionUpdate,
        enqueuer: &E,
    ) -> Result<(ModuleCompositionSnapshot, E::Output), ModuleCompositionError>
    where
        E: ModuleCompositionBuildEnqueuer,
    {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleCompositionError::Store(error.to_string()))?;
        let result = async {
            let snapshot = Self::replace_active_snapshot_on(&transaction, update).await?;
            let output = enqueuer
                .enqueue(&transaction, &snapshot)
                .await
                .map_err(ModuleCompositionError::BuildEnqueue)?;
            Ok((snapshot, output))
        }
        .await;
        match result {
            Ok(result) => {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ModuleCompositionError::Store(error.to_string()))?;
                Ok(result)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn replace_active_snapshot_on<C: ConnectionTrait>(
        connection: &C,
        update: ModuleCompositionUpdate,
    ) -> Result<ModuleCompositionSnapshot, ModuleCompositionError> {
        let current = Self::active_snapshot_on(connection).await?;
        if let Some(expected) = update.expected_revision {
            if expected != current.revision {
                return Err(ModuleCompositionError::RevisionConflict {
                    expected,
                    current: current.revision,
                });
            }
        }
        let next_revision = current
            .revision
            .checked_add(1)
            .ok_or(ModuleCompositionError::RevisionOverflow)?;
        let canonical_manifest = canonical_manifest_snapshot_json(&update.manifest)
            .map_err(|error| ModuleCompositionError::Serialize(error.to_string()))?;
        let manifest_hash = hash_manifest_snapshot(&canonical_manifest);
        let backend = connection.get_database_backend();
        let placeholders = (1..=6)
            .map(|index| placeholder(backend, index))
            .collect::<Vec<_>>();
        let result = connection
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE platform_state SET revision = {}, manifest_json = {}, manifest_hash = {}, \
                     updated_by = {}, updated_at = {} WHERE id = {} AND revision = {}",
                    placeholders[0],
                    placeholders[1],
                    placeholders[2],
                    placeholders[3],
                    now_expression(backend),
                    placeholders[4],
                    placeholders[5],
                ),
                vec![
                    next_revision.into(),
                    SqlValue::Json(Some(Box::new(canonical_manifest.clone()))),
                    manifest_hash.clone().into(),
                    optional_string_value(update.updated_by),
                    ACTIVE_MODULE_COMPOSITION_ID.into(),
                    current.revision.into(),
                ],
            ))
            .await
            .map_err(|error| ModuleCompositionError::Store(error.to_string()))?;
        if result.rows_affected() != 1 {
            let refreshed = Self::active_snapshot_on(connection).await?;
            return Err(ModuleCompositionError::RevisionConflict {
                expected: current.revision,
                current: refreshed.revision,
            });
        }
        Ok(ModuleCompositionSnapshot {
            revision: next_revision,
            manifest_hash,
            manifest: canonical_manifest,
        })
    }

    pub async fn active_snapshot_on<C: ConnectionTrait>(
        connection: &C,
    ) -> Result<ModuleCompositionSnapshot, ModuleCompositionError> {
        let backend = connection.get_database_backend();
        let row = connection
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT revision, manifest_hash, CAST(manifest_json AS TEXT) AS manifest_json \
                     FROM platform_state WHERE id = {}",
                    placeholder(backend, 1),
                ),
                vec![ACTIVE_MODULE_COMPOSITION_ID.into()],
            ))
            .await
            .map_err(|error| ModuleCompositionError::Store(error.to_string()))?
            .ok_or(ModuleCompositionError::MissingActiveComposition)?;
        let revision = row
            .try_get("", "revision")
            .map_err(|error| ModuleCompositionError::Store(error.to_string()))?;
        if revision < 1 {
            return Err(ModuleCompositionError::InvalidRevision);
        }
        let manifest_json: String = row
            .try_get("", "manifest_json")
            .map_err(|error| ModuleCompositionError::Store(error.to_string()))?;
        let manifest = serde_json::from_str(&manifest_json)
            .map_err(|error| ModuleCompositionError::Deserialize(error.to_string()))?;
        Ok(ModuleCompositionSnapshot {
            revision,
            manifest_hash: row
                .try_get("", "manifest_hash")
                .map_err(|error| ModuleCompositionError::Store(error.to_string()))?,
            manifest,
        })
    }

    pub async fn ensure_active_snapshot_on<C: ConnectionTrait>(
        connection: &C,
        bootstrap_manifest: &Value,
        bootstrap_actor: &str,
    ) -> Result<ModuleCompositionSnapshot, ModuleCompositionError> {
        if bootstrap_actor.trim().is_empty() {
            return Err(ModuleCompositionError::InvalidBootstrapActor);
        }
        let canonical_manifest = canonical_manifest_snapshot_json(bootstrap_manifest)
            .map_err(|error| ModuleCompositionError::Serialize(error.to_string()))?;
        let backend = connection.get_database_backend();
        let placeholders = (1..=4)
            .map(|index| placeholder(backend, index))
            .collect::<Vec<_>>();
        connection
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO platform_state (\
                        id, revision, manifest_json, manifest_hash, active_release_id, updated_by, created_at, updated_at\
                     ) VALUES ({}, 1, {}, {}, NULL, {}, {}, {}) ON CONFLICT DO NOTHING",
                    placeholders[0],
                    placeholders[1],
                    placeholders[2],
                    placeholders[3],
                    now_expression(backend),
                    now_expression(backend),
                ),
                vec![
                    ACTIVE_MODULE_COMPOSITION_ID.into(),
                    SqlValue::Json(Some(Box::new(canonical_manifest.clone()))),
                    hash_manifest_snapshot(&canonical_manifest).into(),
                    bootstrap_actor.to_owned().into(),
                ],
            ))
            .await
            .map_err(|error| ModuleCompositionError::Store(error.to_string()))?;
        Self::active_snapshot_on(connection).await
    }

    /// Records the release that is active for the durable platform composition.
    ///
    /// A release activation never bootstraps or rewrites composition state: the
    /// active snapshot must already exist and a missing row fails closed.
    pub async fn set_active_release(&self, release_id: &str) -> Result<(), ModuleCompositionError> {
        if release_id.trim().is_empty() {
            return Err(ModuleCompositionError::InvalidReleaseIdentity);
        }
        let backend = self.db.get_database_backend();
        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE platform_state SET active_release_id = {}, updated_at = {} \
                     WHERE id = {}",
                    placeholder(backend, 1),
                    now_expression(backend),
                    placeholder(backend, 2),
                ),
                vec![
                    release_id.to_owned().into(),
                    ACTIVE_MODULE_COMPOSITION_ID.into(),
                ],
            ))
            .await
            .map_err(|error| ModuleCompositionError::Store(error.to_string()))?;
        if result.rows_affected() != 1 {
            return Err(ModuleCompositionError::MissingActiveComposition);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ModuleCompositionError {
    #[error("active composition revision is invalid")]
    InvalidRevision,
    #[error("active composition revision overflowed")]
    RevisionOverflow,
    #[error("active composition revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: i64, current: i64 },
    #[error("bootstrap actor identity is required")]
    InvalidBootstrapActor,
    #[error("active release identity is required")]
    InvalidReleaseIdentity,
    #[error("active module composition is unavailable")]
    MissingActiveComposition,
    #[error("module composition store error: {0}")]
    Store(String),
    #[error("module composition build enqueue failed: {0}")]
    BuildEnqueue(String),
    #[error("failed to serialize module composition: {0}")]
    Serialize(String),
    #[error("failed to deserialize module composition: {0}")]
    Deserialize(String),
}

fn optional_string_value(value: Option<String>) -> SqlValue {
    SqlValue::String(value.map(Box::new))
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    use super::*;

    struct RecordingEnqueuer;

    #[async_trait]
    impl ModuleCompositionBuildEnqueuer for RecordingEnqueuer {
        type Output = i64;

        async fn enqueue(
            &self,
            _transaction: &DatabaseTransaction,
            snapshot: &ModuleCompositionSnapshot,
        ) -> Result<Self::Output, String> {
            Ok(snapshot.revision)
        }
    }

    struct FailingEnqueuer;

    #[async_trait]
    impl ModuleCompositionBuildEnqueuer for FailingEnqueuer {
        type Output = ();

        async fn enqueue(
            &self,
            _transaction: &DatabaseTransaction,
            _snapshot: &ModuleCompositionSnapshot,
        ) -> Result<Self::Output, String> {
            Err("build queue unavailable".to_string())
        }
    }

    #[tokio::test]
    async fn active_release_update_requires_existing_composition() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE platform_state (\
                    id TEXT PRIMARY KEY,\
                    revision INTEGER NOT NULL,\
                    manifest_json TEXT NOT NULL,\
                    manifest_hash TEXT NOT NULL,\
                    active_release_id TEXT NULL,\
                    updated_by TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL\
                 )"
                .to_string(),
            ))
            .await
            .expect("composition table");
        let service = SeaOrmModuleCompositionService::new(database.clone());

        assert!(matches!(
            service.set_active_release("release-1").await,
            Err(ModuleCompositionError::MissingActiveComposition)
        ));

        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO platform_state (\
                    id, revision, manifest_json, manifest_hash, active_release_id, updated_by, created_at, updated_at\
                 ) VALUES (\
                    'active', 1, '{}', 'bootstrap', NULL, 'bootstrap', datetime('now'), datetime('now')\
                 )"
                    .to_string(),
            ))
            .await
            .expect("active composition");
        service
            .set_active_release("release-1")
            .await
            .expect("active release");
        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT active_release_id FROM platform_state WHERE id = 'active'".to_string(),
            ))
            .await
            .expect("query")
            .expect("row");
        assert_eq!(
            row.try_get::<String>("", "active_release_id")
                .expect("release id"),
            "release-1"
        );
    }

    #[tokio::test]
    async fn bootstrap_canonicalizes_and_reuses_the_active_snapshot() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE platform_state (\
                    id TEXT PRIMARY KEY,\
                    revision INTEGER NOT NULL,\
                    manifest_json TEXT NOT NULL,\
                    manifest_hash TEXT NOT NULL,\
                    active_release_id TEXT NULL,\
                    updated_by TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL\
                 )"
                .to_string(),
            ))
            .await
            .expect("composition table");
        let service = SeaOrmModuleCompositionService::new(database);

        let snapshot = service
            .ensure_active_snapshot(&serde_json::json!({ "z": 1, "a": 2 }), "bootstrap")
            .await
            .expect("bootstrap snapshot");
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.manifest, serde_json::json!({ "a": 2, "z": 1 }));
        assert_eq!(
            service
                .ensure_active_snapshot(&serde_json::json!({ "changed": true }), "bootstrap")
                .await
                .expect("existing snapshot"),
            snapshot
        );
    }

    #[tokio::test]
    async fn snapshot_replacement_uses_revision_cas_and_canonical_digest() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE platform_state (\
                    id TEXT PRIMARY KEY,\
                    revision INTEGER NOT NULL,\
                    manifest_json TEXT NOT NULL,\
                    manifest_hash TEXT NOT NULL,\
                    active_release_id TEXT NULL,\
                    updated_by TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL\
                 )"
                .to_string(),
            ))
            .await
            .expect("composition table");
        let service = SeaOrmModuleCompositionService::new(database);
        service
            .ensure_active_snapshot(&serde_json::json!({ "initial": true }), "bootstrap")
            .await
            .expect("bootstrap snapshot");

        let updated = service
            .replace_active_snapshot(ModuleCompositionUpdate {
                expected_revision: Some(1),
                manifest: serde_json::json!({ "z": 1, "a": 2 }),
                updated_by: Some("operator".to_string()),
            })
            .await
            .expect("replace snapshot");
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.manifest, serde_json::json!({ "a": 2, "z": 1 }));
        assert!(matches!(
            service
                .replace_active_snapshot(ModuleCompositionUpdate {
                    expected_revision: Some(1),
                    manifest: serde_json::json!({ "another": true }),
                    updated_by: None,
                })
                .await,
            Err(ModuleCompositionError::RevisionConflict {
                expected: 1,
                current: 2,
            })
        ));
    }

    #[tokio::test]
    async fn build_enqueue_and_snapshot_cas_commit_or_rollback_together() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE platform_state (\
                    id TEXT PRIMARY KEY,\
                    revision INTEGER NOT NULL,\
                    manifest_json TEXT NOT NULL,\
                    manifest_hash TEXT NOT NULL,\
                    active_release_id TEXT NULL,\
                    updated_by TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL\
                 )"
                .to_string(),
            ))
            .await
            .expect("composition table");
        let service = SeaOrmModuleCompositionService::new(database);
        service
            .ensure_active_snapshot(&serde_json::json!({ "initial": true }), "bootstrap")
            .await
            .expect("bootstrap snapshot");

        let (snapshot, build_revision) = service
            .replace_active_snapshot_and_enqueue(
                ModuleCompositionUpdate {
                    expected_revision: Some(1),
                    manifest: serde_json::json!({ "committed": true }),
                    updated_by: Some("operator".to_string()),
                },
                &RecordingEnqueuer,
            )
            .await
            .expect("atomic composition update");
        assert_eq!(snapshot.revision, 2);
        assert_eq!(build_revision, 2);
        assert!(matches!(
            service
                .replace_active_snapshot_and_enqueue(
                    ModuleCompositionUpdate {
                        expected_revision: Some(2),
                        manifest: serde_json::json!({ "rolled_back": true }),
                        updated_by: Some("operator".to_string()),
                    },
                    &FailingEnqueuer,
                )
                .await,
            Err(ModuleCompositionError::BuildEnqueue(_))
        ));
        assert_eq!(
            service.active_snapshot().await.expect("active snapshot"),
            snapshot
        );
    }
}
