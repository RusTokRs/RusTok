//! Durable owner service for module transition convergence.

use rustok_api::{
    ModuleRetentionHoldView, ModuleTransitionCheckpointView, ModuleTransitionStateView,
};
use rustok_events::DomainEvent;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, QueryResult, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use std::time::Duration;

use chrono::Utc;
use rustok_api::manifest_hash::hash_manifest;

use crate::backend::SupportedBackend;
use crate::installation::GovernedArtifactActivation;
use crate::{
    ArtifactActivationRequest, ArtifactActivationResult, ControlPlaneInfrastructure,
    EvaluateReadinessCommand, ExecutorPoolIdentity, ExecutorReadinessError,
    ExecutorReadinessService, ExecutorSmokeReceipt, InstalledModuleArtifact,
    MAX_EXPIRED_RETENTION_HOLDS_PER_REAP, MigrationPreflightInput, MigrationPreflightReceipt,
    ModuleBindingIdempotency, ModuleCommandContext, ModuleInstallationScope,
    ModuleRuntimeBindingKind, ModuleTransitionCheckpoint, ModuleTransitionCoordinator,
    ModuleTransitionFinalizeCommand, ModuleTransitionState, OwnerPlacementPolicy,
    ReleaseReadinessTarget, RetentionHoldReapReport, RetentionHoldStore,
    SeaOrmArtifactInstallationStore, SeaOrmSecurityEpochStore, StartTransitionInput,
    TransitionCheckpointStore, TransitionCoordinatorError, TransitionStoreError, UpdateMode,
    data::uuid_from_row, evaluate_migration_preflight,
};

const OPERATION_KIND: &str = "finalize";

impl From<ModuleTransitionCheckpoint> for ModuleTransitionCheckpointView {
    fn from(checkpoint: ModuleTransitionCheckpoint) -> Self {
        let (state, state_details) = match checkpoint.state {
            crate::ModuleTransitionState::Preflighting => {
                (ModuleTransitionStateView::Preflighting, None)
            }
            crate::ModuleTransitionState::Fenced => (ModuleTransitionStateView::Fenced, None),
            crate::ModuleTransitionState::PreStaging => {
                (ModuleTransitionStateView::Prestaging, None)
            }
            crate::ModuleTransitionState::Activating => {
                (ModuleTransitionStateView::Activating, None)
            }
            crate::ModuleTransitionState::Observing { timeout_at } => (
                ModuleTransitionStateView::Observing,
                Some(format!("Timeout at {}", timeout_at.to_rfc3339())),
            ),
            crate::ModuleTransitionState::PointOfNoReturn {
                reason,
                committed_at,
            } => (
                ModuleTransitionStateView::PointOfNoReturn,
                Some(format!(
                    "Committed at {}: {}",
                    committed_at.to_rfc3339(),
                    reason
                )),
            ),
            crate::ModuleTransitionState::RecoveredToPredecessor { failure_reason, .. } => (
                ModuleTransitionStateView::RecoveredToPredecessor,
                Some(failure_reason),
            ),
            crate::ModuleTransitionState::Converged { finalized_at } => (
                ModuleTransitionStateView::Converged,
                Some(format!("Finalized at {}", finalized_at.to_rfc3339())),
            ),
            crate::ModuleTransitionState::InterventionRequired { failure_reason } => (
                ModuleTransitionStateView::InterventionRequired,
                Some(failure_reason),
            ),
        };

        Self {
            operation_id: checkpoint.operation_id.to_string(),
            revision: i64::try_from(checkpoint.revision)
                .expect("persisted transition revisions are constrained to BIGINT"),
            module_slug: checkpoint.module_slug,
            tenant_id: checkpoint.tenant_id.map(|tenant_id| tenant_id.to_string()),
            predecessor_digest: checkpoint.predecessor_digest,
            candidate_digest: checkpoint.candidate_digest,
            state,
            state_details,
            security_epoch: i64::try_from(checkpoint.security_epoch.value())
                .expect("persisted transition security epochs are constrained to BIGINT"),
            recovery_attempt_count: i32::try_from(checkpoint.recovery_attempt_count)
                .expect("persisted transition recovery attempts are constrained to INTEGER"),
            created_at: checkpoint.created_at.to_rfc3339(),
            updated_at: checkpoint.updated_at.to_rfc3339(),
        }
    }
}

impl From<crate::RetentionHoldRecord> for ModuleRetentionHoldView {
    fn from(record: crate::RetentionHoldRecord) -> Self {
        let (target_type, target_identity) = record.target.identity_key();
        Self {
            hold_id: record.hold_id.to_string(),
            target_type: target_type.to_string(),
            target_identity,
            kind: serde_json::to_string(&record.kind)
                .expect("retention hold kinds are serializable owner contracts"),
            created_at: record.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleTransitionFinalizeReceipt {
    pub checkpoint: ModuleTransitionCheckpoint,
    pub released_holds: u64,
    pub created: bool,
}

/// Command to start one governed *automatic* artifact transition.
///
/// The host supplies authenticated authorization, the exact admitted
/// candidate installation, its scope, and the executor topology facts
/// (placement policy and every pool that may serve or recover the
/// release). It does not supply a predecessor, migration classification,
/// readiness evidence, or checkpoint state: the owner resolves the
/// serving predecessor, derives data/settings preflight from the exact
/// admitted descriptors, executes smoke readiness through the injected
/// readiness service, and binds all of it to one durable checkpoint
/// before serving selection changes.
#[derive(Debug, Clone)]
pub struct GovernedArtifactTransitionCommand {
    pub candidate_installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_candidate_revision: u64,
    pub context: ModuleCommandContext,
    pub reason: String,
    /// Authenticated host fact. The owner rejects a command that did not
    /// pass the platform `modules:manage` authorization boundary.
    pub actor_can_manage_modules: bool,
    pub placement_policy: OwnerPlacementPolicy,
    pub required_pools: Vec<ExecutorPoolIdentity>,
    pub observation_duration: Duration,
}

/// Receipt of a governed automatic transition that reached `Observing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedArtifactTransitionReceipt {
    pub checkpoint: ModuleTransitionCheckpoint,
    pub activation: ArtifactActivationResult,
    /// Owner-derived migration/data preflight evaluated by this call.
    /// `None` only when resuming a checkpoint that had already reached
    /// `Observing` in a previous call: the preflight was evaluated then,
    /// and the checkpoint plus activation operation are the durable
    /// evidence returned here.
    pub preflight: Option<MigrationPreflightReceipt>,
    /// Durable smoke-readiness receipts produced by this call for the
    /// local executor pool (candidate first, then predecessor). Empty
    /// only on the already-`Observing` resume path above.
    pub readiness_receipt_ids: Vec<Uuid>,
    pub resumed: bool,
}

#[derive(Debug, Error)]
pub enum GovernedArtifactTransitionError {
    #[error("governed transition command is invalid: {0}")]
    InvalidCommand(String),
    #[error("governed transition authorization denied")]
    AuthorizationDenied,
    #[error("governed transition conflict: {0}")]
    Conflict(String),
    #[error("governed transition preflight denied automatic mode: {reasons:?}")]
    PreflightFailed { reasons: Vec<String> },
    #[error(transparent)]
    Readiness(#[from] ExecutorReadinessError),
    #[error(transparent)]
    Coordinator(#[from] TransitionCoordinatorError),
    #[error("governed transition installation owner failed: {0}")]
    Installation(String),
    #[error("governed transition store failed: {0}")]
    Store(String),
}

#[derive(Debug, Error)]
pub enum ModuleTransitionServiceError {
    #[error("transition command is invalid: {0}")]
    InvalidCommand(String),
    #[error("module transition authorization denied")]
    AuthorizationDenied,
    #[error("transition checkpoint `{0}` was not found")]
    NotFound(Uuid),
    #[error("transition revision conflict: expected `{expected}`, current `{current}")]
    RevisionConflict { expected: u64, current: u64 },
    #[error("transition idempotency key was used for a different command")]
    IdempotencyConflict,
    #[error("transition operation is still in progress")]
    OperationInProgress,
    #[error(transparent)]
    Coordinator(#[from] TransitionCoordinatorError),
    #[error("transition store failed: {0}")]
    Store(String),
    #[error("transition outbox append failed: {0}")]
    Outbox(String),
}

#[derive(Clone)]
pub struct SeaOrmModuleTransitionService {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
}

impl SeaOrmModuleTransitionService {
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self { db, infrastructure }
    }

    pub async fn checkpoint(
        &self,
        operation_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
        let checkpoint = TransitionCheckpointStore::load_checkpoint(&self.db, operation_id)
            .await
            .map_err(store_error)?;
        Ok(checkpoint.filter(|checkpoint| checkpoint.tenant_id == tenant_id))
    }

    /// Lists unresolved checkpoints in one scope, including checkpoints
    /// whose automatic progression stopped at `InterventionRequired`.
    pub async fn active_checkpoints(
        &self,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
        TransitionCheckpointStore::list_active_checkpoints_for_tenant(&self.db, tenant_id)
            .await
            .map_err(store_error)
    }

    pub async fn active_checkpoint_for_module(
        &self,
        module_slug: &str,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
        Ok(self
            .active_checkpoints(tenant_id)
            .await?
            .into_iter()
            .find(|checkpoint| checkpoint.module_slug == module_slug))
    }

    pub async fn active_observing_checkpoint_for_module(
        &self,
        module_slug: &str,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
        Ok(self
            .active_checkpoints(tenant_id)
            .await?
            .into_iter()
            .find(|checkpoint| {
                checkpoint.module_slug == module_slug
                    && matches!(
                        checkpoint.state,
                        crate::ModuleTransitionState::Observing { .. }
                    )
            }))
    }

    pub async fn retention_holds(
        &self,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<crate::RetentionHoldRecord>, ModuleTransitionServiceError> {
        let operation_ids = self
            .active_checkpoints(tenant_id)
            .await?
            .into_iter()
            .map(|checkpoint| checkpoint.operation_id)
            .collect::<std::collections::HashSet<_>>();
        let holds = RetentionHoldStore::list_active_holds(&self.db)
            .await
            .map_err(store_error)?;
        Ok(holds
            .into_iter()
            .filter(|hold| {
                matches!(
                    hold.kind,
                    crate::RetentionHoldKind::ActiveRolloutWindow { operation_id, .. }
                        if operation_ids.contains(&operation_id)
                )
            })
            .collect())
    }

    /// Starts one governed automatic artifact transition and drives it
    /// to `Observing`.
    ///
    /// Order is part of the safety contract: resolve the exact candidate
    /// and serving predecessor; derive migration/data preflight from
    /// their admitted descriptors (never from caller assertions); durably
    /// record the checkpoint in `PreStaging`; execute candidate and
    /// predecessor smoke readiness outside any database transaction and
    /// require automatic-mode eligibility on every required pool; only
    /// then change serving selection through the governed activation,
    /// which revalidates the checkpoint under the activation lock and
    /// commits checkpoint `Observing` plus predecessor retention in the
    /// same transaction as the selection change.
    ///
    /// A retry discovers the unresolved checkpoint for the same
    /// slug/scope/candidate and resumes it. A different unresolved
    /// governed transition for the slug, a changed predecessor, a
    /// maintenance-classified data/settings change, or missing smoke
    /// readiness on any required pool fails closed without changing
    /// serving selection. Maintenance-mode data/settings migrations are
    /// deliberately not started here: they require their dedicated
    /// guarded migration operations before any activation exists.
    pub async fn start_governed_automatic_artifact_transition(
        &self,
        command: GovernedArtifactTransitionCommand,
        readiness_services: &[ExecutorReadinessService],
    ) -> Result<GovernedArtifactTransitionReceipt, GovernedArtifactTransitionError> {
        validate_governed_command(&command)?;
        if readiness_services.is_empty() {
            return Err(GovernedArtifactTransitionError::InvalidCommand(
                "at least one executor readiness service must be composed".to_string(),
            ));
        }
        if !command.actor_can_manage_modules {
            return Err(GovernedArtifactTransitionError::AuthorizationDenied);
        }
        let tenant_id = match &command.scope {
            ModuleInstallationScope::Platform => None,
            ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
        };
        let installations = SeaOrmArtifactInstallationStore::with_infrastructure(
            self.db.clone(),
            self.infrastructure.clone(),
        );
        let candidate = installations
            .load_admitted_installation_exact(command.candidate_installation_id, &command.scope)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Installation(error.to_string()))?;
        let candidate_digest = candidate.descriptor.artifact_digest.clone();

        let mut resumed = false;
        let mut checkpoint_to_resume = None;
        let existing_for_slug =
            TransitionCheckpointStore::list_active_checkpoints_for_tenant(&self.db, tenant_id)
                .await
                .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?
                .into_iter()
                .filter(|checkpoint| checkpoint.module_slug == candidate.release.slug)
                .collect::<Vec<_>>();
        if let Some(existing) = existing_for_slug
            .iter()
            .find(|checkpoint| checkpoint.candidate_digest == candidate_digest)
        {
            match &existing.state {
                ModuleTransitionState::Observing { .. } => {
                    let activation = installations
                        .load_activation_result(existing.operation_id, &command.scope)
                        .await
                        .map_err(|error| {
                            GovernedArtifactTransitionError::Installation(error.to_string())
                        })?
                        .ok_or_else(|| {
                            GovernedArtifactTransitionError::Store(format!(
                                "observing governed checkpoint `{}` has no activation operation",
                                existing.operation_id
                            ))
                        })?;
                    return Ok(GovernedArtifactTransitionReceipt {
                        checkpoint: existing.clone(),
                        activation,
                        preflight: None,
                        readiness_receipt_ids: Vec::new(),
                        resumed: true,
                    });
                }
                ModuleTransitionState::Fenced | ModuleTransitionState::PreStaging => {
                    resumed = true;
                    checkpoint_to_resume = Some(existing.clone());
                }
                state => {
                    return Err(GovernedArtifactTransitionError::Conflict(format!(
                        "governed transition `{}` for this candidate is in `{}` and cannot be started again",
                        existing.operation_id,
                        state.name()
                    )));
                }
            }
        } else if let Some(other) = existing_for_slug.first() {
            return Err(GovernedArtifactTransitionError::Conflict(format!(
                "unresolved governed transition `{}` already owns module `{}` in this scope",
                other.operation_id, candidate.release.slug
            )));
        }

        let predecessor = installations
            .load_active_installation_for_scope(&command.scope, &candidate.release.slug)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Installation(error.to_string()))?;
        if predecessor
            .as_ref()
            .is_some_and(|artifact| artifact.installation_id == candidate.installation_id)
        {
            return Err(GovernedArtifactTransitionError::Conflict(
                "candidate installation is already the serving selection".to_string(),
            ));
        }
        let predecessor_digest = predecessor
            .as_ref()
            .map(|artifact| artifact.descriptor.artifact_digest.clone());
        if let Some(checkpoint) = &checkpoint_to_resume
            && checkpoint.predecessor_digest != predecessor_digest
        {
            return Err(GovernedArtifactTransitionError::Conflict(format!(
                "governed transition `{}` was staged against a different serving predecessor",
                checkpoint.operation_id
            )));
        }

        let operation_id = checkpoint_to_resume
            .as_ref()
            .map(|checkpoint| checkpoint.operation_id)
            .unwrap_or_else(|| self.infrastructure.new_id());
        let preflight = derive_governed_preflight(operation_id, &candidate, predecessor.as_ref())?;
        if preflight.mode != UpdateMode::Automatic {
            return Err(GovernedArtifactTransitionError::PreflightFailed {
                reasons: preflight.denial_reasons.clone(),
            });
        }

        let security_registry = SeaOrmSecurityEpochStore::load_registry(&self.db)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        let _coordinator = match checkpoint_to_resume {
            Some(checkpoint) => {
                let mut coordinator = ModuleTransitionCoordinator::new(checkpoint);
                if coordinator.state() == &ModuleTransitionState::Fenced {
                    coordinator.advance_to_prestaging(&security_registry)?;
                    self.save_governed_checkpoint(coordinator.checkpoint())
                        .await?;
                }
                coordinator
            }
            None => {
                let mut coordinator = ModuleTransitionCoordinator::start_transition(
                    StartTransitionInput {
                        operation_id,
                        module_slug: candidate.release.slug.clone(),
                        tenant_id,
                        predecessor_digest: predecessor_digest.clone(),
                        candidate_digest: candidate_digest.clone(),
                        affected_nodes: command
                            .required_pools
                            .iter()
                            .map(|pool| pool.pool_id.clone())
                            .collect(),
                        preflight_receipt: preflight.clone(),
                        requested_mode: UpdateMode::Automatic,
                    },
                    &security_registry,
                )?;
                coordinator.advance_to_prestaging(&security_registry)?;
                self.save_governed_checkpoint(coordinator.checkpoint())
                    .await?;
                coordinator
            }
        };

        let candidate_receipt = evaluate_readiness_on_supporting_pool(
            readiness_services,
            &command,
            operation_id,
            &candidate,
        )
        .await?;
        let mut readiness_receipt_ids = vec![candidate_receipt.id];
        if let Some(predecessor) = &predecessor {
            let predecessor_receipt = evaluate_readiness_on_supporting_pool(
                readiness_services,
                &command,
                operation_id,
                predecessor,
            )
            .await?;
            readiness_receipt_ids.push(predecessor_receipt.id);
        }
        readiness_services[0]
            .check_automatic_mode_eligibility(
                &candidate.reference.digest,
                predecessor
                    .as_ref()
                    .map(|artifact| artifact.reference.digest.as_str()),
                &command.required_pools,
                &command.placement_policy,
            )
            .await?;

        let activation = match installations
            .activate_artifact_governed(
                ArtifactActivationRequest {
                    installation_id: command.candidate_installation_id,
                    scope: command.scope.clone(),
                    expected_revision: command.expected_candidate_revision,
                    context: command.context.clone(),
                    reason: command.reason.clone(),
                },
                GovernedArtifactActivation {
                    operation_id,
                    expected_candidate_digest: candidate_digest,
                    expected_predecessor_digest: predecessor_digest,
                    observation_duration: command.observation_duration,
                },
            )
            .await
        {
            Ok(activation) => activation,
            Err(error) => {
                self.mark_governed_checkpoint_intervention_if_stale(
                    operation_id,
                    tenant_id,
                    &installations,
                    &command.scope,
                    &candidate.release.slug,
                )
                .await
                .map_err(|mark_error| {
                    GovernedArtifactTransitionError::Store(format!(
                        "governed activation failed ({error}); recording intervention also failed ({mark_error})"
                    ))
                })?;
                return Err(GovernedArtifactTransitionError::Installation(
                    error.to_string(),
                ));
            }
        };
        let checkpoint = self
            .checkpoint(operation_id, tenant_id)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?
            .ok_or_else(|| {
                GovernedArtifactTransitionError::Store(format!(
                    "governed checkpoint `{operation_id}` is missing after activation"
                ))
            })?;
        Ok(GovernedArtifactTransitionReceipt {
            checkpoint,
            activation,
            preflight: Some(preflight),
            readiness_receipt_ids,
            resumed,
        })
    }

    /// Marks a still-`PreStaging` governed checkpoint
    /// `InterventionRequired` when activation proved its staged
    /// predecessor pair or security epoch stale. Transient activation
    /// failures leave the checkpoint resumable; a stale pair can never
    /// become valid again for this operation, so automatic action stops
    /// and the checkpoint surfaces to operators instead of fencing the
    /// module forever in an ambiguous state.
    async fn mark_governed_checkpoint_intervention_if_stale(
        &self,
        operation_id: Uuid,
        tenant_id: Option<Uuid>,
        installations: &SeaOrmArtifactInstallationStore,
        scope: &ModuleInstallationScope,
        module_slug: &str,
    ) -> Result<(), GovernedArtifactTransitionError> {
        let Some(observed) = self
            .checkpoint(operation_id, tenant_id)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?
        else {
            return Ok(());
        };
        if !matches!(
            observed.state,
            ModuleTransitionState::Fenced | ModuleTransitionState::PreStaging
        ) {
            return Ok(());
        }
        let mut reasons = Vec::new();
        let security_registry = SeaOrmSecurityEpochStore::load_registry(&self.db)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        if security_registry
            .validate_epoch(observed.security_epoch)
            .is_err()
        {
            reasons.push("security epoch advanced after the transition was staged".to_string());
        }
        let current_predecessor = installations
            .load_active_installation_for_scope(scope, module_slug)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Installation(error.to_string()))?;
        let current_predecessor_digest = current_predecessor
            .as_ref()
            .map(|artifact| artifact.descriptor.artifact_digest.clone());
        if observed.predecessor_digest != current_predecessor_digest {
            reasons.push("serving predecessor changed after the transition was staged".to_string());
        }
        if reasons.is_empty() {
            return Ok(());
        }
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        match tenant_id {
            Some(tenant_id) => crate::backend::set_tenant_scope(&transaction, tenant_id).await,
            None => crate::backend::set_platform_scope(&transaction).await,
        }
        .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        let current = TransitionCheckpointStore::load_checkpoint(&transaction, operation_id)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        let Some(current) = current else {
            return Ok(());
        };
        if current.revision != observed.revision
            || !matches!(
                current.state,
                ModuleTransitionState::Fenced | ModuleTransitionState::PreStaging
            )
        {
            return Ok(());
        }
        let mut coordinator = ModuleTransitionCoordinator::new(current);
        coordinator.require_intervention(format!(
            "Governed transition cannot activate: {}",
            reasons.join("; ")
        ))?;
        TransitionCheckpointStore::save_checkpoint(&transaction, coordinator.checkpoint())
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))
    }

    async fn save_governed_checkpoint(
        &self,
        checkpoint: &ModuleTransitionCheckpoint,
    ) -> Result<(), GovernedArtifactTransitionError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        match checkpoint.tenant_id {
            Some(tenant_id) => crate::backend::set_tenant_scope(&transaction, tenant_id).await,
            None => crate::backend::set_platform_scope(&transaction).await,
        }
        .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        TransitionCheckpointStore::save_checkpoint(&transaction, checkpoint)
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))
    }

    pub async fn finalize(
        &self,
        command: ModuleTransitionFinalizeCommand,
    ) -> Result<ModuleTransitionFinalizeReceipt, ModuleTransitionServiceError> {
        validate_command(&command)?;
        if !command.actor_can_manage_modules {
            return Err(ModuleTransitionServiceError::AuthorizationDenied);
        }
        let request_digest = request_digest(&command)?;
        if let Some(receipt) = load_operation(&self.db, &command, &request_digest).await? {
            return Ok(receipt);
        }

        let transaction = self.db.begin().await.map_err(database_error)?;
        match command.context.tenant_id {
            Some(tenant_id) => crate::backend::set_tenant_scope(&transaction, tenant_id).await,
            None => crate::backend::set_platform_scope(&transaction).await,
        }
        .map_err(database_error)?;
        if let Some(receipt) = load_operation(&transaction, &command, &request_digest).await? {
            return Ok(receipt);
        }
        let checkpoint =
            TransitionCheckpointStore::load_checkpoint(&transaction, command.operation_id)
                .await
                .map_err(store_error)?
                .ok_or(ModuleTransitionServiceError::NotFound(command.operation_id))?;
        if checkpoint.tenant_id != command.context.tenant_id {
            return Err(ModuleTransitionServiceError::AuthorizationDenied);
        }
        if checkpoint.revision != command.expected_revision {
            return Err(ModuleTransitionServiceError::RevisionConflict {
                expected: command.expected_revision,
                current: checkpoint.revision,
            });
        }
        if let Some(receipt) = reserve_operation(&transaction, &command, &request_digest).await? {
            return Ok(receipt);
        }

        // Validate against the durable platform epoch: a quarantine or
        // revocation recorded by any process since the checkpoint was taken
        // must preempt this finalization. (Previously a fresh in-memory
        // registry pinned at epoch 1 made the check a permanent no-op.)
        let security_registry = SeaOrmSecurityEpochStore::load_registry(&transaction)
            .await
            .map_err(database_error)?;
        let mut coordinator = ModuleTransitionCoordinator::new(checkpoint);
        coordinator.finalize_convergence(&security_registry)?;
        TransitionCheckpointStore::save_checkpoint(&transaction, coordinator.checkpoint())
            .await
            .map_err(store_error)?;
        let released_holds =
            RetentionHoldStore::release_holds_for_operation(&transaction, command.operation_id)
                .await
                .map_err(store_error)?;
        let receipt = ModuleTransitionFinalizeReceipt {
            checkpoint: coordinator.checkpoint().clone(),
            released_holds,
            created: true,
        };
        complete_operation(&transaction, command.context.idempotency_key, &receipt).await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &command.context,
                    DomainEvent::ModuleTransitionFinalized {
                        operation_id: command.operation_id,
                        module_slug: receipt.checkpoint.module_slug.clone(),
                        revision: receipt.checkpoint.revision,
                        released_holds,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleTransitionServiceError::Outbox(error.to_string()))?;
        transaction.commit().await.map_err(database_error)?;
        Ok(receipt)
    }

    /// Sweeps active transition checkpoints against the durable security
    /// epoch and their observation windows.
    ///
    /// Convergence is an attested decision, never an automatic timeout
    /// consequence: the checkpoint model records no positive health evidence
    /// during `Observing`, so an elapsed window proves nothing about the
    /// candidate. The watchdog therefore marks both stale-epoch and
    /// expired-window checkpoints `InterventionRequired`. That state stops
    /// automatic transition action but deliberately does not change serving
    /// selection; an operator must explicitly rollback or deactivate the
    /// candidate through the installation owner if traffic must move. The
    /// only convergence path is the explicit, actor-authorized
    /// [`finalize`](Self::finalize) command before intervention is required.
    pub async fn evaluate_watchdog(
        &self,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
        // The epoch is durable and shared across processes; a snapshot taken
        // at sweep start is sufficient because a racing advancement is then
        // picked up by the next sweep, and `finalize` re-reads the epoch
        // inside its own transaction.
        let security_registry = SeaOrmSecurityEpochStore::load_registry(&self.db)
            .await
            .map_err(database_error)?;
        let active = TransitionCheckpointStore::list_active_checkpoints(&self.db)
            .await
            .map_err(store_error)?;
        let mut updated = Vec::new();
        for observed in active {
            // The unresolved listing deliberately includes
            // intervention-required checkpoints for operator discovery.
            // Automatic actors must not rewrite or re-emit them on every
            // sweep; only the explicit operator recovery path may move one.
            if observed.state.requires_operator_attention() {
                continue;
            }
            let stale_epoch = security_registry
                .validate_epoch(observed.security_epoch)
                .is_err();
            let timed_out = matches!(
                observed.state,
                crate::ModuleTransitionState::Observing { timeout_at }
                    if self.infrastructure.now() >= timeout_at
            );
            let failure_reason = if stale_epoch {
                format!(
                    "Security epoch preemption: epoch {} is stale; current epoch is {}. Automatic rollback requires a fresh owner policy grant and was not attempted; serving selection was not changed by the watchdog.",
                    observed.security_epoch.value(),
                    security_registry.current_epoch().value(),
                )
            } else if timed_out {
                "Observation window elapsed without an explicit finalize attestation; \
                 no positive health evidence is recorded for the candidate, so automatic \
                 convergence is not permitted. Operator intervention is required; serving \
                 selection was not changed by the watchdog."
                    .to_string()
            } else {
                continue;
            };

            let transaction = self.db.begin().await.map_err(database_error)?;
            match observed.tenant_id {
                Some(tenant_id) => crate::backend::set_tenant_scope(&transaction, tenant_id).await,
                None => crate::backend::set_platform_scope(&transaction).await,
            }
            .map_err(database_error)?;
            let current =
                TransitionCheckpointStore::load_checkpoint(&transaction, observed.operation_id)
                    .await
                    .map_err(store_error)?
                    .ok_or(ModuleTransitionServiceError::NotFound(
                        observed.operation_id,
                    ))?;
            if current.revision != observed.revision || current.state.is_terminal() {
                continue;
            }
            let mut coordinator = ModuleTransitionCoordinator::new(current);
            coordinator.require_intervention(failure_reason.clone())?;
            TransitionCheckpointStore::save_checkpoint(&transaction, coordinator.checkpoint())
                .await
                .map_err(store_error)?;
            // The v1 root event contract retains its historical
            // `module.transition.failed_closed` name. Its checkpoint fact is
            // that automatic action stopped and intervention is required; it
            // does not assert that serving selection changed.
            self.infrastructure
                .write_event(
                    &transaction,
                    self.infrastructure.event_envelope(
                        coordinator.checkpoint().tenant_id,
                        None,
                        DomainEvent::ModuleTransitionFailedClosed {
                            operation_id: coordinator.checkpoint().operation_id,
                            module_slug: coordinator.checkpoint().module_slug.clone(),
                            revision: coordinator.checkpoint().revision,
                            failure_reason,
                        },
                    ),
                )
                .await
                .map_err(|error| ModuleTransitionServiceError::Outbox(error.to_string()))?;
            transaction.commit().await.map_err(database_error)?;
            updated.push(coordinator.checkpoint().clone());
        }
        Ok(updated)
    }
}

pub async fn evaluate_transition_watchdog(
    db: &DatabaseConnection,
) -> Result<Vec<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
    SeaOrmModuleTransitionService::with_infrastructure(
        db.clone(),
        ControlPlaneInfrastructure::for_database(db.clone()),
    )
    .evaluate_watchdog()
    .await
}

/// Deletes one bounded batch of elapsed rollout-window retention holds.
/// Hosts run this independently of checkpoint evaluation so retention stays
/// time-bounded even when a transition is never finalized or rolled back.
pub async fn reap_expired_retention_holds(
    db: &DatabaseConnection,
) -> Result<RetentionHoldReapReport, ModuleTransitionServiceError> {
    RetentionHoldStore::reap_expired_holds(db, Utc::now(), MAX_EXPIRED_RETENTION_HOLDS_PER_REAP)
        .await
        .map_err(store_error)
}

fn validate_governed_command(
    command: &GovernedArtifactTransitionCommand,
) -> Result<(), GovernedArtifactTransitionError> {
    let invalid =
        |message: &str| GovernedArtifactTransitionError::InvalidCommand(message.to_string());
    if command.candidate_installation_id.is_nil()
        || command.expected_candidate_revision == 0
        || command.expected_candidate_revision > i64::MAX as u64
        || command.reason.trim().is_empty()
    {
        return Err(invalid(
            "candidate identity, positive expected revision, and a non-empty reason are required",
        ));
    }
    if matches!(
        command.scope,
        ModuleInstallationScope::Tenant { tenant_id } if tenant_id.is_nil()
    ) || !command.scope.matches_command_context(&command.context)
    {
        return Err(invalid(
            "command context does not match the candidate installation scope",
        ));
    }
    command
        .context
        .validate()
        .map_err(|error| invalid(&error.to_string()))?;
    if command.observation_duration.is_zero()
        || command.observation_duration > Duration::from_secs(7 * 24 * 60 * 60)
    {
        return Err(invalid(
            "observation duration must be positive and no longer than seven days",
        ));
    }
    if command.required_pools.is_empty() {
        return Err(invalid(
            "at least one required executor pool must be configured",
        ));
    }
    for (index, pool) in command.required_pools.iter().enumerate() {
        pool.validate()?;
        command.placement_policy.enforce(pool)?;
        for other in &command.required_pools[..index] {
            if other.pool_id == pool.pool_id && other.pool_generation == pool.pool_generation {
                return Err(invalid(
                    "required executor pools must not repeat a pool generation",
                ));
            }
        }
    }
    Ok(())
}

fn readiness_command(
    command: &GovernedArtifactTransitionCommand,
    operation_id: Uuid,
    artifact: &InstalledModuleArtifact,
) -> EvaluateReadinessCommand {
    EvaluateReadinessCommand {
        operation_id,
        target: ReleaseReadinessTarget {
            release_digest: artifact.reference.digest.clone(),
            descriptor: artifact.descriptor.clone(),
            payload_digest: artifact.descriptor.artifact_digest.clone(),
            installed_artifact: artifact.clone(),
        },
        policy: command.placement_policy.clone(),
        context: command.context.clone(),
        actor_can_manage_modules: command.actor_can_manage_modules,
    }
}

/// Evaluates smoke readiness on the composed pool that supports this
/// artifact's payload kind. Mixed deployments compose one readiness
/// service per executor pool (for example an in-process WASM pool and an
/// isolated Rhai pool); an unsupported-kind or wrong-placement result
/// from one pool is a routing fact, while every other failure is
/// terminal for the start.
/// The produced receipt must belong to one of the command's required
/// pools, so a host cannot accidentally smoke on an unlisted pool and
/// then claim required-pool eligibility from stale receipts alone.
async fn evaluate_readiness_on_supporting_pool(
    readiness_services: &[ExecutorReadinessService],
    command: &GovernedArtifactTransitionCommand,
    operation_id: Uuid,
    artifact: &InstalledModuleArtifact,
) -> Result<ExecutorSmokeReceipt, GovernedArtifactTransitionError> {
    let mut last_pool_mismatch: Option<ExecutorReadinessError> = None;
    for service in readiness_services {
        match service
            .evaluate_readiness(readiness_command(command, operation_id, artifact))
            .await
        {
            Ok(receipt) => {
                if !command.required_pools.iter().any(|pool| {
                    pool.pool_id == receipt.pool_id
                        && pool.pool_generation == receipt.pool_generation
                        && pool.fingerprint.compute_digest() == receipt.runtime_fingerprint
                }) {
                    return Err(GovernedArtifactTransitionError::InvalidCommand(
                        "the readiness service pool must be one of the required serving or recovery pools"
                            .to_string(),
                    ));
                }
                return Ok(receipt);
            }
            // Placement policy is enforced before payload-kind support
            // inside `evaluate_readiness`, so a pool that is simply the
            // wrong pool for this artifact can fail with a placement
            // error rather than `UnsupportedPayloadKind`. All three are
            // per-pool routing facts: try the next composed pool. If no
            // pool qualifies, the last mismatch is the truthful error.
            Err(
                error @ (ExecutorReadinessError::UnsupportedPayloadKind(_)
                | ExecutorReadinessError::IsolatedWorkerRequired
                | ExecutorReadinessError::MissingWorkerAttestation),
            ) => {
                last_pool_mismatch = Some(error);
            }
            Err(error) => return Err(error.into()),
        }
    }
    match last_pool_mismatch {
        Some(error) => Err(error.into()),
        None => Err(GovernedArtifactTransitionError::InvalidCommand(format!(
            "no composed executor readiness pool supports payload kind {:?}",
            artifact.descriptor.payload_kind
        ))),
    }
}

/// Canonical digest standing for "no brokered data schema" in preflight
/// receipts (SHA-256 of the empty byte string).
const EMPTY_DATA_SCHEMA_DIGEST: &str =
    "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn artifact_data_schema_digest(descriptor: &crate::ModuleArtifactDescriptor) -> String {
    if let Some(contract) = &descriptor.persistence_contract {
        contract.schema_digest.clone()
    } else if let Some(digest) = &descriptor.data_schema_digest {
        digest.clone()
    } else {
        EMPTY_DATA_SCHEMA_DIGEST.to_string()
    }
}

/// Derives migration/data preflight from the exact admitted descriptors.
///
/// Dynamic artifacts carry no SQL or DDL: brokered-data safety is a
/// property of the declared persistence contract and data schema. An
/// identical contract means the candidate serves the predecessor's
/// namespace unchanged; any contract or data-schema change would switch
/// namespaces or require a cross-revision copy and is therefore
/// maintenance-only. A settings-schema change is likewise not
/// automatically migratable (no guarded settings migration exists), and
/// lifecycle bindings that are not declared idempotency-`Required`
/// cannot prove their enable/disable effects are compensable, so they
/// deny automatic mode as potentially irreversible.
fn derive_governed_preflight(
    operation_id: Uuid,
    candidate: &InstalledModuleArtifact,
    predecessor: Option<&InstalledModuleArtifact>,
) -> Result<MigrationPreflightReceipt, GovernedArtifactTransitionError> {
    let settings_changed = predecessor.is_some_and(|artifact| {
        artifact.descriptor.settings_schema_digest != candidate.descriptor.settings_schema_digest
    });
    let requires_cross_revision_data_copy = predecessor.is_some_and(|artifact| {
        artifact.descriptor.persistence_contract != candidate.descriptor.persistence_contract
            || artifact.descriptor.data_schema_digest != candidate.descriptor.data_schema_digest
    });
    let mut migration_reasons = Vec::new();
    if settings_changed {
        migration_reasons.push(
            "Artifact settings schema changed and no guarded settings migration is available"
                .to_string(),
        );
    }
    if requires_cross_revision_data_copy {
        migration_reasons.push(
            "Artifact persistence contract or data schema changed across revisions".to_string(),
        );
    }
    let has_irreversible_external_effects = candidate.descriptor.bindings.iter().any(|binding| {
        matches!(
            binding.kind,
            ModuleRuntimeBindingKind::PreEnable
                | ModuleRuntimeBindingKind::PostEnable
                | ModuleRuntimeBindingKind::PreDisable
                | ModuleRuntimeBindingKind::PostDisable
                | ModuleRuntimeBindingKind::DataUpgrade
        ) && binding.idempotency != ModuleBindingIdempotency::Required
    });
    let plan = serde_json::json!({
        "contract": "rustok.governed_artifact_migration_plan.v1",
        "module_slug": candidate.release.slug,
        "source": predecessor.map(|artifact| serde_json::json!({
            "installation_id": artifact.installation_id,
            "payload_digest": artifact.descriptor.artifact_digest,
            "manifest_digest": artifact.reference.digest,
            "persistence_contract": artifact.descriptor.persistence_contract,
            "data_schema_digest": artifact.descriptor.data_schema_digest,
            "settings_schema_digest": artifact.descriptor.settings_schema_digest,
        })),
        "target": {
            "installation_id": candidate.installation_id,
            "payload_digest": candidate.descriptor.artifact_digest,
            "manifest_digest": candidate.reference.digest,
            "persistence_contract": candidate.descriptor.persistence_contract,
            "data_schema_digest": candidate.descriptor.data_schema_digest,
            "settings_schema_digest": candidate.descriptor.settings_schema_digest,
        },
    });
    let migration_plan_digest = hash_manifest(&plan)
        .map(|digest| format!("sha256:{digest}"))
        .map_err(|error| GovernedArtifactTransitionError::Store(error.to_string()))?;
    let target_schema_digest = artifact_data_schema_digest(&candidate.descriptor);
    let source_schema_digest = predecessor
        .map(|artifact| artifact_data_schema_digest(&artifact.descriptor))
        .unwrap_or_else(|| target_schema_digest.clone());
    Ok(evaluate_migration_preflight(MigrationPreflightInput {
        operation_id,
        module_slug: candidate.release.slug.clone(),
        source_schema_digest,
        target_schema_digest,
        migration_plan_digest,
        is_additive_safe: !settings_changed && !requires_cross_revision_data_copy,
        migration_reasons,
        settings_guard_installed: !settings_changed,
        has_irreversible_external_effects,
        requires_cross_revision_data_copy,
    }))
}

fn validate_command(
    command: &ModuleTransitionFinalizeCommand,
) -> Result<(), ModuleTransitionServiceError> {
    if command.operation_id.is_nil() || command.expected_revision == 0 {
        return Err(ModuleTransitionServiceError::InvalidCommand(
            "operation identity and expected revision must be positive".to_string(),
        ));
    }
    command
        .context
        .validate()
        .map_err(|error| ModuleTransitionServiceError::InvalidCommand(error.to_string()))
}

fn request_digest(
    command: &ModuleTransitionFinalizeCommand,
) -> Result<String, ModuleTransitionServiceError> {
    crate::idempotency::semantic_request_digest(command)
        .map_err(|error| ModuleTransitionServiceError::Store(error.to_string()))
}

async fn reserve_operation(
    transaction: &DatabaseTransaction,
    command: &ModuleTransitionFinalizeCommand,
    request_digest: &str,
) -> Result<Option<ModuleTransitionFinalizeReceipt>, ModuleTransitionServiceError> {
    let backend =
        SupportedBackend::new(transaction.get_database_backend()).map_err(database_error)?;
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_transition_operations \
                 (idempotency_key, operation_kind, request_digest, actor_id, tenant_id, trace_id, \
                  correlation_id, operation_id, created_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}) \
                 ON CONFLICT (idempotency_key) DO NOTHING",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
                backend.placeholder(6),
                backend.placeholder(7),
                backend.placeholder(8),
                backend.now_sql(),
            ),
            vec![
                sea_orm::Value::Uuid(Some(command.context.idempotency_key)),
                OPERATION_KIND.into(),
                request_digest.to_owned().into(),
                sea_orm::Value::Uuid(Some(command.context.actor_id)),
                command
                    .context
                    .tenant_id
                    .map(|tenant_id| sea_orm::Value::Uuid(Some(tenant_id)))
                    .unwrap_or(sea_orm::Value::Uuid(None)),
                command.context.trace_id.clone().into(),
                sea_orm::Value::Uuid(Some(command.context.correlation_id)),
                sea_orm::Value::Uuid(Some(command.operation_id)),
            ],
        ))
        .await
        .map_err(database_error)?;
    if inserted.rows_affected() == 1 {
        return Ok(None);
    }
    load_operation(transaction, command, request_digest).await
}

async fn load_operation<C: ConnectionTrait>(
    connection: &C,
    command: &ModuleTransitionFinalizeCommand,
    request_digest: &str,
) -> Result<Option<ModuleTransitionFinalizeReceipt>, ModuleTransitionServiceError> {
    let backend =
        SupportedBackend::new(connection.get_database_backend()).map_err(database_error)?;
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT operation_kind, request_digest, actor_id, tenant_id, trace_id, \
                 correlation_id, operation_id, receipt_json \
                 FROM module_transition_operations WHERE idempotency_key = {}",
                backend.placeholder(1),
            ),
            vec![sea_orm::Value::Uuid(Some(command.context.idempotency_key))],
        ))
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    validate_operation_row(&row, backend, command, request_digest)?;
    let receipt_json: Option<String> = row.try_get("", "receipt_json").map_err(database_error)?;
    let Some(receipt_json) = receipt_json else {
        return Err(ModuleTransitionServiceError::OperationInProgress);
    };
    let mut receipt: ModuleTransitionFinalizeReceipt = serde_json::from_str(&receipt_json)
        .map_err(|error| ModuleTransitionServiceError::Store(error.to_string()))?;
    receipt.created = false;
    Ok(Some(receipt))
}

fn validate_operation_row(
    row: &QueryResult,
    backend: SupportedBackend,
    command: &ModuleTransitionFinalizeCommand,
    request_digest: &str,
) -> Result<(), ModuleTransitionServiceError> {
    let stored_tenant = backend
        .optional_uuid_from_row(row, "tenant_id")
        .map_err(database_error)?;
    let matches = row
        .try_get::<String>("", "operation_kind")
        .map_err(database_error)?
        == OPERATION_KIND
        && row
            .try_get::<String>("", "request_digest")
            .map_err(database_error)?
            == request_digest
        && uuid_from_row(row, "actor_id", backend).map_err(database_error)?
            == command.context.actor_id
        && stored_tenant == command.context.tenant_id
        && uuid_from_row(row, "operation_id", backend).map_err(database_error)?
            == command.operation_id;
    if matches {
        Ok(())
    } else {
        Err(ModuleTransitionServiceError::IdempotencyConflict)
    }
}

async fn complete_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    receipt: &ModuleTransitionFinalizeReceipt,
) -> Result<(), ModuleTransitionServiceError> {
    let backend =
        SupportedBackend::new(transaction.get_database_backend()).map_err(database_error)?;
    let receipt_json = serde_json::to_string(receipt)
        .map_err(|error| ModuleTransitionServiceError::Store(error.to_string()))?;
    let result = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "UPDATE module_transition_operations SET resulting_revision = {}, \
                 receipt_json = {}, completed_at = {} \
                 WHERE idempotency_key = {} AND completed_at IS NULL",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.now_sql(),
                backend.placeholder(3),
            ),
            vec![
                i64::try_from(receipt.checkpoint.revision)
                    .map_err(|_| {
                        ModuleTransitionServiceError::Store(
                            "transition revision exceeds database range".to_string(),
                        )
                    })?
                    .into(),
                receipt_json.into(),
                sea_orm::Value::Uuid(Some(idempotency_key)),
            ],
        ))
        .await
        .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(ModuleTransitionServiceError::OperationInProgress);
    }
    Ok(())
}

fn store_error(error: TransitionStoreError) -> ModuleTransitionServiceError {
    match error {
        TransitionStoreError::CheckpointNotFound(operation_id) => {
            ModuleTransitionServiceError::NotFound(operation_id)
        }
        TransitionStoreError::RevisionConflict {
            expected, current, ..
        } => ModuleTransitionServiceError::RevisionConflict { expected, current },
        error => ModuleTransitionServiceError::Store(error.to_string()),
    }
}

fn database_error(error: impl std::fmt::Display) -> ModuleTransitionServiceError {
    ModuleTransitionServiceError::Store(error.to_string())
}
