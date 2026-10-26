//! Durable executor-pool identity registry.
//!
//! A pool generation is deployment evidence, not a caller assertion: smoke
//! receipts are bound to `(pool_id, pool_generation, fingerprint)` and a new
//! generation must never reuse earlier smoke. The generation therefore has
//! to survive process restarts and increase monotonically whenever the
//! fingerprint, placement, or placement attestation of a pool changes. This
//! registry is the durable owner of that counter. Hosts register the pools
//! they actually composed at startup; re-registering byte-identical facts
//! resumes the current generation so an ordinary restart does not
//! invalidate still-valid smoke receipts.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ConnectionTrait, DatabaseConnection, DeriveEntityModel,
    DerivePrimaryKey, DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::backend::SupportedBackend;
use crate::{ExecutorPoolIdentity, RuntimeFingerprint};
use rustok_sandbox::SandboxExecutorPlacement;

#[derive(Debug, Error)]
pub enum ExecutorPoolRegistryError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("Executor pool registration is invalid: {0}")]
    InvalidRegistration(String),
    #[error("Stored executor pool record is corrupt: {0}")]
    CorruptData(String),
    #[error("Executor pool generation overflow for pool `{0}`")]
    GenerationOverflow(String),
}

/// Host-supplied deployment facts for one composed executor pool.
///
/// The generation is deliberately absent: it is assigned by the durable
/// registry. Every field must describe the executor the host actually
/// registered — the production readiness probe cross-checks the claimed
/// placement against the sandbox registry before any smoke runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutorPoolRegistration {
    pub pool_id: String,
    pub fingerprint: RuntimeFingerprint,
    pub placement: SandboxExecutorPlacement,
    pub placement_attestation: Option<String>,
}

impl ExecutorPoolRegistration {
    fn identity_with_generation(&self, pool_generation: u64) -> ExecutorPoolIdentity {
        ExecutorPoolIdentity {
            pool_id: self.pool_id.clone(),
            pool_generation,
            fingerprint: self.fingerprint.clone(),
            placement: self.placement,
            placement_attestation: self.placement_attestation.clone(),
        }
    }

    fn validate(&self) -> Result<(), ExecutorPoolRegistryError> {
        // Generation 1 stands in for "some positive generation" so the
        // full pool-identity contract (including the isolated-worker
        // attestation and image-digest requirements) is reused as-is.
        self.identity_with_generation(1)
            .validate()
            .map_err(|error| ExecutorPoolRegistryError::InvalidRegistration(error.to_string()))
    }
}

// ============================================================================
// SeaORM Entity: module_executor_pools
// ============================================================================

pub mod executor_pool_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "module_executor_pools")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub pool_id: String,
        pub placement: String,
        pub fingerprint: serde_json::Value,
        pub fingerprint_digest: String,
        pub placement_attestation: Option<String>,
        pub pool_generation: i64,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

fn placement_from_stored(value: &str) -> Result<SandboxExecutorPlacement, ExecutorPoolRegistryError> {
    match value {
        "in_process" => Ok(SandboxExecutorPlacement::InProcess),
        "isolated_worker" => Ok(SandboxExecutorPlacement::IsolatedWorker),
        other => Err(ExecutorPoolRegistryError::CorruptData(format!(
            "unknown stored executor placement `{other}`"
        ))),
    }
}

fn identity_from_model(
    model: executor_pool_entity::Model,
) -> Result<ExecutorPoolIdentity, ExecutorPoolRegistryError> {
    let fingerprint: RuntimeFingerprint = serde_json::from_value(model.fingerprint).map_err(
        |error| {
            ExecutorPoolRegistryError::CorruptData(format!(
                "stored runtime fingerprint is invalid: {error}"
            ))
        },
    )?;
    if fingerprint.compute_digest() != model.fingerprint_digest {
        return Err(ExecutorPoolRegistryError::CorruptData(format!(
            "stored fingerprint digest for pool `{}` does not match its components",
            model.pool_id
        )));
    }
    let pool_generation = u64::try_from(model.pool_generation).map_err(|_| {
        ExecutorPoolRegistryError::CorruptData(format!(
            "stored pool generation for pool `{}` is outside the supported range",
            model.pool_id
        ))
    })?;
    let identity = ExecutorPoolIdentity {
        pool_id: model.pool_id,
        pool_generation,
        fingerprint,
        placement: placement_from_stored(&model.placement)?,
        placement_attestation: model.placement_attestation,
    };
    identity.validate().map_err(|error| {
        ExecutorPoolRegistryError::CorruptData(format!(
            "stored executor pool identity is invalid: {error}"
        ))
    })?;
    Ok(identity)
}

/// SeaORM-backed durable registry of composed executor pools.
#[derive(Clone)]
pub struct SeaOrmExecutorPoolRegistry {
    db: DatabaseConnection,
}

impl SeaOrmExecutorPoolRegistry {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Registers one composed pool or resumes its existing registration.
    ///
    /// A first registration starts at generation 1. Re-registering the
    /// exact same fingerprint, placement, and attestation returns the
    /// current generation unchanged, so a process restart alone never
    /// invalidates smoke evidence. Any change to those facts advances the
    /// generation by one inside the same transaction, which makes earlier
    /// smoke receipts stale by construction.
    pub async fn register_or_resume(
        &self,
        registration: &ExecutorPoolRegistration,
    ) -> Result<ExecutorPoolIdentity, ExecutorPoolRegistryError> {
        registration.validate()?;
        let fingerprint_digest = registration.fingerprint.compute_digest();
        let fingerprint_json = serde_json::to_value(&registration.fingerprint).map_err(|error| {
            ExecutorPoolRegistryError::InvalidRegistration(format!(
                "runtime fingerprint cannot be encoded: {error}"
            ))
        })?;
        let placement = registration.placement.to_string();

        let transaction = self.db.begin().await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ExecutorPoolRegistryError::Database(error.into()))?;
        let existing = match backend {
            // PostgreSQL serializes concurrent registrations of the same
            // pool on the row lock; SQLite admits a single writer.
            SupportedBackend::Postgres => {
                executor_pool_entity::Entity::find_by_id(registration.pool_id.clone())
                    .lock_exclusive()
                    .one(&transaction)
                    .await?
            }
            SupportedBackend::Sqlite => {
                executor_pool_entity::Entity::find_by_id(registration.pool_id.clone())
                    .one(&transaction)
                    .await?
            }
        };

        let now = Utc::now();
        let identity = match existing {
            None => {
                let model = executor_pool_entity::ActiveModel {
                    pool_id: Set(registration.pool_id.clone()),
                    placement: Set(placement),
                    fingerprint: Set(fingerprint_json),
                    fingerprint_digest: Set(fingerprint_digest),
                    placement_attestation: Set(registration.placement_attestation.clone()),
                    pool_generation: Set(1),
                    created_at: Set(now),
                    updated_at: Set(now),
                };
                model.insert(&transaction).await?;
                registration.identity_with_generation(1)
            }
            Some(model) => {
                let current = identity_from_model(model.clone())?;
                if current.fingerprint == registration.fingerprint
                    && current.placement == registration.placement
                    && current.placement_attestation == registration.placement_attestation
                {
                    current
                } else {
                    let next_generation = current
                        .pool_generation
                        .checked_add(1)
                        .ok_or_else(|| {
                            ExecutorPoolRegistryError::GenerationOverflow(
                                registration.pool_id.clone(),
                            )
                        })?;
                    let next_generation_stored = i64::try_from(next_generation).map_err(|_| {
                        ExecutorPoolRegistryError::GenerationOverflow(registration.pool_id.clone())
                    })?;
                    let mut active: executor_pool_entity::ActiveModel = model.into();
                    active.placement = Set(placement);
                    active.fingerprint = Set(fingerprint_json);
                    active.fingerprint_digest = Set(fingerprint_digest);
                    active.placement_attestation =
                        Set(registration.placement_attestation.clone());
                    active.pool_generation = Set(next_generation_stored);
                    active.updated_at = Set(now);
                    active.update(&transaction).await?;
                    registration.identity_with_generation(next_generation)
                }
            }
        };
        transaction.commit().await?;
        Ok(identity)
    }

    /// Lists every registered pool in deterministic `pool_id` order.
    ///
    /// The governed transition host uses this as its required-pool set: a
    /// module may execute on any registered pool, so every one of them owes
    /// current smoke evidence before an automatic transition may proceed.
    pub async fn list_pools(
        &self,
    ) -> Result<Vec<ExecutorPoolIdentity>, ExecutorPoolRegistryError> {
        let models = executor_pool_entity::Entity::find()
            .order_by_asc(executor_pool_entity::Column::PoolId)
            .all(&self.db)
            .await?;
        models.into_iter().map(identity_from_model).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint() -> RuntimeFingerprint {
        RuntimeFingerprint {
            executor_binary_digest: format!("sha256:{}", "a".repeat(64)),
            engine_build_digest: format!("sha256:{}", "b".repeat(64)),
            engine_config_revision: "cfg-v1".to_string(),
            isolated_worker_image_digest: None,
            target_cpu_contract: "x86_64-v2".to_string(),
            runtime_abi: "rustok-module-abi-v1".to_string(),
        }
    }

    #[test]
    fn registration_validation_reuses_pool_identity_contract() {
        let registration = ExecutorPoolRegistration {
            pool_id: "node-1-wasm".to_string(),
            fingerprint: fingerprint(),
            placement: SandboxExecutorPlacement::InProcess,
            placement_attestation: None,
        };
        registration.validate().expect("valid in-process pool");

        let isolated_without_attestation = ExecutorPoolRegistration {
            placement: SandboxExecutorPlacement::IsolatedWorker,
            ..registration.clone()
        };
        assert!(isolated_without_attestation.validate().is_err());

        let invalid_fingerprint = ExecutorPoolRegistration {
            fingerprint: RuntimeFingerprint {
                executor_binary_digest: "not-a-digest".to_string(),
                ..fingerprint()
            },
            ..registration
        };
        assert!(invalid_fingerprint.validate().is_err());
    }
}
