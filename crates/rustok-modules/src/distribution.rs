//! Immutable static distribution composition and build-intent ownership.
//!
//! Every change replaces the complete reviewed native-module selection and
//! queues a new distribution build. This service never edits the running
//! process, the active runtime composition, or a Cargo manifest.

use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::{
    ControlPlaneInfrastructure, ModuleStaticPromotionError, ModuleStaticPromotionStatus,
    data::{now_expression, placeholder, uuid_from_row, uuid_value},
    promotion::{
        digest_json, load_platform_build_evidence, load_promotion, normalize_native_entry_type,
        valid_cargo_package, valid_cas_source_reference, valid_digest, valid_reference,
        validate_promotion_review,
    },
};

const DISTRIBUTION_STATE_ID: &str = "current";
const MAX_DISTRIBUTION_ITEMS: usize = 256;
const MAX_RUNNER_ID_BYTES: usize = 128;
const MAX_FAILURE_CODE_BYTES: usize = 128;
const MAX_FAILURE_DETAIL_BYTES: usize = 2_000;
const DISTRIBUTION_LEASE_SECONDS: i64 = 300;
const DISTRIBUTION_HEARTBEAT_SECONDS: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStaticDistributionBuildStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl ModuleStaticDistributionBuildStatus {
    fn parse(value: &str) -> Result<Self, ModuleStaticDistributionError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(ModuleStaticDistributionError::Store(
                "static distribution build status is invalid".to_string(),
            )),
        }
    }
}

/// The only executor class admitted into a trusted native distribution.
///
/// This value is persisted and participates in the immutable composition
/// digest so runtime consumers never infer native execution from a package or
/// source layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStaticDistributionExecutorMode {
    StaticNative,
}

impl ModuleStaticDistributionExecutorMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::StaticNative => "static_native",
        }
    }

    fn parse(value: &str) -> Result<Self, ModuleStaticDistributionError> {
        match value {
            "static_native" => Ok(Self::StaticNative),
            _ => Err(ModuleStaticDistributionError::Store(
                "static distribution executor mode is invalid".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionSelection {
    pub promotion_id: Uuid,
    pub expected_promotion_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionBuildCommand {
    pub expected_distribution_revision: u64,
    pub platform_source_reference: String,
    pub platform_source_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub selections: Vec<ModuleStaticDistributionSelection>,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionItem {
    pub promotion_id: Uuid,
    pub promotion_revision: u64,
    pub release_id: String,
    pub module_slug: String,
    pub module_version: String,
    pub cargo_package: String,
    pub entry_type: String,
    pub source_reference: String,
    pub source_digest: String,
    pub dependency_lock_digest: String,
    pub executor_mode: ModuleStaticDistributionExecutorMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionBuild {
    pub distribution_build_id: Uuid,
    pub predecessor_build_id: Option<Uuid>,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub platform_source_reference: String,
    pub platform_source_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub status: ModuleStaticDistributionBuildStatus,
    pub requested_by: Uuid,
    pub attempt_count: u32,
    pub active_claim_id: Option<Uuid>,
    pub claimed_by: Option<String>,
    pub result: Option<ModuleStaticDistributionBuildEvidence>,
    pub failure: Option<ModuleStaticDistributionFailure>,
    pub completion_digest: Option<String>,
    pub items: Vec<ModuleStaticDistributionItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionFailure {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionBuildReceipt {
    pub distribution_build_id: Uuid,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionState {
    pub revision: u64,
    pub current_build_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionClaimCommand {
    pub runner_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionHeartbeatCommand {
    pub claim_id: Uuid,
    pub runner_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionBuildEvidence {
    pub artifact_reference: String,
    pub artifact_digest: String,
    pub sbom_reference: String,
    pub sbom_digest: String,
    pub provenance_reference: String,
    pub provenance_digest: String,
    pub signature_reference: String,
    pub signature_digest: String,
    pub test_evidence_reference: String,
    pub test_evidence_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ModuleStaticDistributionCompletionOutcome {
    Succeeded {
        evidence: ModuleStaticDistributionBuildEvidence,
    },
    Failed {
        failure_code: String,
        failure_detail: String,
    },
    Cancelled {
        failure_code: String,
        failure_detail: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionCompletionCommand {
    pub claim_id: Uuid,
    pub runner_id: String,
    pub distribution_build_id: Uuid,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub outcome: ModuleStaticDistributionCompletionOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionWorkItem {
    pub claim_id: Uuid,
    pub attempt_number: u32,
    pub lease_expires_at: DateTime<Utc>,
    pub build: ModuleStaticDistributionBuild,
}

impl ModuleStaticDistributionWorkItem {
    /// Revalidates the complete immutable claim before it crosses into build
    /// tooling. Transport adapters and executors must call this instead of
    /// trusting deserialized work-item fields.
    pub fn validate(&self) -> Result<(), ModuleStaticDistributionError> {
        let build = &self.build;
        if self.claim_id.is_nil()
            || self.attempt_number == 0
            || build.distribution_build_id.is_nil()
            || build
                .predecessor_build_id
                .is_some_and(|predecessor| predecessor.is_nil())
            || build.composition_revision == 0
            || !valid_digest(&build.composition_digest)
            || !valid_cas_source_reference(
                &build.platform_source_reference,
                &build.platform_source_digest,
            )
            || !valid_digest(&build.toolchain_digest)
            || build.build_target.trim().is_empty()
            || build.build_target.trim() != build.build_target
            || build.build_target.len() > 128
            || build.build_target.chars().any(char::is_control)
            || build.status != ModuleStaticDistributionBuildStatus::Running
            || build.requested_by.is_nil()
            || build.attempt_count != self.attempt_number
            || build.active_claim_id != Some(self.claim_id)
            || build.result.is_some()
            || build.failure.is_some()
            || build.completion_digest.is_some()
            || build.items.len() > MAX_DISTRIBUTION_ITEMS
        {
            return Err(ModuleStaticDistributionError::InvalidCommand);
        }
        validate_runner_id(
            build
                .claimed_by
                .as_deref()
                .ok_or(ModuleStaticDistributionError::InvalidCommand)?,
        )?;
        for item in &build.items {
            if item.promotion_id.is_nil()
                || item.promotion_revision == 0
                || !valid_reference(&item.release_id)
                || !crate::is_valid_static_module_slug(&item.module_slug)
                || Version::parse(&item.module_version).is_err()
                || !valid_cargo_package(&item.cargo_package)
                || normalize_native_entry_type(&item.entry_type).as_deref()
                    != Some(item.entry_type.as_str())
                || !valid_cas_source_reference(&item.source_reference, &item.source_digest)
                || !valid_digest(&item.dependency_lock_digest)
                || item.executor_mode != ModuleStaticDistributionExecutorMode::StaticNative
            {
                return Err(ModuleStaticDistributionError::InvalidCommand);
            }
        }
        if build
            .items
            .windows(2)
            .any(|pair| pair[0].module_slug >= pair[1].module_slug)
        {
            return Err(ModuleStaticDistributionError::InvalidCommand);
        }
        let expected_digest = distribution_composition_digest(
            &build.platform_source_reference,
            &build.platform_source_digest,
            &build.toolchain_digest,
            &build.build_target,
            &build.items,
        )?;
        if build.composition_digest != expected_digest {
            return Err(ModuleStaticDistributionError::InvalidCommand);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionHeartbeatReceipt {
    pub claim_id: Uuid,
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionCompletionReceipt {
    pub distribution_build_id: Uuid,
    pub status: ModuleStaticDistributionBuildStatus,
    pub created: bool,
}

/// Remote execution boundary for one already claimed immutable distribution
/// build. Implementations run outside the control plane and return only a
/// terminal outcome; they cannot mutate owner state directly.
#[async_trait]
pub trait ModuleStaticDistributionExecutor: Send + Sync {
    async fn execute(
        &self,
        work_item: ModuleStaticDistributionWorkItem,
    ) -> Result<ModuleStaticDistributionCompletionOutcome, ModuleStaticDistributionExecutorError>;
}

/// Worker-owned readiness probe used by authenticated transports. Readiness is
/// true only after the deployment-specific CI executor validates its runtime,
/// credentials, and pinned toolchain configuration.
pub trait ModuleStaticDistributionExecutorReadiness: Send + Sync {
    fn is_ready(&self) -> bool;
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleStaticDistributionExecutorError {
    #[error("static distribution executor transport failed: {0}")]
    Transport(String),
    #[error("static distribution executor rejected the immutable work item: {0}")]
    Rejected(String),
}

#[derive(Serialize)]
struct CompositionDigestInput<'a> {
    platform_source_reference: &'a str,
    platform_source_digest: &'a str,
    toolchain_digest: &'a str,
    build_target: &'a str,
    items: &'a [ModuleStaticDistributionItem],
}

pub(crate) fn distribution_composition_digest(
    platform_source_reference: &str,
    platform_source_digest: &str,
    toolchain_digest: &str,
    build_target: &str,
    items: &[ModuleStaticDistributionItem],
) -> Result<String, ModuleStaticDistributionError> {
    digest_json(&CompositionDigestInput {
        platform_source_reference,
        platform_source_digest,
        toolchain_digest,
        build_target,
        items,
    })
    .map_err(promotion_error)
}

#[async_trait]
pub trait ModuleStaticDistributionAuthorizer: Send + Sync {
    async fn authorize_build(
        &self,
        command: &ModuleStaticDistributionBuildCommand,
    ) -> Result<(), ModuleStaticDistributionError>;
}

#[async_trait]
pub trait ModuleStaticDistributionWorkerAuthorizer: Send + Sync {
    async fn authorize_claim(
        &self,
        command: &ModuleStaticDistributionClaimCommand,
    ) -> Result<(), ModuleStaticDistributionError>;

    async fn authorize_heartbeat(
        &self,
        command: &ModuleStaticDistributionHeartbeatCommand,
    ) -> Result<(), ModuleStaticDistributionError>;

    async fn authorize_completion(
        &self,
        command: &ModuleStaticDistributionCompletionCommand,
    ) -> Result<(), ModuleStaticDistributionError>;
}

#[derive(Clone)]
pub struct SeaOrmModuleStaticDistributionService<A> {
    db: DatabaseConnection,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmModuleStaticDistributionService<A>
where
    A: ModuleStaticDistributionAuthorizer,
{
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            infrastructure,
        }
    }

    /// Queues a build for one complete immutable static distribution snapshot.
    /// Empty selection is allowed only after an earlier snapshot exists, which
    /// represents removal of every reviewed native module in a new build.
    pub async fn queue_build(
        &self,
        command: ModuleStaticDistributionBuildCommand,
    ) -> Result<ModuleStaticDistributionBuildReceipt, ModuleStaticDistributionError> {
        validate_command(&command)?;
        self.authorizer.authorize_build(&command).await?;
        let request_digest = digest_json(&command).map_err(promotion_error)?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(receipt) = reserve_operation(
            &transaction,
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return Ok(receipt);
        }

        let (current_revision, predecessor_build_id) =
            lock_distribution_state(&transaction).await?;
        if current_revision != command.expected_distribution_revision {
            return Err(ModuleStaticDistributionError::RevisionConflict {
                expected: command.expected_distribution_revision,
                current: current_revision,
            });
        }
        if current_revision == 0 && command.selections.is_empty() {
            return Err(ModuleStaticDistributionError::EmptyInitialComposition);
        }

        let mut items = Vec::with_capacity(command.selections.len());
        for selection in &command.selections {
            let promotion = load_promotion(&transaction, selection.promotion_id)
                .await
                .map_err(promotion_load_error)?;
            if promotion.status != ModuleStaticPromotionStatus::Approved
                || promotion.revision != selection.expected_promotion_revision
            {
                return Err(ModuleStaticDistributionError::PromotionNotApproved);
            }
            validate_promotion_review(&transaction, &promotion)
                .await
                .map_err(promotion_evidence_error)?;
            let pinned = load_platform_build_evidence(&transaction, &promotion.release_id)
                .await
                .map_err(promotion_evidence_error)?;
            if pinned.publish_request_id != promotion.publish_request_id
                || pinned.module_slug != promotion.module_slug
                || pinned.module_version != promotion.module_version
                || pinned.cargo_package != promotion.cargo_package
                || pinned.entry_type != promotion.entry_type
                || pinned.source_reference != promotion.source_reference
                || pinned.source_digest != promotion.source_digest
                || pinned.dependency_lock_digest != promotion.dependency_lock_digest
            {
                return Err(ModuleStaticDistributionError::PromotionEvidenceChanged);
            }
            items.push(ModuleStaticDistributionItem {
                promotion_id: promotion.promotion_id,
                promotion_revision: promotion.revision,
                release_id: promotion.release_id,
                module_slug: promotion.module_slug,
                module_version: promotion.module_version,
                cargo_package: promotion.cargo_package,
                entry_type: promotion.entry_type,
                source_reference: promotion.source_reference,
                source_digest: promotion.source_digest,
                dependency_lock_digest: promotion.dependency_lock_digest,
                executor_mode: ModuleStaticDistributionExecutorMode::StaticNative,
            });
        }
        items.sort_by(|left, right| {
            left.module_slug
                .cmp(&right.module_slug)
                .then_with(|| left.promotion_id.cmp(&right.promotion_id))
        });
        if items
            .windows(2)
            .any(|pair| pair[0].module_slug == pair[1].module_slug)
        {
            return Err(ModuleStaticDistributionError::DuplicateModuleSelection);
        }
        let composition_digest = distribution_composition_digest(
            &command.platform_source_reference,
            &command.platform_source_digest,
            &command.toolchain_digest,
            &command.build_target,
            &items,
        )?;
        if let Some(predecessor_build_id) = predecessor_build_id {
            let predecessor_digest =
                load_composition_digest(&transaction, predecessor_build_id).await?;
            if predecessor_digest == composition_digest {
                return Err(ModuleStaticDistributionError::NoCompositionChange);
            }
        }
        let composition_revision = current_revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionError::RevisionOverflow)?;
        let distribution_build_id = self.infrastructure.new_id();
        if let Some(predecessor_build_id) = predecessor_build_id {
            crate::distribution_release::cancel_pending_rollback_for_build(
                &transaction,
                predecessor_build_id,
            )
            .await
            .map_err(|error| ModuleStaticDistributionError::Store(error.to_string()))?;
        }
        insert_build(
            &transaction,
            distribution_build_id,
            predecessor_build_id,
            composition_revision,
            &composition_digest,
            &command.platform_source_reference,
            &command.platform_source_digest,
            &command.toolchain_digest,
            &command.build_target,
            command.actor_id,
            &items,
        )
        .await?;
        advance_distribution_state(
            &transaction,
            current_revision,
            composition_revision,
            distribution_build_id,
        )
        .await?;
        complete_operation(
            &transaction,
            command.idempotency_key,
            distribution_build_id,
            composition_revision,
            &composition_digest,
        )
        .await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticDistributionBuildQueued {
                        distribution_build_id,
                        predecessor_build_id,
                        composition_revision,
                        composition_digest: composition_digest.clone(),
                        selected_promotions: u32::try_from(items.len())
                            .map_err(|_| ModuleStaticDistributionError::TooManySelections)?,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticDistributionBuildReceipt {
            distribution_build_id,
            composition_revision,
            composition_digest,
            created: true,
        })
    }

    pub async fn load_build(
        &self,
        distribution_build_id: Uuid,
    ) -> Result<ModuleStaticDistributionBuild, ModuleStaticDistributionError> {
        if distribution_build_id.is_nil() {
            return Err(ModuleStaticDistributionError::InvalidCommand);
        }
        load_build(&self.db, distribution_build_id).await
    }

    pub async fn current_state(
        &self,
    ) -> Result<ModuleStaticDistributionState, ModuleStaticDistributionError> {
        load_distribution_state(&self.db, false).await
    }
}

#[derive(Clone)]
pub struct SeaOrmModuleStaticDistributionWorkerService<A> {
    db: DatabaseConnection,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmModuleStaticDistributionWorkerService<A>
where
    A: ModuleStaticDistributionWorkerAuthorizer,
{
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            infrastructure,
        }
    }

    pub async fn claim_next(
        &self,
        command: ModuleStaticDistributionClaimCommand,
    ) -> Result<Option<ModuleStaticDistributionWorkItem>, ModuleStaticDistributionError> {
        validate_runner_id(&command.runner_id)?;
        self.authorizer.authorize_claim(&command).await?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        let Some(candidate) = lock_next_build_candidate(&transaction).await? else {
            transaction.commit().await.map_err(store_error)?;
            return Ok(None);
        };
        let now = self.infrastructure.now();
        let lease_expires_at = now
            .checked_add_signed(Duration::seconds(DISTRIBUTION_LEASE_SECONDS))
            .ok_or(ModuleStaticDistributionError::LeaseOverflow)?;
        if candidate.status == ModuleStaticDistributionBuildStatus::Running {
            expire_active_attempt(
                &transaction,
                candidate
                    .active_claim_id
                    .ok_or(ModuleStaticDistributionError::ClaimConflict)?,
            )
            .await?;
        }
        let attempt_number = candidate
            .attempt_count
            .checked_add(1)
            .ok_or(ModuleStaticDistributionError::AttemptOverflow)?;
        let claim_id = self.infrastructure.new_id();
        claim_build(
            &transaction,
            &candidate,
            claim_id,
            attempt_number,
            &command.runner_id,
            &lease_expires_at,
        )
        .await?;
        let build = load_build(&transaction, candidate.distribution_build_id).await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    None,
                    DomainEvent::ModuleStaticDistributionBuildClaimed {
                        distribution_build_id: candidate.distribution_build_id,
                        claim_id,
                        attempt_number,
                        runner_id: command.runner_id,
                        reclaimed_expired_lease: candidate.status
                            == ModuleStaticDistributionBuildStatus::Running,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(Some(ModuleStaticDistributionWorkItem {
            claim_id,
            attempt_number,
            lease_expires_at,
            build,
        }))
    }

    pub async fn heartbeat(
        &self,
        command: ModuleStaticDistributionHeartbeatCommand,
    ) -> Result<ModuleStaticDistributionHeartbeatReceipt, ModuleStaticDistributionError> {
        validate_heartbeat_command(&command)?;
        self.authorizer.authorize_heartbeat(&command).await?;
        let lease_expires_at = self
            .infrastructure
            .now()
            .checked_add_signed(Duration::seconds(DISTRIBUTION_LEASE_SECONDS))
            .ok_or(ModuleStaticDistributionError::LeaseOverflow)?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        heartbeat_claim(&transaction, &command, &lease_expires_at).await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticDistributionHeartbeatReceipt {
            claim_id: command.claim_id,
            lease_expires_at,
        })
    }

    pub async fn complete(
        &self,
        command: ModuleStaticDistributionCompletionCommand,
    ) -> Result<ModuleStaticDistributionCompletionReceipt, ModuleStaticDistributionError> {
        validate_completion_command(&command)?;
        self.authorizer.authorize_completion(&command).await?;
        let completion_digest = digest_json(&command).map_err(promotion_error)?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        let build = lock_claimed_build(&transaction, command.distribution_build_id).await?;
        if build.active_claim_id != Some(command.claim_id)
            || build.claimed_by.as_deref() != Some(command.runner_id.as_str())
            || build.composition_revision != command.composition_revision
            || build.composition_digest != command.composition_digest
        {
            return Err(ModuleStaticDistributionError::ClaimConflict);
        }
        let attempt = lock_attempt(&transaction, command.claim_id).await?;
        if attempt.runner_id != command.runner_id
            || attempt.distribution_build_id != command.distribution_build_id
        {
            return Err(ModuleStaticDistributionError::ClaimConflict);
        }
        if attempt.status != "running" {
            if matches!(
                attempt.status.as_str(),
                "succeeded" | "failed" | "cancelled"
            ) && attempt.completion_digest.as_deref() == Some(completion_digest.as_str())
            {
                let build = load_build(&transaction, command.distribution_build_id).await?;
                transaction.commit().await.map_err(store_error)?;
                return Ok(ModuleStaticDistributionCompletionReceipt {
                    distribution_build_id: command.distribution_build_id,
                    status: build.status,
                    created: false,
                });
            }
            return Err(if attempt.status == "lease_expired" {
                ModuleStaticDistributionError::LeaseExpired
            } else {
                ModuleStaticDistributionError::CompletionConflict
            });
        }
        let terminal = TerminalBuildFields::from_outcome(&command.outcome);
        complete_claim(&transaction, &command, &completion_digest, &terminal).await?;
        if terminal.status != "succeeded" {
            crate::distribution_release::cancel_pending_rollback_for_build(
                &transaction,
                command.distribution_build_id,
            )
            .await
            .map_err(|error| ModuleStaticDistributionError::Store(error.to_string()))?;
        }
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    None,
                    DomainEvent::ModuleStaticDistributionBuildCompleted {
                        distribution_build_id: command.distribution_build_id,
                        claim_id: command.claim_id,
                        composition_revision: command.composition_revision,
                        composition_digest: command.composition_digest,
                        outcome: terminal.status.to_string(),
                        result_digest: terminal.result_digest.clone(),
                        completion_digest: completion_digest.clone(),
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticDistributionCompletionReceipt {
            distribution_build_id: command.distribution_build_id,
            status: ModuleStaticDistributionBuildStatus::parse(terminal.status)?,
            created: true,
        })
    }

    /// Claims one queued build, delegates it through the external executor
    /// port, renews the owner lease while execution is in flight, and persists
    /// the returned terminal outcome. An executor/transport failure is not
    /// converted into a build failure; the lease expires so another runner can
    /// reclaim the immutable build.
    pub async fn dispatch_next<E>(
        &self,
        command: ModuleStaticDistributionClaimCommand,
        executor: &E,
    ) -> Result<Option<ModuleStaticDistributionCompletionReceipt>, ModuleStaticDistributionError>
    where
        E: ModuleStaticDistributionExecutor + ?Sized,
    {
        let runner_id = command.runner_id.clone();
        let Some(work_item) = self.claim_next(command).await? else {
            return Ok(None);
        };
        let claim_id = work_item.claim_id;
        let distribution_build_id = work_item.build.distribution_build_id;
        let composition_revision = work_item.build.composition_revision;
        let composition_digest = work_item.build.composition_digest.clone();
        let mut execution = Box::pin(executor.execute(work_item));

        let outcome = loop {
            tokio::select! {
                biased;
                result = &mut execution => break result?,
                _ = tokio::time::sleep(std::time::Duration::from_secs(
                    DISTRIBUTION_HEARTBEAT_SECONDS,
                )) => {
                    self.heartbeat(ModuleStaticDistributionHeartbeatCommand {
                        claim_id,
                        runner_id: runner_id.clone(),
                    }).await?;
                }
            }
        };

        self.complete(ModuleStaticDistributionCompletionCommand {
            claim_id,
            runner_id,
            distribution_build_id,
            composition_revision,
            composition_digest,
            outcome,
        })
        .await
        .map(Some)
    }
}

#[derive(Debug, Error)]
pub enum ModuleStaticDistributionError {
    #[error("static distribution command is invalid")]
    InvalidCommand,
    #[error("static distribution command was not authorized")]
    AuthorizationDenied,
    #[error("static distribution selection exceeds the bounded maximum")]
    TooManySelections,
    #[error("the initial static distribution cannot be empty")]
    EmptyInitialComposition,
    #[error("one module slug was selected more than once")]
    DuplicateModuleSelection,
    #[error("selected promotion is not approved at the expected revision")]
    PromotionNotApproved,
    #[error("selected promotion no longer matches its release/build evidence")]
    PromotionEvidenceChanged,
    #[error("static distribution composition is unchanged")]
    NoCompositionChange,
    #[error("static distribution revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: u64, current: u64 },
    #[error("static distribution revision overflowed")]
    RevisionOverflow,
    #[error("static distribution build was not found")]
    BuildNotFound,
    #[error("static distribution idempotency key was reused for another command")]
    IdempotencyConflict,
    #[error("static distribution worker identity is invalid")]
    InvalidRunner,
    #[error("static distribution build claim was not found")]
    ClaimNotFound,
    #[error("static distribution build claim ownership or identity conflict")]
    ClaimConflict,
    #[error("static distribution build claim lease expired")]
    LeaseExpired,
    #[error("static distribution build completion conflicts with the persisted result")]
    CompletionConflict,
    #[error("static distribution build attempt counter overflowed")]
    AttemptOverflow,
    #[error("static distribution build lease timestamp overflowed")]
    LeaseOverflow,
    #[error(transparent)]
    Executor(#[from] ModuleStaticDistributionExecutorError),
    #[error("static distribution store error: {0}")]
    Store(String),
}

struct BuildCandidate {
    distribution_build_id: Uuid,
    status: ModuleStaticDistributionBuildStatus,
    active_claim_id: Option<Uuid>,
    attempt_count: u32,
}

struct AttemptRecord {
    distribution_build_id: Uuid,
    runner_id: String,
    status: String,
    completion_digest: Option<String>,
}

struct ClaimedBuildRecord {
    active_claim_id: Option<Uuid>,
    claimed_by: Option<String>,
    composition_revision: u64,
    composition_digest: String,
}

struct TerminalBuildFields {
    status: &'static str,
    result_reference: Option<String>,
    result_digest: Option<String>,
    sbom_reference: Option<String>,
    sbom_digest: Option<String>,
    provenance_reference: Option<String>,
    provenance_digest: Option<String>,
    signature_reference: Option<String>,
    signature_digest: Option<String>,
    test_evidence_reference: Option<String>,
    test_evidence_digest: Option<String>,
    failure_code: Option<String>,
    failure_detail: Option<String>,
}

impl TerminalBuildFields {
    fn from_outcome(outcome: &ModuleStaticDistributionCompletionOutcome) -> Self {
        match outcome {
            ModuleStaticDistributionCompletionOutcome::Succeeded { evidence } => Self {
                status: "succeeded",
                result_reference: Some(evidence.artifact_reference.clone()),
                result_digest: Some(evidence.artifact_digest.clone()),
                sbom_reference: Some(evidence.sbom_reference.clone()),
                sbom_digest: Some(evidence.sbom_digest.clone()),
                provenance_reference: Some(evidence.provenance_reference.clone()),
                provenance_digest: Some(evidence.provenance_digest.clone()),
                signature_reference: Some(evidence.signature_reference.clone()),
                signature_digest: Some(evidence.signature_digest.clone()),
                test_evidence_reference: Some(evidence.test_evidence_reference.clone()),
                test_evidence_digest: Some(evidence.test_evidence_digest.clone()),
                failure_code: None,
                failure_detail: None,
            },
            ModuleStaticDistributionCompletionOutcome::Failed {
                failure_code,
                failure_detail,
            } => Self {
                status: "failed",
                result_reference: None,
                result_digest: None,
                sbom_reference: None,
                sbom_digest: None,
                provenance_reference: None,
                provenance_digest: None,
                signature_reference: None,
                signature_digest: None,
                test_evidence_reference: None,
                test_evidence_digest: None,
                failure_code: Some(failure_code.clone()),
                failure_detail: Some(failure_detail.clone()),
            },
            ModuleStaticDistributionCompletionOutcome::Cancelled {
                failure_code,
                failure_detail,
            } => Self {
                status: "cancelled",
                result_reference: None,
                result_digest: None,
                sbom_reference: None,
                sbom_digest: None,
                provenance_reference: None,
                provenance_digest: None,
                signature_reference: None,
                signature_digest: None,
                test_evidence_reference: None,
                test_evidence_digest: None,
                failure_code: Some(failure_code.clone()),
                failure_detail: Some(failure_detail.clone()),
            },
        }
    }
}

async fn lock_next_build_candidate(
    transaction: &DatabaseTransaction,
) -> Result<Option<BuildCandidate>, ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let lock = if backend == DbBackend::Postgres {
        " FOR UPDATE SKIP LOCKED"
    } else {
        ""
    };
    let row = transaction
        .query_one(Statement::from_string(
            backend,
            format!(
                "SELECT distribution_build_id, status, active_claim_id, attempt_count
                 FROM module_static_distribution_builds
                 WHERE status = 'queued'
                    OR (status = 'running' AND lease_expires_at < {})
                 ORDER BY composition_revision, requested_at, distribution_build_id
                 LIMIT 1{lock}",
                now_expression(backend),
            ),
        ))
        .await
        .map_err(store_error)?;
    row.map(|row| {
        let attempt_count: i64 = row.try_get("", "attempt_count").map_err(store_error)?;
        Ok(BuildCandidate {
            distribution_build_id: uuid_from_row(&row, "distribution_build_id", backend)
                .map_err(store_error)?,
            status: ModuleStaticDistributionBuildStatus::parse(
                &row.try_get::<String>("", "status").map_err(store_error)?,
            )?,
            active_claim_id: optional_uuid_from_row(&row, "active_claim_id", backend)?,
            attempt_count: u32::try_from(attempt_count)
                .map_err(|_| ModuleStaticDistributionError::AttemptOverflow)?,
        })
    })
    .transpose()
}

async fn expire_active_attempt(
    transaction: &DatabaseTransaction,
    claim_id: Uuid,
) -> Result<(), ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_attempts
                 SET status = 'lease_expired', completed_at = {}
                 WHERE claim_id = {} AND status = 'running' AND lease_expires_at < {}",
                now_expression(backend),
                placeholder(backend, 1),
                now_expression(backend),
            ),
            vec![uuid_value(claim_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionError::ClaimConflict);
    }
    Ok(())
}

async fn claim_build(
    transaction: &DatabaseTransaction,
    candidate: &BuildCandidate,
    claim_id: Uuid,
    attempt_number: u32,
    runner_id: &str,
    lease_expires_at: &DateTime<Utc>,
) -> Result<(), ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let candidate_guard = match candidate.status {
        ModuleStaticDistributionBuildStatus::Queued => {
            "status = 'queued' AND active_claim_id IS NULL".to_string()
        }
        ModuleStaticDistributionBuildStatus::Running => format!(
            "status = 'running' AND active_claim_id = {} AND lease_expires_at < {}",
            placeholder(backend, 6),
            now_expression(backend),
        ),
        _ => return Err(ModuleStaticDistributionError::ClaimConflict),
    };
    let mut values = vec![
        uuid_value(claim_id, backend),
        runner_id.to_owned().into(),
        lease_expires_at.to_owned().into(),
        i64::from(attempt_number).into(),
        uuid_value(candidate.distribution_build_id, backend),
    ];
    if let Some(active_claim_id) = candidate.active_claim_id {
        values.push(uuid_value(active_claim_id, backend));
    }
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_builds
                 SET status = 'running', active_claim_id = {}, claimed_by = {},
                     lease_expires_at = {}, last_heartbeat_at = {}, attempt_count = {},
                     started_at = COALESCE(started_at, {})
                 WHERE distribution_build_id = {} AND {candidate_guard}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
                placeholder(backend, 4),
                now_expression(backend),
                placeholder(backend, 5),
            ),
            values,
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionError::ClaimConflict);
    }
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_attempts
                 (claim_id, distribution_build_id, attempt_number, runner_id, status,
                  lease_expires_at, last_heartbeat_at, started_at)
                 VALUES ({}, {}, {}, {}, 'running', {}, {}, {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                now_expression(backend),
                now_expression(backend),
            ),
            vec![
                uuid_value(claim_id, backend),
                uuid_value(candidate.distribution_build_id, backend),
                i64::from(attempt_number).into(),
                runner_id.to_owned().into(),
                lease_expires_at.to_owned().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn heartbeat_claim(
    transaction: &DatabaseTransaction,
    command: &ModuleStaticDistributionHeartbeatCommand,
    lease_expires_at: &DateTime<Utc>,
) -> Result<(), ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let updated_build = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_builds
                 SET lease_expires_at = {}, last_heartbeat_at = {}
                 WHERE active_claim_id = {} AND claimed_by = {} AND status = 'running'
                   AND lease_expires_at >= {}",
                placeholder(backend, 1),
                now_expression(backend),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
            ),
            vec![
                lease_expires_at.to_owned().into(),
                uuid_value(command.claim_id, backend),
                command.runner_id.clone().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated_build.rows_affected() != 1 {
        return Err(
            classify_claim_failure(transaction, command.claim_id, &command.runner_id).await?,
        );
    }
    let updated_attempt = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_attempts
                 SET lease_expires_at = {}, last_heartbeat_at = {}
                 WHERE claim_id = {} AND runner_id = {} AND status = 'running'
                   AND lease_expires_at >= {}",
                placeholder(backend, 1),
                now_expression(backend),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
            ),
            vec![
                lease_expires_at.to_owned().into(),
                uuid_value(command.claim_id, backend),
                command.runner_id.clone().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated_attempt.rows_affected() != 1 {
        return Err(ModuleStaticDistributionError::ClaimConflict);
    }
    Ok(())
}

async fn classify_claim_failure(
    connection: &impl ConnectionTrait,
    claim_id: Uuid,
    runner_id: &str,
) -> Result<ModuleStaticDistributionError, ModuleStaticDistributionError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT runner_id, status FROM module_static_distribution_attempts WHERE claim_id = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(claim_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionError::ClaimNotFound)?;
    let stored_runner: String = row.try_get("", "runner_id").map_err(store_error)?;
    let status: String = row.try_get("", "status").map_err(store_error)?;
    Ok(if stored_runner != runner_id {
        ModuleStaticDistributionError::ClaimConflict
    } else if status == "running" || status == "lease_expired" {
        ModuleStaticDistributionError::LeaseExpired
    } else {
        ModuleStaticDistributionError::CompletionConflict
    })
}

async fn lock_attempt(
    transaction: &DatabaseTransaction,
    claim_id: Uuid,
) -> Result<AttemptRecord, ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let lock = if backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT distribution_build_id, runner_id, status, completion_digest
                 FROM module_static_distribution_attempts WHERE claim_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(claim_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionError::ClaimNotFound)?;
    Ok(AttemptRecord {
        distribution_build_id: uuid_from_row(&row, "distribution_build_id", backend)
            .map_err(store_error)?,
        runner_id: row.try_get("", "runner_id").map_err(store_error)?,
        status: row.try_get("", "status").map_err(store_error)?,
        completion_digest: row.try_get("", "completion_digest").map_err(store_error)?,
    })
}

async fn lock_claimed_build(
    transaction: &DatabaseTransaction,
    distribution_build_id: Uuid,
) -> Result<ClaimedBuildRecord, ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let lock = if backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT active_claim_id, claimed_by, composition_revision, composition_digest
                 FROM module_static_distribution_builds WHERE distribution_build_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_build_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionError::BuildNotFound)?;
    Ok(ClaimedBuildRecord {
        active_claim_id: optional_uuid_from_row(&row, "active_claim_id", backend)?,
        claimed_by: row.try_get("", "claimed_by").map_err(store_error)?,
        composition_revision: positive_revision(&row, "composition_revision")?,
        composition_digest: row.try_get("", "composition_digest").map_err(store_error)?,
    })
}

async fn complete_claim(
    transaction: &DatabaseTransaction,
    command: &ModuleStaticDistributionCompletionCommand,
    completion_digest: &str,
    terminal: &TerminalBuildFields,
) -> Result<(), ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let terminal_values = || -> Vec<sea_orm::Value> {
        vec![
            terminal.status.into(),
            terminal.result_reference.clone().into(),
            terminal.result_digest.clone().into(),
            terminal.sbom_reference.clone().into(),
            terminal.sbom_digest.clone().into(),
            terminal.provenance_reference.clone().into(),
            terminal.provenance_digest.clone().into(),
            terminal.signature_reference.clone().into(),
            terminal.signature_digest.clone().into(),
            terminal.test_evidence_reference.clone().into(),
            terminal.test_evidence_digest.clone().into(),
            terminal.failure_code.clone().into(),
            terminal.failure_detail.clone().into(),
            completion_digest.to_owned().into(),
        ]
    };
    let mut attempt_values = terminal_values();
    attempt_values.extend([
        uuid_value(command.claim_id, backend),
        command.runner_id.clone().into(),
    ]);
    let updated_attempt = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_attempts
                 SET status = {}, result_reference = {}, result_digest = {},
                     sbom_reference = {}, sbom_digest = {}, provenance_reference = {},
                     provenance_digest = {}, signature_reference = {}, signature_digest = {},
                     test_evidence_reference = {}, test_evidence_digest = {}, failure_code = {}, failure_detail = {},
                     completion_digest = {}, completed_at = {}
                 WHERE claim_id = {} AND runner_id = {} AND status = 'running'
                   AND lease_expires_at >= {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                placeholder(backend, 9),
                placeholder(backend, 10),
                placeholder(backend, 11),
                placeholder(backend, 12),
                placeholder(backend, 13),
                placeholder(backend, 14),
                now_expression(backend),
                placeholder(backend, 15),
                placeholder(backend, 16),
                now_expression(backend),
            ),
            attempt_values,
        ))
        .await
        .map_err(store_error)?;
    if updated_attempt.rows_affected() != 1 {
        return Err(ModuleStaticDistributionError::LeaseExpired);
    }
    let mut build_values = terminal_values();
    build_values.extend([
        uuid_value(command.distribution_build_id, backend),
        uuid_value(command.claim_id, backend),
        command.runner_id.clone().into(),
    ]);
    let updated_build = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_builds
                 SET status = {}, result_reference = {}, result_digest = {},
                     sbom_reference = {}, sbom_digest = {}, provenance_reference = {},
                     provenance_digest = {}, signature_reference = {}, signature_digest = {},
                     test_evidence_reference = {}, test_evidence_digest = {}, failure_code = {}, failure_detail = {},
                     completion_digest = {}, lease_expires_at = NULL, completed_at = {}
                 WHERE distribution_build_id = {} AND active_claim_id = {} AND claimed_by = {}
                   AND status = 'running' AND lease_expires_at >= {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                placeholder(backend, 9),
                placeholder(backend, 10),
                placeholder(backend, 11),
                placeholder(backend, 12),
                placeholder(backend, 13),
                placeholder(backend, 14),
                now_expression(backend),
                placeholder(backend, 15),
                placeholder(backend, 16),
                placeholder(backend, 17),
                now_expression(backend),
            ),
            build_values,
        ))
        .await
        .map_err(store_error)?;
    if updated_build.rows_affected() != 1 {
        return Err(ModuleStaticDistributionError::ClaimConflict);
    }
    Ok(())
}

pub(crate) async fn insert_build(
    transaction: &DatabaseTransaction,
    distribution_build_id: Uuid,
    predecessor_build_id: Option<Uuid>,
    composition_revision: u64,
    composition_digest: &str,
    platform_source_reference: &str,
    platform_source_digest: &str,
    toolchain_digest: &str,
    build_target: &str,
    actor_id: Uuid,
    items: &[ModuleStaticDistributionItem],
) -> Result<(), ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_builds
                 (distribution_build_id, predecessor_build_id, composition_revision,
                  composition_digest, platform_source_reference, platform_source_digest,
                  toolchain_digest, build_target, status, requested_by, requested_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, 'queued', {}, {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                placeholder(backend, 9),
                now_expression(backend),
            ),
            vec![
                uuid_value(distribution_build_id, backend),
                optional_uuid_value(predecessor_build_id, backend),
                revision_value(composition_revision)?,
                composition_digest.to_owned().into(),
                platform_source_reference.to_owned().into(),
                platform_source_digest.to_owned().into(),
                toolchain_digest.to_owned().into(),
                build_target.to_owned().into(),
                uuid_value(actor_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    for (ordinal, item) in items.iter().enumerate() {
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_static_distribution_items
                     (distribution_build_id, ordinal, promotion_id, promotion_revision,
                      release_id, module_slug, module_version, cargo_package, entry_type,
                      source_reference, source_digest, dependency_lock_digest, executor_mode)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    placeholder(backend, 10),
                    placeholder(backend, 11),
                    placeholder(backend, 12),
                    placeholder(backend, 13),
                ),
                vec![
                    uuid_value(distribution_build_id, backend),
                    i64::try_from(ordinal)
                        .map_err(|_| ModuleStaticDistributionError::TooManySelections)?
                        .into(),
                    uuid_value(item.promotion_id, backend),
                    revision_value(item.promotion_revision)?,
                    item.release_id.clone().into(),
                    item.module_slug.clone().into(),
                    item.module_version.clone().into(),
                    item.cargo_package.clone().into(),
                    item.entry_type.clone().into(),
                    item.source_reference.clone().into(),
                    item.source_digest.clone().into(),
                    item.dependency_lock_digest.clone().into(),
                    item.executor_mode.as_str().into(),
                ],
            ))
            .await
            .map_err(store_error)?;
    }
    Ok(())
}

async fn lock_distribution_state(
    transaction: &DatabaseTransaction,
) -> Result<(u64, Option<Uuid>), ModuleStaticDistributionError> {
    let state = load_distribution_state(transaction, true).await?;
    Ok((state.revision, state.current_build_id))
}

pub(crate) async fn load_distribution_state<C: ConnectionTrait>(
    connection: &C,
    lock_row: bool,
) -> Result<ModuleStaticDistributionState, ModuleStaticDistributionError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT revision, current_build_id FROM module_static_distribution_state
                 WHERE state_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![DISTRIBUTION_STATE_ID.into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or_else(|| {
            ModuleStaticDistributionError::Store(
                "static distribution state is unavailable".to_string(),
            )
        })?;
    Ok(ModuleStaticDistributionState {
        revision: positive_or_zero_revision(&row, "revision")?,
        current_build_id: optional_uuid_from_row(&row, "current_build_id", backend)?,
    })
}

pub(crate) async fn advance_distribution_state(
    transaction: &DatabaseTransaction,
    expected_revision: u64,
    next_revision: u64,
    distribution_build_id: Uuid,
) -> Result<(), ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_state
                 SET revision = {}, current_build_id = {}, updated_at = {}
                 WHERE state_id = {} AND revision = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                now_expression(backend),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            vec![
                revision_value(next_revision)?,
                uuid_value(distribution_build_id, backend),
                DISTRIBUTION_STATE_ID.into(),
                revision_value_allow_zero(expected_revision)?,
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        let current = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT revision FROM module_static_distribution_state WHERE state_id = {}",
                    placeholder(backend, 1),
                ),
                vec![DISTRIBUTION_STATE_ID.into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or_else(|| {
                ModuleStaticDistributionError::Store(
                    "static distribution state is unavailable".to_string(),
                )
            })?;
        return Err(ModuleStaticDistributionError::RevisionConflict {
            expected: expected_revision,
            current: positive_or_zero_revision(&current, "revision")?,
        });
    }
    Ok(())
}

async fn load_composition_digest<C: ConnectionTrait>(
    connection: &C,
    distribution_build_id: Uuid,
) -> Result<String, ModuleStaticDistributionError> {
    let backend = connection.get_database_backend();
    connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT composition_digest FROM module_static_distribution_builds
                 WHERE distribution_build_id = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_build_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionError::BuildNotFound)?
        .try_get("", "composition_digest")
        .map_err(store_error)
}

async fn reserve_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionBuildReceipt>, ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_operations
                 (idempotency_key, request_digest, actor_id, created_at)
                 VALUES ({}, {}, {}, {}) ON CONFLICT (idempotency_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
            ),
            vec![
                uuid_value(idempotency_key, backend),
                request_digest.to_owned().into(),
                uuid_value(actor_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if inserted.rows_affected() == 1 {
        return Ok(None);
    }
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_digest, actor_id, distribution_build_id,
                        composition_revision, composition_digest,
                        CASE WHEN completed_at IS NULL THEN 0 ELSE 1 END AS completed
                 FROM module_static_distribution_operations WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionError::IdempotencyConflict)?;
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
    if stored_digest != request_digest || stored_actor != actor_id {
        return Err(ModuleStaticDistributionError::IdempotencyConflict);
    }
    replay_receipt(&row, backend).map(Some)
}

async fn complete_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    distribution_build_id: Uuid,
    composition_revision: u64,
    composition_digest: &str,
) -> Result<(), ModuleStaticDistributionError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_operations
                 SET distribution_build_id = {}, composition_revision = {},
                     composition_digest = {}, completed_at = {}
                 WHERE idempotency_key = {} AND distribution_build_id IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
                placeholder(backend, 4),
            ),
            vec![
                uuid_value(distribution_build_id, backend),
                revision_value(composition_revision)?,
                composition_digest.to_owned().into(),
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionError::IdempotencyConflict);
    }
    Ok(())
}

fn replay_receipt(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ModuleStaticDistributionBuildReceipt, ModuleStaticDistributionError> {
    let distribution_build_id = optional_uuid_from_row(row, "distribution_build_id", backend)?
        .ok_or(ModuleStaticDistributionError::IdempotencyConflict)?;
    let composition_revision = optional_positive_revision(row, "composition_revision")?
        .ok_or(ModuleStaticDistributionError::IdempotencyConflict)?;
    let composition_digest: Option<String> =
        row.try_get("", "composition_digest").map_err(store_error)?;
    let completed: i64 = row.try_get("", "completed").map_err(store_error)?;
    if completed != 1 {
        return Err(ModuleStaticDistributionError::IdempotencyConflict);
    }
    Ok(ModuleStaticDistributionBuildReceipt {
        distribution_build_id,
        composition_revision,
        composition_digest: composition_digest
            .ok_or(ModuleStaticDistributionError::IdempotencyConflict)?,
        created: false,
    })
}

pub(crate) async fn load_build<C: ConnectionTrait>(
    connection: &C,
    distribution_build_id: Uuid,
) -> Result<ModuleStaticDistributionBuild, ModuleStaticDistributionError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT distribution_build_id, predecessor_build_id, composition_revision,
                        composition_digest, platform_source_reference, platform_source_digest,
                        toolchain_digest, build_target, status, requested_by, attempt_count,
                        active_claim_id, claimed_by, result_reference, result_digest,
                        sbom_reference, sbom_digest, provenance_reference, provenance_digest,
                        signature_reference, signature_digest, test_evidence_reference,
                        test_evidence_digest, failure_code, failure_detail, completion_digest
                 FROM module_static_distribution_builds WHERE distribution_build_id = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_build_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionError::BuildNotFound)?;
    let item_rows = connection
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT promotion_id, promotion_revision, release_id, module_slug,
                        module_version, cargo_package, entry_type, source_reference, source_digest,
                        dependency_lock_digest, executor_mode
                 FROM module_static_distribution_items WHERE distribution_build_id = {}
                 ORDER BY ordinal",
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_build_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    let items = item_rows
        .iter()
        .map(|item| item_from_row(item, backend))
        .collect::<Result<Vec<_>, _>>()?;
    let result_reference: Option<String> =
        row.try_get("", "result_reference").map_err(store_error)?;
    let result_digest: Option<String> = row.try_get("", "result_digest").map_err(store_error)?;
    let result = match (result_reference, result_digest) {
        (Some(artifact_reference), Some(artifact_digest)) => {
            Some(ModuleStaticDistributionBuildEvidence {
                artifact_reference,
                artifact_digest,
                sbom_reference: required_optional_string(&row, "sbom_reference")?,
                sbom_digest: required_optional_string(&row, "sbom_digest")?,
                provenance_reference: required_optional_string(&row, "provenance_reference")?,
                provenance_digest: required_optional_string(&row, "provenance_digest")?,
                signature_reference: required_optional_string(&row, "signature_reference")?,
                signature_digest: required_optional_string(&row, "signature_digest")?,
                test_evidence_reference: required_optional_string(&row, "test_evidence_reference")?,
                test_evidence_digest: required_optional_string(&row, "test_evidence_digest")?,
            })
        }
        (None, None) => None,
        _ => {
            return Err(ModuleStaticDistributionError::Store(
                "static distribution result identity is incomplete".to_string(),
            ));
        }
    };
    let failure_code: Option<String> = row.try_get("", "failure_code").map_err(store_error)?;
    let failure_detail: Option<String> = row.try_get("", "failure_detail").map_err(store_error)?;
    let failure = match (failure_code, failure_detail) {
        (Some(code), Some(detail)) => Some(ModuleStaticDistributionFailure { code, detail }),
        (None, None) => None,
        _ => {
            return Err(ModuleStaticDistributionError::Store(
                "static distribution failure identity is incomplete".to_string(),
            ));
        }
    };
    let attempt_count: i64 = row.try_get("", "attempt_count").map_err(store_error)?;
    Ok(ModuleStaticDistributionBuild {
        distribution_build_id: uuid_from_row(&row, "distribution_build_id", backend)
            .map_err(store_error)?,
        predecessor_build_id: optional_uuid_from_row(&row, "predecessor_build_id", backend)?,
        composition_revision: positive_revision(&row, "composition_revision")?,
        composition_digest: row.try_get("", "composition_digest").map_err(store_error)?,
        platform_source_reference: row
            .try_get("", "platform_source_reference")
            .map_err(store_error)?,
        platform_source_digest: row
            .try_get("", "platform_source_digest")
            .map_err(store_error)?,
        toolchain_digest: row.try_get("", "toolchain_digest").map_err(store_error)?,
        build_target: row.try_get("", "build_target").map_err(store_error)?,
        status: ModuleStaticDistributionBuildStatus::parse(
            &row.try_get::<String>("", "status").map_err(store_error)?,
        )?,
        requested_by: uuid_from_row(&row, "requested_by", backend).map_err(store_error)?,
        attempt_count: u32::try_from(attempt_count)
            .map_err(|_| ModuleStaticDistributionError::AttemptOverflow)?,
        active_claim_id: optional_uuid_from_row(&row, "active_claim_id", backend)?,
        claimed_by: row.try_get("", "claimed_by").map_err(store_error)?,
        result,
        failure,
        completion_digest: row.try_get("", "completion_digest").map_err(store_error)?,
        items,
    })
}

fn required_optional_string(
    row: &QueryResult,
    column: &str,
) -> Result<String, ModuleStaticDistributionError> {
    row.try_get::<Option<String>>("", column)
        .map_err(store_error)?
        .ok_or_else(|| {
            ModuleStaticDistributionError::Store(format!(
                "static distribution result column `{column}` is missing"
            ))
        })
}

fn item_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ModuleStaticDistributionItem, ModuleStaticDistributionError> {
    Ok(ModuleStaticDistributionItem {
        promotion_id: uuid_from_row(row, "promotion_id", backend).map_err(store_error)?,
        promotion_revision: positive_revision(row, "promotion_revision")?,
        release_id: row.try_get("", "release_id").map_err(store_error)?,
        module_slug: row.try_get("", "module_slug").map_err(store_error)?,
        module_version: row.try_get("", "module_version").map_err(store_error)?,
        cargo_package: row.try_get("", "cargo_package").map_err(store_error)?,
        entry_type: row.try_get("", "entry_type").map_err(store_error)?,
        source_reference: row.try_get("", "source_reference").map_err(store_error)?,
        source_digest: row.try_get("", "source_digest").map_err(store_error)?,
        dependency_lock_digest: row
            .try_get("", "dependency_lock_digest")
            .map_err(store_error)?,
        executor_mode: ModuleStaticDistributionExecutorMode::parse(
            &row.try_get::<String>("", "executor_mode")
                .map_err(store_error)?,
        )?,
    })
}

fn validate_command(
    command: &ModuleStaticDistributionBuildCommand,
) -> Result<(), ModuleStaticDistributionError> {
    if command.actor_id.is_nil()
        || command.idempotency_key.is_nil()
        || !valid_reference(&command.platform_source_reference)
        || !valid_cas_source_reference(
            &command.platform_source_reference,
            &command.platform_source_digest,
        )
        || !valid_digest(&command.platform_source_digest)
        || !valid_digest(&command.toolchain_digest)
        || command.build_target.trim().is_empty()
        || command.build_target.len() > 128
        || command.build_target.chars().any(char::is_control)
        || command.selections.len() > MAX_DISTRIBUTION_ITEMS
    {
        return Err(if command.selections.len() > MAX_DISTRIBUTION_ITEMS {
            ModuleStaticDistributionError::TooManySelections
        } else {
            ModuleStaticDistributionError::InvalidCommand
        });
    }
    let mut promotion_ids = HashSet::with_capacity(command.selections.len());
    if command.selections.iter().any(|selection| {
        selection.promotion_id.is_nil()
            || selection.expected_promotion_revision == 0
            || !promotion_ids.insert(selection.promotion_id)
    }) {
        return Err(ModuleStaticDistributionError::InvalidCommand);
    }
    Ok(())
}

fn validate_runner_id(runner_id: &str) -> Result<(), ModuleStaticDistributionError> {
    if runner_id.trim().is_empty()
        || runner_id.len() > MAX_RUNNER_ID_BYTES
        || runner_id.chars().any(char::is_control)
    {
        return Err(ModuleStaticDistributionError::InvalidRunner);
    }
    Ok(())
}

fn validate_heartbeat_command(
    command: &ModuleStaticDistributionHeartbeatCommand,
) -> Result<(), ModuleStaticDistributionError> {
    if command.claim_id.is_nil() {
        return Err(ModuleStaticDistributionError::InvalidCommand);
    }
    validate_runner_id(&command.runner_id)
}

fn validate_completion_command(
    command: &ModuleStaticDistributionCompletionCommand,
) -> Result<(), ModuleStaticDistributionError> {
    if command.claim_id.is_nil()
        || command.distribution_build_id.is_nil()
        || command.composition_revision == 0
        || !valid_digest(&command.composition_digest)
    {
        return Err(ModuleStaticDistributionError::InvalidCommand);
    }
    validate_runner_id(&command.runner_id)?;
    match &command.outcome {
        ModuleStaticDistributionCompletionOutcome::Succeeded { evidence } => {
            for (reference, digest) in [
                (&evidence.artifact_reference, &evidence.artifact_digest),
                (&evidence.sbom_reference, &evidence.sbom_digest),
                (&evidence.provenance_reference, &evidence.provenance_digest),
                (&evidence.signature_reference, &evidence.signature_digest),
                (
                    &evidence.test_evidence_reference,
                    &evidence.test_evidence_digest,
                ),
            ] {
                if !valid_reference(reference) || !valid_digest(digest) {
                    return Err(ModuleStaticDistributionError::InvalidCommand);
                }
            }
        }
        ModuleStaticDistributionCompletionOutcome::Failed {
            failure_code,
            failure_detail,
        }
        | ModuleStaticDistributionCompletionOutcome::Cancelled {
            failure_code,
            failure_detail,
        } => {
            if failure_code.trim().is_empty()
                || failure_code.len() > MAX_FAILURE_CODE_BYTES
                || failure_code.chars().any(char::is_control)
                || failure_detail.trim().is_empty()
                || failure_detail.len() > MAX_FAILURE_DETAIL_BYTES
                || failure_detail.chars().any(char::is_control)
            {
                return Err(ModuleStaticDistributionError::InvalidCommand);
            }
        }
    }
    Ok(())
}

fn optional_uuid_value(value: Option<Uuid>, backend: DbBackend) -> sea_orm::Value {
    match (backend, value) {
        (DbBackend::Postgres, value) => sea_orm::Value::Uuid(value.map(Box::new)),
        (_, Some(value)) => value.to_string().into(),
        (_, None) => sea_orm::Value::String(None),
    }
}

fn optional_uuid_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<Uuid>, ModuleStaticDistributionError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(store_error),
        _ => row
            .try_get::<Option<String>>("", column)
            .map_err(store_error)?
            .map(|value| Uuid::parse_str(&value).map_err(store_error))
            .transpose(),
    }
}

fn revision_value(value: u64) -> Result<sea_orm::Value, ModuleStaticDistributionError> {
    i64::try_from(value)
        .map(Into::into)
        .map_err(|_| ModuleStaticDistributionError::RevisionOverflow)
}

fn revision_value_allow_zero(value: u64) -> Result<sea_orm::Value, ModuleStaticDistributionError> {
    revision_value(value)
}

fn positive_revision(
    row: &QueryResult,
    column: &str,
) -> Result<u64, ModuleStaticDistributionError> {
    optional_positive_revision(row, column)?
        .ok_or_else(|| ModuleStaticDistributionError::Store("revision is invalid".to_string()))
}

fn optional_positive_revision(
    row: &QueryResult,
    column: &str,
) -> Result<Option<u64>, ModuleStaticDistributionError> {
    let value: Option<i64> = row.try_get("", column).map_err(store_error)?;
    value
        .map(|value| {
            u64::try_from(value)
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| ModuleStaticDistributionError::Store("revision is invalid".into()))
        })
        .transpose()
}

fn positive_or_zero_revision(
    row: &QueryResult,
    column: &str,
) -> Result<u64, ModuleStaticDistributionError> {
    let value: i64 = row.try_get("", column).map_err(store_error)?;
    u64::try_from(value)
        .map_err(|_| ModuleStaticDistributionError::Store("revision is invalid".into()))
}

fn promotion_error(error: impl std::fmt::Display) -> ModuleStaticDistributionError {
    ModuleStaticDistributionError::Store(error.to_string())
}

fn promotion_load_error(error: ModuleStaticPromotionError) -> ModuleStaticDistributionError {
    match error {
        ModuleStaticPromotionError::Store(message) => ModuleStaticDistributionError::Store(message),
        _ => ModuleStaticDistributionError::PromotionNotApproved,
    }
}

fn promotion_evidence_error(error: ModuleStaticPromotionError) -> ModuleStaticDistributionError {
    match error {
        ModuleStaticPromotionError::Store(message) => ModuleStaticDistributionError::Store(message),
        _ => ModuleStaticDistributionError::PromotionEvidenceChanged,
    }
}

fn store_error(error: impl std::fmt::Display) -> ModuleStaticDistributionError {
    ModuleStaticDistributionError::Store(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    #[test]
    fn duplicate_promotion_selection_is_rejected() {
        let promotion_id = Uuid::new_v4();
        let command = ModuleStaticDistributionBuildCommand {
            expected_distribution_revision: 1,
            platform_source_reference: format!("cas://sha256:{}", "a".repeat(64)),
            platform_source_digest: format!("sha256:{}", "a".repeat(64)),
            toolchain_digest: format!("sha256:{}", "b".repeat(64)),
            build_target: "x86_64-unknown-linux-gnu".to_string(),
            selections: vec![
                ModuleStaticDistributionSelection {
                    promotion_id,
                    expected_promotion_revision: 2,
                },
                ModuleStaticDistributionSelection {
                    promotion_id,
                    expected_promotion_revision: 2,
                },
            ],
            actor_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        };
        assert!(matches!(
            validate_command(&command),
            Err(ModuleStaticDistributionError::InvalidCommand)
        ));
    }

    #[test]
    fn successful_completion_requires_signature_evidence() {
        let command = ModuleStaticDistributionCompletionCommand {
            claim_id: Uuid::new_v4(),
            runner_id: "distribution-worker".to_string(),
            distribution_build_id: Uuid::new_v4(),
            composition_revision: 2,
            composition_digest: digest('a'),
            outcome: ModuleStaticDistributionCompletionOutcome::Succeeded {
                evidence: ModuleStaticDistributionBuildEvidence {
                    artifact_reference: "oci://distribution".to_string(),
                    artifact_digest: digest('b'),
                    sbom_reference: "oci://distribution/sbom".to_string(),
                    sbom_digest: digest('c'),
                    provenance_reference: "oci://distribution/provenance".to_string(),
                    provenance_digest: digest('d'),
                    signature_reference: String::new(),
                    signature_digest: digest('e'),
                    test_evidence_reference: "evidence://distribution/tests".to_string(),
                    test_evidence_digest: digest('f'),
                },
            },
        };
        assert!(matches!(
            validate_completion_command(&command),
            Err(ModuleStaticDistributionError::InvalidCommand)
        ));
    }
}
