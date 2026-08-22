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
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::data::{now_expression, placeholder};
use rustok_api::{
    PortError,
    manifest_hash::{canonical_manifest_snapshot_json, hash_manifest_snapshot},
};
use rustok_outbox::idempotency::{self, Admission};

/// Stable identity of the single active platform composition projection.
pub const ACTIVE_MODULE_COMPOSITION_ID: &str = "active";

const MODULE_COMPOSITION_OWNER_SLUG: &str = "modules.composition";
const REPLACE_AND_ENQUEUE_BUILD_OPERATION: &str = "replace_and_enqueue_build";

/// Immutable durable view of the platform's active module composition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleCompositionSnapshot {
    pub revision: i64,
    pub manifest_hash: String,
    pub manifest: Value,
}

/// Typed caller identity and concurrency context for one composition mutation.
/// The owner admits it before the host adapts the active manifest, which keeps
/// completed retries replayable after later composition changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleCompositionOperation {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
    pub expected_revision: i64,
}

/// Owner-provided immutable manifest replacement for an admitted operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleCompositionUpdate {
    pub operation: ModuleCompositionOperation,
    pub manifest: Value,
}

/// Terminal composition mutation result retained by the shared owner-operation
/// ledger. Replaying the same idempotency key returns this exact snapshot and
/// build output without evaluating a new CAS or queueing another build.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleCompositionBuildReceipt<T> {
    pub snapshot: ModuleCompositionSnapshot,
    pub output: T,
}

/// Owner result for an admitted composition mutation. `replayed` is runtime
/// delivery evidence and is intentionally not persisted in the terminal
/// receipt, so host notification adapters can repair at-least-once delivery.
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleCompositionBuildEnqueueResult<T> {
    pub snapshot: ModuleCompositionSnapshot,
    pub output: T,
    pub replayed: bool,
}

/// Opaque lease for a composition mutation admitted by the shared receipt
/// ledger. It can only be completed or failed by this owner.
pub struct ModuleCompositionBuildLease(idempotency::Lease);

/// Admission outcome before the host reads or adapts a static manifest.
pub enum ModuleCompositionBuildAdmission<T> {
    Run(ModuleCompositionBuildLease),
    Replay(ModuleCompositionBuildEnqueueResult<T>),
}

#[derive(Serialize)]
struct ModuleCompositionOperationReceiptRequest<'a, R> {
    actor_id: Uuid,
    expected_revision: i64,
    request: &'a R,
}

/// Host adapter that enqueues a build using the composition owner's open
/// transaction. The owner controls the transaction boundary; the host only
/// adapts its build persistence contract.
#[async_trait]
pub trait ModuleCompositionBuildEnqueuer: Send + Sync {
    type Output: Send + Serialize + DeserializeOwned;

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

    /// Admits one static composition mutation before the host reads or adapts
    /// the active manifest. Completed receipts return their immutable output
    /// without invoking the host adapter again.
    pub async fn admit_build_operation<T, R>(
        &self,
        operation: &ModuleCompositionOperation,
        operation_request: &R,
    ) -> Result<ModuleCompositionBuildAdmission<T>, ModuleCompositionError>
    where
        T: Send + Serialize + DeserializeOwned,
        R: Serialize,
    {
        validate_operation(operation)?;
        let receipt_request = ModuleCompositionOperationReceiptRequest {
            actor_id: operation.actor_id,
            expected_revision: operation.expected_revision,
            request: operation_request,
        };
        let admission = idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(operation.tenant_id),
            MODULE_COMPOSITION_OWNER_SLUG,
            &operation.idempotency_key.to_string(),
            REPLACE_AND_ENQUEUE_BUILD_OPERATION,
            &receipt_request,
        )
        .await
        .map_err(ModuleCompositionError::OperationReceipt)?;

        match admission {
            Admission::Replay(value) => {
                let receipt = serde_json::from_value::<ModuleCompositionBuildReceipt<T>>(value)
                    .map_err(|error| {
                        ModuleCompositionError::OperationReceiptCorrupt(error.to_string())
                    })?;
                Ok(ModuleCompositionBuildAdmission::Replay(
                    ModuleCompositionBuildEnqueueResult {
                        snapshot: receipt.snapshot,
                        output: receipt.output,
                        replayed: true,
                    },
                ))
            }
            Admission::ReplayError(error) => Err(ModuleCompositionError::OperationReceipt(error)),
            Admission::Run(lease) => Ok(ModuleCompositionBuildAdmission::Run(
                ModuleCompositionBuildLease(lease),
            )),
        }
    }

    /// Persists a terminal pre-transaction failure for an admitted operation.
    pub async fn fail_build_operation(
        &self,
        lease: ModuleCompositionBuildLease,
        error: &PortError,
    ) -> Result<(), ModuleCompositionError> {
        idempotency::fail(&self.db, lease.0, error)
            .await
            .map_err(ModuleCompositionError::OperationReceipt)
    }

    /// Replaces the immutable composition snapshot and requests a build in one
    /// transaction after the owner has admitted its operation. A failed enqueue
    /// rolls the CAS update and receipt completion back before this owner stores
    /// the corresponding terminal error.
    pub async fn replace_active_snapshot_and_enqueue<E>(
        &self,
        update: ModuleCompositionUpdate,
        enqueuer: &E,
        lease: ModuleCompositionBuildLease,
    ) -> Result<ModuleCompositionBuildEnqueueResult<E::Output>, ModuleCompositionError>
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
            let receipt = ModuleCompositionBuildReceipt { snapshot, output };
            idempotency::complete(&transaction, lease.0, &receipt)
                .await
                .map_err(ModuleCompositionError::OperationReceipt)?;
            Ok(receipt)
        }
        .await;
        match result {
            Ok(result) => {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ModuleCompositionError::Store(error.to_string()))?;
                Ok(ModuleCompositionBuildEnqueueResult {
                    snapshot: result.snapshot,
                    output: result.output,
                    replayed: false,
                })
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                let terminal_error = operation_failure(&error);
                self.fail_build_operation(lease, &terminal_error).await?;
                Err(error)
            }
        }
    }

    async fn replace_active_snapshot_on<C: ConnectionTrait>(
        connection: &C,
        update: ModuleCompositionUpdate,
    ) -> Result<ModuleCompositionSnapshot, ModuleCompositionError> {
        let current = Self::active_snapshot_on(connection).await?;
        if update.operation.expected_revision != current.revision {
            return Err(ModuleCompositionError::RevisionConflict {
                expected: update.operation.expected_revision,
                current: current.revision,
            });
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
                    update.operation.actor_id.to_string().into(),
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
                        id, revision, manifest_json, manifest_hash, updated_by, created_at, updated_at\
                     ) VALUES ({}, 1, {}, {}, {}, {}, {}) ON CONFLICT DO NOTHING",
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
}

#[derive(Debug, Error)]
pub enum ModuleCompositionError {
    #[error("active composition revision is invalid")]
    InvalidRevision,
    #[error("composition operation requires a positive expected revision")]
    InvalidExpectedRevision,
    #[error("composition operation requires a non-nil {field}")]
    InvalidOperationIdentity { field: &'static str },
    #[error("active composition revision overflowed")]
    RevisionOverflow,
    #[error("active composition revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: i64, current: i64 },
    #[error("bootstrap actor identity is required")]
    InvalidBootstrapActor,
    #[error("active module composition is unavailable")]
    MissingActiveComposition,
    #[error("module composition owner-operation receipt error: {0}")]
    OperationReceipt(PortError),
    #[error("module composition owner-operation receipt is corrupt: {0}")]
    OperationReceiptCorrupt(String),
    #[error("module composition store error: {0}")]
    Store(String),
    #[error("module composition build enqueue failed: {0}")]
    BuildEnqueue(String),
    #[error("failed to serialize module composition: {0}")]
    Serialize(String),
    #[error("failed to deserialize module composition: {0}")]
    Deserialize(String),
}

fn validate_operation(
    operation: &ModuleCompositionOperation,
) -> Result<(), ModuleCompositionError> {
    if operation.tenant_id.is_nil() {
        return Err(ModuleCompositionError::InvalidOperationIdentity { field: "tenant ID" });
    }
    if operation.actor_id.is_nil() {
        return Err(ModuleCompositionError::InvalidOperationIdentity { field: "actor ID" });
    }
    if operation.idempotency_key.is_nil() {
        return Err(ModuleCompositionError::InvalidOperationIdentity {
            field: "idempotency key",
        });
    }
    if operation.expected_revision < 1 {
        return Err(ModuleCompositionError::InvalidExpectedRevision);
    }
    Ok(())
}

fn operation_failure(error: &ModuleCompositionError) -> PortError {
    match error {
        ModuleCompositionError::RevisionConflict { .. } => {
            PortError::conflict("modules.composition_revision_conflict", error.to_string())
        }
        ModuleCompositionError::BuildEnqueue(_) | ModuleCompositionError::Store(_) => {
            PortError::unavailable("modules.composition_unavailable", error.to_string())
        }
        ModuleCompositionError::InvalidRevision
        | ModuleCompositionError::InvalidExpectedRevision
        | ModuleCompositionError::InvalidOperationIdentity { .. }
        | ModuleCompositionError::InvalidBootstrapActor
        | ModuleCompositionError::Serialize(_)
        | ModuleCompositionError::Deserialize(_) => {
            PortError::validation("modules.composition_invalid_request", error.to_string())
        }
        ModuleCompositionError::RevisionOverflow
        | ModuleCompositionError::MissingActiveComposition
        | ModuleCompositionError::OperationReceipt(_)
        | ModuleCompositionError::OperationReceiptCorrupt(_) => {
            PortError::invariant_violation("modules.composition_invariant", error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rustok_outbox::SysEventsMigration;

    use super::*;

    struct RecordingEnqueuer {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl ModuleCompositionBuildEnqueuer for RecordingEnqueuer {
        type Output = i64;

        async fn enqueue(
            &self,
            _transaction: &DatabaseTransaction,
            snapshot: &ModuleCompositionSnapshot,
        ) -> Result<Self::Output, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
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

    async fn setup_database() -> DatabaseConnection {
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
                    updated_by TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL\
                 )"
                .to_string(),
            ))
            .await
            .expect("composition table");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("owner operation receipt table");
        database
    }

    fn operation(expected_revision: i64, idempotency_key: Uuid) -> ModuleCompositionOperation {
        ModuleCompositionOperation {
            tenant_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            idempotency_key,
            expected_revision,
        }
    }

    fn update(operation: ModuleCompositionOperation, manifest: Value) -> ModuleCompositionUpdate {
        ModuleCompositionUpdate {
            operation,
            manifest,
        }
    }

    async fn admit_recording_operation(
        service: &SeaOrmModuleCompositionService,
        operation: &ModuleCompositionOperation,
        request: &Value,
    ) -> ModuleCompositionBuildLease {
        match service
            .admit_build_operation::<i64, _>(operation, request)
            .await
            .expect("admit composition operation")
        {
            ModuleCompositionBuildAdmission::Run(lease) => lease,
            ModuleCompositionBuildAdmission::Replay(_) => {
                panic!("first operation admission must run")
            }
        }
    }

    #[tokio::test]
    async fn bootstrap_canonicalizes_and_reuses_the_active_snapshot() {
        let database = setup_database().await;
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
    async fn admission_requires_non_nil_identity_and_positive_revision() {
        let service = SeaOrmModuleCompositionService::new(setup_database().await);
        let request = serde_json::json!({ "change": "install" });

        for (operation, expected_field) in [
            (
                ModuleCompositionOperation {
                    tenant_id: Uuid::nil(),
                    actor_id: Uuid::new_v4(),
                    idempotency_key: Uuid::new_v4(),
                    expected_revision: 1,
                },
                Some("tenant ID"),
            ),
            (
                ModuleCompositionOperation {
                    tenant_id: Uuid::new_v4(),
                    actor_id: Uuid::nil(),
                    idempotency_key: Uuid::new_v4(),
                    expected_revision: 1,
                },
                Some("actor ID"),
            ),
            (
                ModuleCompositionOperation {
                    tenant_id: Uuid::new_v4(),
                    actor_id: Uuid::new_v4(),
                    idempotency_key: Uuid::nil(),
                    expected_revision: 1,
                },
                Some("idempotency key"),
            ),
            (
                ModuleCompositionOperation {
                    tenant_id: Uuid::new_v4(),
                    actor_id: Uuid::new_v4(),
                    idempotency_key: Uuid::new_v4(),
                    expected_revision: 0,
                },
                None,
            ),
        ] {
            let error = match service
                .admit_build_operation::<i64, _>(&operation, &request)
                .await
            {
                Ok(_) => panic!("invalid operation context must fail before receipt admission"),
                Err(error) => error,
            };
            match expected_field {
                Some(field) => assert!(matches!(
                    error,
                    ModuleCompositionError::InvalidOperationIdentity { field: actual } if actual == field
                )),
                None => assert!(matches!(
                    error,
                    ModuleCompositionError::InvalidExpectedRevision
                )),
            }
        }
    }

    #[tokio::test]
    async fn snapshot_replacement_requires_revision_and_replays_the_terminal_build() {
        let database = setup_database().await;
        let service = SeaOrmModuleCompositionService::new(database);
        service
            .ensure_active_snapshot(&serde_json::json!({ "initial": true }), "bootstrap")
            .await
            .expect("bootstrap snapshot");

        let enqueuer = RecordingEnqueuer {
            calls: AtomicUsize::new(0),
        };
        let request = serde_json::json!({ "change": "install", "reason": "test" });
        let initial_operation = operation(1, Uuid::new_v4());
        let lease = admit_recording_operation(&service, &initial_operation, &request).await;
        let updated = service
            .replace_active_snapshot_and_enqueue(
                update(
                    initial_operation.clone(),
                    serde_json::json!({ "z": 1, "a": 2 }),
                ),
                &enqueuer,
                lease,
            )
            .await
            .expect("replace snapshot");
        assert_eq!(updated.snapshot.revision, 2);
        assert_eq!(
            updated.snapshot.manifest,
            serde_json::json!({ "a": 2, "z": 1 })
        );
        assert_eq!(updated.output, 2);

        let replay = service
            .admit_build_operation::<i64, _>(&initial_operation, &request)
            .await
            .expect("terminal receipt must replay");
        let ModuleCompositionBuildAdmission::Replay(replay) = replay else {
            panic!("completed operation must replay");
        };
        assert_eq!(replay.snapshot, updated.snapshot);
        assert_eq!(replay.output, updated.output);
        assert!(!updated.replayed);
        assert!(replay.replayed);
        assert_eq!(enqueuer.calls.load(Ordering::SeqCst), 1);

        assert!(matches!(
            {
                let stale_operation = operation(1, Uuid::new_v4());
                let stale_lease = admit_recording_operation(
                    &service,
                    &stale_operation,
                    &serde_json::json!({ "change": "install", "reason": "stale" }),
                )
                .await;
                service
                    .replace_active_snapshot_and_enqueue(
                        update(stale_operation, serde_json::json!({ "another": true })),
                        &enqueuer,
                        stale_lease,
                    )
                    .await
            },
            Err(ModuleCompositionError::RevisionConflict {
                expected: 1,
                current: 2,
            })
        ));
    }

    #[tokio::test]
    async fn build_enqueue_and_snapshot_cas_commit_or_rollback_together() {
        let database = setup_database().await;
        let service = SeaOrmModuleCompositionService::new(database);
        service
            .ensure_active_snapshot(&serde_json::json!({ "initial": true }), "bootstrap")
            .await
            .expect("bootstrap snapshot");

        let committed_operation = operation(1, Uuid::new_v4());
        let committed_lease = admit_recording_operation(
            &service,
            &committed_operation,
            &serde_json::json!({ "change": "install", "reason": "commit" }),
        )
        .await;
        let receipt = service
            .replace_active_snapshot_and_enqueue(
                update(
                    committed_operation,
                    serde_json::json!({ "committed": true }),
                ),
                &RecordingEnqueuer {
                    calls: AtomicUsize::new(0),
                },
                committed_lease,
            )
            .await
            .expect("atomic composition update");
        assert_eq!(receipt.snapshot.revision, 2);
        assert_eq!(receipt.output, 2);
        let failing_operation = operation(2, Uuid::new_v4());
        let failing_lease = admit_recording_operation(
            &service,
            &failing_operation,
            &serde_json::json!({ "change": "install", "reason": "rollback" }),
        )
        .await;
        assert!(matches!(
            service
                .replace_active_snapshot_and_enqueue(
                    update(
                        failing_operation,
                        serde_json::json!({ "rolled_back": true })
                    ),
                    &FailingEnqueuer,
                    failing_lease,
                )
                .await,
            Err(ModuleCompositionError::BuildEnqueue(_))
        ));
        assert_eq!(
            service.active_snapshot().await.expect("active snapshot"),
            receipt.snapshot
        );
    }
}
