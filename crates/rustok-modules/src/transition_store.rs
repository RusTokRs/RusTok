//! SeaORM store for persisting module transition checkpoints and retention holds.
//!
//! Provides crash-resilient storage ensuring operations resume idempotently from
//! database checkpoints and retention holds survive server restarts.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ConnectionTrait, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, Set,
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

pub mod transition_operation_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "module_transition_operations")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub idempotency_key: Uuid,
        pub operation_kind: String,
        pub request_digest: String,
        pub actor_id: Uuid,
        pub tenant_id: Option<Uuid>,
        pub trace_id: String,
        pub correlation_id: Uuid,
        pub operation_id: Uuid,
        pub resulting_revision: i64,
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
    /// Persists or updates an immutable transition checkpoint in the database.
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
            state: Set(state_json),
            security_epoch: Set(checkpoint.security_epoch.value() as i64),
            fences: Set(fences_json),
            recovery_attempt_count: Set(checkpoint.recovery_attempt_count as i32),
            created_at: Set(checkpoint.created_at),
            updated_at: Set(checkpoint.updated_at),
        };

        match transition_checkpoint_entity::Entity::find_by_id(checkpoint.operation_id)
            .one(db)
            .await?
        {
            Some(_) => {
                model.update(db).await?;
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
}

// ============================================================================
// Retention Hold Store
// ============================================================================

pub struct RetentionHoldStore;

impl RetentionHoldStore {
    fn target_identity_key(target: &RetentionTarget) -> (&'static str, String) {
        match target {
            RetentionTarget::SourceCasBlob { digest } => ("source_cas", digest.clone()),
            RetentionTarget::AdmittedPayloadCas { digest } => ("payload_cas", digest.clone()),
            RetentionTarget::NodeSlot {
                node_id,
                slot_digest,
            } => ("node_slot", format!("{node_id}:{slot_digest}")),
            RetentionTarget::RecoveryPoint { snapshot_id } => {
                ("recovery_point", snapshot_id.to_string())
            }
            RetentionTarget::DiagnosticLog { operation_id } => {
                ("diagnostic_log", operation_id.to_string())
            }
        }
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
}
