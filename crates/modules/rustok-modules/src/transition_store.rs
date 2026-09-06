//! SeaORM store for persisting module transition checkpoints and retention holds.
//!
//! Provides crash-resilient storage ensuring operations resume idempotently from
//! database checkpoints and retention holds survive server restarts.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ConnectionTrait, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, Set, Statement,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ConflictFenceSet, GlobalSecurityEpoch, ModuleTransitionCheckpoint, ModuleTransitionState,
    RetentionHoldKind, RetentionHoldLedger, RetentionHoldRecord, RetentionTarget,
};

#[derive(Debug, Error)]
pub enum TransitionStoreError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Checkpoint not found for operation `{0}`")]
    CheckpointNotFound(Uuid),
    #[error(
        "Checkpoint revision conflict for operation `{operation_id}`: expected `{expected}`, current `{current}`"
    )]
    RevisionConflict {
        operation_id: Uuid,
        expected: u64,
        current: u64,
    },
    #[error("Corrupt stored state data: {0}")]
    CorruptData(String),
}

// ============================================================================
// SeaORM Entity: module_transition_checkpoints
// ============================================================================

pub mod transition_checkpoint_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "module_transition_checkpoints")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub operation_id: Uuid,
        pub revision: i64,
        pub module_slug: String,
        pub tenant_id: Option<Uuid>,
        pub predecessor_digest: Option<String>,
        pub candidate_digest: String,
        pub state: serde_json::Value,
        pub security_epoch: i64,
        pub fences: serde_json::Value,
        pub recovery_attempt_count: i32,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================================================
// SeaORM Entity: module_retention_holds
// ============================================================================

pub mod retention_hold_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "module_retention_holds")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub hold_id: Uuid,
        pub target_type: String,
        pub target_identity: String,
        pub target: serde_json::Value,
        pub kind: serde_json::Value,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================================================
// Transition Checkpoint Store
// ============================================================================

pub struct TransitionCheckpointStore;

impl TransitionCheckpointStore {
    /// Inserts a checkpoint or advances it by exactly one owner revision.
    ///
    /// Existing rows are updated with compare-and-swap semantics. A caller
    /// cannot overwrite a concurrent transition decision with a stale copy.
    pub async fn save_checkpoint<C: ConnectionTrait>(
        db: &C,
        checkpoint: &ModuleTransitionCheckpoint,
    ) -> Result<(), TransitionStoreError> {
        let state_json = serde_json::to_value(&checkpoint.state)?;
        let fences_json = serde_json::to_value(&checkpoint.fences)?;

        let model = transition_checkpoint_entity::ActiveModel {
            operation_id: Set(checkpoint.operation_id),
            revision: Set(checkpoint.revision as i64),
            module_slug: Set(checkpoint.module_slug.clone()),
            tenant_id: Set(checkpoint.tenant_id),
            predecessor_digest: Set(checkpoint.predecessor_digest.clone()),
            candidate_digest: Set(checkpoint.candidate_digest.clone()),
            state: Set(state_json.clone()),
            security_epoch: Set(checkpoint.security_epoch.value() as i64),
            fences: Set(fences_json.clone()),
            recovery_attempt_count: Set(checkpoint.recovery_attempt_count as i32),
            created_at: Set(checkpoint.created_at),
            updated_at: Set(checkpoint.updated_at),
        };

        match transition_checkpoint_entity::Entity::find_by_id(checkpoint.operation_id)
            .one(db)
            .await?
        {
            Some(existing) => {
                let current = u64::try_from(existing.revision).map_err(|_| {
                    TransitionStoreError::CorruptData(
                        "Checkpoint revision is outside the supported range".to_string(),
                    )
                })?;
                let expected = checkpoint.revision.checked_sub(1).ok_or_else(|| {
                    TransitionStoreError::CorruptData(
                        "Checkpoint revision must be positive".to_string(),
                    )
                })?;
                if current != expected {
                    return Err(TransitionStoreError::RevisionConflict {
                        operation_id: checkpoint.operation_id,
                        expected,
                        current,
                    });
                }

                let backend = db.get_database_backend();
                let result = db
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "UPDATE module_transition_checkpoints SET revision = {}, state = {}, \
                             security_epoch = {}, fences = {}, recovery_attempt_count = {}, updated_at = {} \
                             WHERE operation_id = {} AND revision = {}",
                            crate::data::placeholder(backend, 1),
                            crate::data::placeholder(backend, 2),
                            crate::data::placeholder(backend, 3),
                            crate::data::placeholder(backend, 4),
                            crate::data::placeholder(backend, 5),
                            crate::data::placeholder(backend, 6),
                            crate::data::placeholder(backend, 7),
                            crate::data::placeholder(backend, 8),
                        ),
                        vec![
                            i64::try_from(checkpoint.revision)
                                .map_err(|_| TransitionStoreError::CorruptData(
                                    "Checkpoint revision exceeds the database range".to_string(),
                                ))?
                                .into(),
                            state_json.into(),
                            i64::try_from(checkpoint.security_epoch.value())
                                .map_err(|_| TransitionStoreError::CorruptData(
                                    "Security epoch exceeds the database range".to_string(),
                                ))?
                                .into(),
                            fences_json.into(),
                            i32::try_from(checkpoint.recovery_attempt_count)
                                .map_err(|_| TransitionStoreError::CorruptData(
                                    "Recovery attempt count exceeds the database range".to_string(),
                                ))?
                                .into(),
                            checkpoint.updated_at.into(),
                            crate::data::uuid_value(checkpoint.operation_id, backend),
                            i64::try_from(expected)
                                .map_err(|_| TransitionStoreError::CorruptData(
                                    "Expected checkpoint revision exceeds the database range".to_string(),
                                ))?
                                .into(),
                        ],
                    ))
                    .await?;
                if result.rows_affected() != 1 {
                    let current = Self::load_checkpoint(db, checkpoint.operation_id)
                        .await?
                        .ok_or(TransitionStoreError::CheckpointNotFound(
                            checkpoint.operation_id,
                        ))?
                        .revision;
                    return Err(TransitionStoreError::RevisionConflict {
                        operation_id: checkpoint.operation_id,
                        expected,
                        current,
                    });
                }
            }
            None => {
                model.insert(db).await?;
            }
        }

        Ok(())
    }

    /// Loads a transition checkpoint by operation ID.
    pub async fn load_checkpoint<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<Option<ModuleTransitionCheckpoint>, TransitionStoreError> {
        let model = match transition_checkpoint_entity::Entity::find_by_id(operation_id)
            .one(db)
            .await?
        {
            Some(m) => m,
            None => return Ok(None),
        };

        let state: ModuleTransitionState = serde_json::from_value(model.state)
            .map_err(|e| TransitionStoreError::CorruptData(format!("Invalid state JSON: {e}")))?;
        let fences: ConflictFenceSet = serde_json::from_value(model.fences)
            .map_err(|e| TransitionStoreError::CorruptData(format!("Invalid fences JSON: {e}")))?;

        Ok(Some(ModuleTransitionCheckpoint {
            operation_id: model.operation_id,
            revision: model.revision.max(0) as u64,
            module_slug: model.module_slug,
            tenant_id: model.tenant_id,
            predecessor_digest: model.predecessor_digest,
            candidate_digest: model.candidate_digest,
            state,
            security_epoch: GlobalSecurityEpoch(model.security_epoch as u64),
            fences,
            recovery_attempt_count: model.recovery_attempt_count.max(0) as u32,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }))
    }

    /// Lists all active (non-terminal) module transition checkpoints.
    pub async fn list_active_checkpoints<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, TransitionStoreError> {
        let models = transition_checkpoint_entity::Entity::find().all(db).await?;
        let mut checkpoints = Vec::new();

        for model in models {
            let state: ModuleTransitionState =
                serde_json::from_value(model.state).map_err(|e| {
                    TransitionStoreError::CorruptData(format!("Invalid state JSON: {e}"))
                })?;
            if !state.is_terminal() {
                let fences: ConflictFenceSet =
                    serde_json::from_value(model.fences).map_err(|e| {
                        TransitionStoreError::CorruptData(format!("Invalid fences JSON: {e}"))
                    })?;

                checkpoints.push(ModuleTransitionCheckpoint {
                    operation_id: model.operation_id,
                    revision: model.revision.max(0) as u64,
                    module_slug: model.module_slug,
                    tenant_id: model.tenant_id,
                    predecessor_digest: model.predecessor_digest,
                    candidate_digest: model.candidate_digest,
                    state,
                    security_epoch: GlobalSecurityEpoch(model.security_epoch as u64),
                    fences,
                    recovery_attempt_count: model.recovery_attempt_count.max(0) as u32,
                    created_at: model.created_at,
                    updated_at: model.updated_at,
                });
            }
        }

        Ok(checkpoints)
    }

    /// Finds any active (non-terminal) checkpoint for a specific module slug and optional tenant.
    pub async fn find_active_checkpoint_for_module<C: ConnectionTrait>(
        db: &C,
        module_slug: &str,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<ModuleTransitionCheckpoint>, TransitionStoreError> {
        let active = Self::list_active_checkpoints(db).await?;
        Ok(active.into_iter().find(|cp| {
            cp.module_slug == module_slug && (tenant_id.is_none() || cp.tenant_id == tenant_id)
        }))
    }

    /// Finds any active checkpoint currently under observation (`Observing` state) for a module.
    pub async fn find_active_observing_checkpoint<C: ConnectionTrait>(
        db: &C,
        module_slug: &str,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<ModuleTransitionCheckpoint>, TransitionStoreError> {
        let active = Self::list_active_checkpoints(db).await?;
        Ok(active.into_iter().find(|cp| {
            cp.module_slug == module_slug
                && (tenant_id.is_none() || cp.tenant_id == tenant_id)
                && matches!(cp.state, ModuleTransitionState::Observing { .. })
        }))
    }
}

// ============================================================================
// Retention Hold Store
// ============================================================================

pub struct RetentionHoldStore;

impl RetentionHoldStore {
    fn target_identity_key(target: &RetentionTarget) -> (&'static str, String) {
        target.identity_key()
    }

    /// Persists a new retention hold record to the database.
    pub async fn insert_hold<C: ConnectionTrait>(
        db: &C,
        record: &RetentionHoldRecord,
    ) -> Result<(), TransitionStoreError> {
        let (target_type, target_identity) = Self::target_identity_key(&record.target);
        let target_json = serde_json::to_value(&record.target)?;
        let kind_json = serde_json::to_value(&record.kind)?;

        let model = retention_hold_entity::ActiveModel {
            hold_id: Set(record.hold_id),
            target_type: Set(target_type.to_string()),
            target_identity: Set(target_identity),
            target: Set(target_json),
            kind: Set(kind_json),
            created_at: Set(record.created_at),
        };

        model.insert(db).await?;
        Ok(())
    }

    /// Deletes a retention hold record from the database upon hold release.
    pub async fn delete_hold<C: ConnectionTrait>(
        db: &C,
        hold_id: Uuid,
    ) -> Result<bool, TransitionStoreError> {
        let result = retention_hold_entity::Entity::delete_by_id(hold_id)
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Loads all active retention hold records from the database.
    pub async fn list_active_holds<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<RetentionHoldRecord>, TransitionStoreError> {
        let models = retention_hold_entity::Entity::find().all(db).await?;
        let mut records = Vec::with_capacity(models.len());

        for m in models {
            let target: RetentionTarget = serde_json::from_value(m.target).map_err(|e| {
                TransitionStoreError::CorruptData(format!("Invalid target JSON: {e}"))
            })?;
            let kind: RetentionHoldKind = serde_json::from_value(m.kind).map_err(|e| {
                TransitionStoreError::CorruptData(format!("Invalid kind JSON: {e}"))
            })?;

            records.push(RetentionHoldRecord {
                hold_id: m.hold_id,
                target,
                kind,
                created_at: m.created_at,
            });
        }

        Ok(records)
    }

    /// Loads all active retention holds from the database and constructs a `RetentionHoldLedger`.
    pub async fn load_active_ledger<C: ConnectionTrait>(
        db: &C,
    ) -> Result<RetentionHoldLedger, TransitionStoreError> {
        let records = Self::list_active_holds(db).await?;
        let mut ledger = RetentionHoldLedger::new();
        for r in records {
            ledger.place_hold(r.target, r.kind);
        }
        Ok(ledger)
    }

    /// Releases all active rollout window holds for a given transition operation.
    pub async fn release_holds_for_operation<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<u64, TransitionStoreError> {
        let active = Self::list_active_holds(db).await?;
        let mut released_count = 0;
        for record in active {
            let matches_op = match &record.kind {
                RetentionHoldKind::ActiveRolloutWindow {
                    operation_id: op, ..
                } => *op == operation_id,
                _ => false,
            };
            if matches_op && Self::delete_hold(db, record.hold_id).await? {
                released_count += 1;
            }
        }
        Ok(released_count)
    }
}
