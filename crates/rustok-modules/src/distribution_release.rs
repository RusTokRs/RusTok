//! Verified activation ledger for completed static distribution builds.
//!
//! Activation records a release candidate that deployment tooling may consume.
//! It never mutates the running process or the active module runtime projection.

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::{
    data::{now_expression, placeholder, uuid_from_row, uuid_value},
    distribution::{
        advance_distribution_state, distribution_composition_digest, insert_build, load_build,
        load_distribution_state,
    },
    promotion::{
        digest_json, load_platform_build_evidence, load_promotion, valid_cas_source_reference,
        valid_digest, valid_reference, validate_promotion_review,
    },
    ControlPlaneInfrastructure, ModuleStaticDistributionBuild,
    ModuleStaticDistributionBuildEvidence, ModuleStaticDistributionBuildStatus,
    ModuleStaticDistributionCompletionCommand, ModuleStaticDistributionCompletionOutcome,
    ModuleStaticPromotionError, ModuleStaticPromotionStatus,
};

const DISTRIBUTION_RELEASE_STATE_ID: &str = "current";
const MAX_POLICY_REVISION_BYTES: usize = 128;
const MAX_VERIFIER_IDENTITY_BYTES: usize = 256;
const MAX_REASON_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStaticDistributionReleaseStatus {
    Active,
    Superseded,
    Revoked,
}

impl ModuleStaticDistributionReleaseStatus {
    fn parse(value: &str) -> Result<Self, ModuleStaticDistributionReleaseError> {
        match value {
            "active" => Ok(Self::Active),
            "superseded" => Ok(Self::Superseded),
            "revoked" => Ok(Self::Revoked),
            _ => Err(ModuleStaticDistributionReleaseError::Store(
                "static distribution release status is invalid".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionReleaseAdmission {
    pub verifier_identity: String,
    pub policy_revision: String,
    pub evidence_reference: String,
    pub evidence_digest: String,
    pub signature_verified: bool,
    pub provenance_verified: bool,
    pub sbom_verified: bool,
    pub test_evidence_verified: bool,
    pub dependency_policy_verified: bool,
}

impl ModuleStaticDistributionReleaseAdmission {
    fn admitted(&self) -> bool {
        self.signature_verified
            && self.provenance_verified
            && self.sbom_verified
            && self.test_evidence_verified
            && self.dependency_policy_verified
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionReleaseVerificationRequest {
    pub build: ModuleStaticDistributionBuild,
    pub policy_revision: String,
}

#[async_trait]
pub trait ModuleStaticDistributionReleaseVerifier: Send + Sync {
    async fn verify(
        &self,
        request: ModuleStaticDistributionReleaseVerificationRequest,
    ) -> Result<ModuleStaticDistributionReleaseAdmission, String>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionActivationCommand {
    pub distribution_build_id: Uuid,
    pub expected_release_revision: u64,
    pub verification_policy_revision: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRollbackCommand {
    pub target_release_id: Uuid,
    pub expected_release_revision: u64,
    pub expected_distribution_revision: u64,
    pub policy_revision: String,
    pub reason: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRevocationCommand {
    pub distribution_release_id: Uuid,
    pub expected_release_revision: u64,
    pub policy_revision: String,
    pub reason: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[async_trait]
pub trait ModuleStaticDistributionReleaseAuthorizer: Send + Sync {
    async fn authorize_activation(
        &self,
        command: &ModuleStaticDistributionActivationCommand,
    ) -> Result<(), ModuleStaticDistributionReleaseError>;

    async fn authorize_rollback(
        &self,
        command: &ModuleStaticDistributionRollbackCommand,
    ) -> Result<(), ModuleStaticDistributionReleaseError>;

    async fn authorize_revocation(
        &self,
        command: &ModuleStaticDistributionRevocationCommand,
    ) -> Result<(), ModuleStaticDistributionReleaseError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionReleaseState {
    pub revision: u64,
    pub active_release_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRelease {
    pub distribution_release_id: Uuid,
    pub distribution_build_id: Uuid,
    pub predecessor_release_id: Option<Uuid>,
    pub release_revision: u64,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub evidence: ModuleStaticDistributionBuildEvidence,
    pub status: ModuleStaticDistributionReleaseStatus,
    pub activated_by: Uuid,
    pub activated_at: chrono::DateTime<chrono::Utc>,
    pub superseded_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_by: Option<Uuid>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revocation_reason: Option<String>,
    pub revocation_policy_revision: Option<String>,
    pub verified_at: chrono::DateTime<chrono::Utc>,
    pub admission: ModuleStaticDistributionReleaseAdmission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionActivationReceipt {
    pub distribution_release_id: Uuid,
    pub release_revision: u64,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRollbackReceipt {
    pub rollback_id: Uuid,
    pub distribution_build_id: Uuid,
    pub composition_revision: u64,
    pub created: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStaticDistributionRollbackStatus {
    BuildQueued,
    Released,
    Cancelled,
}

impl ModuleStaticDistributionRollbackStatus {
    fn parse(value: &str) -> Result<Self, ModuleStaticDistributionReleaseError> {
        match value {
            "build_queued" => Ok(Self::BuildQueued),
            "released" => Ok(Self::Released),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(ModuleStaticDistributionReleaseError::Store(
                "static distribution rollback status is invalid".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRollback {
    pub rollback_id: Uuid,
    pub from_release_id: Uuid,
    pub target_release_id: Uuid,
    pub distribution_build_id: Uuid,
    pub release_state_revision: u64,
    pub composition_revision: u64,
    pub reason: String,
    pub policy_revision: String,
    pub requested_by: Uuid,
    pub status: ModuleStaticDistributionRollbackStatus,
    pub resulting_release_id: Option<Uuid>,
    pub requested_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRevocationReceipt {
    pub distribution_release_id: Uuid,
    pub release_state_revision: u64,
    pub was_active: bool,
    pub created: bool,
}

struct RollbackRequestRecord {
    rollback_id: Uuid,
    from_release_id: Uuid,
    target_release_id: Uuid,
    status: String,
}

#[derive(Clone)]
pub struct SeaOrmModuleStaticDistributionReleaseService<A, V> {
    db: DatabaseConnection,
    authorizer: A,
    verifier: V,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A, V> SeaOrmModuleStaticDistributionReleaseService<A, V>
where
    A: ModuleStaticDistributionReleaseAuthorizer,
    V: ModuleStaticDistributionReleaseVerifier,
{
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        verifier: V,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            verifier,
            infrastructure,
        }
    }

    pub async fn activate(
        &self,
        command: ModuleStaticDistributionActivationCommand,
    ) -> Result<ModuleStaticDistributionActivationReceipt, ModuleStaticDistributionReleaseError>
    {
        validate_activation_command(&command)?;
        self.authorizer.authorize_activation(&command).await?;
        let request_digest = digest_json(&command).map_err(promotion_error)?;
        validate_release_idempotency_key(
            &self.db,
            "activate",
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?;
        if let Some(receipt) = load_activation_operation(
            &self.db,
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?
        {
            return Ok(receipt);
        }
        let observed_release_state = load_release_state(&self.db, false).await?;
        if observed_release_state.revision != command.expected_release_revision {
            return Err(ModuleStaticDistributionReleaseError::RevisionConflict {
                expected: command.expected_release_revision,
                current: observed_release_state.revision,
            });
        }
        let observed_distribution_state = load_distribution_state(&self.db, false)
            .await
            .map_err(distribution_error)?;
        if observed_distribution_state.current_build_id != Some(command.distribution_build_id) {
            return Err(ModuleStaticDistributionReleaseError::BuildIsNotCurrent);
        }
        let verified_build = load_build(&self.db, command.distribution_build_id)
            .await
            .map_err(distribution_error)?;
        ensure_build_ready(&verified_build)?;
        if observed_distribution_state.revision != verified_build.composition_revision {
            return Err(ModuleStaticDistributionReleaseError::BuildIsNotCurrent);
        }
        let verification_request = ModuleStaticDistributionReleaseVerificationRequest {
            build: verified_build.clone(),
            policy_revision: command.verification_policy_revision.clone(),
        };
        let admission = self
            .verifier
            .verify(verification_request)
            .await
            .map_err(ModuleStaticDistributionReleaseError::VerificationFailed)?;
        validate_admission(&admission, &command.verification_policy_revision)?;

        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(receipt) = reserve_activation_operation(
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
        let distribution_state = load_distribution_state(&transaction, true)
            .await
            .map_err(distribution_error)?;
        if distribution_state.current_build_id != Some(command.distribution_build_id)
            || distribution_state.revision != verified_build.composition_revision
        {
            return Err(ModuleStaticDistributionReleaseError::BuildIsNotCurrent);
        }
        lock_build_for_activation(&transaction, command.distribution_build_id).await?;
        let current_build = load_build(&transaction, command.distribution_build_id)
            .await
            .map_err(distribution_error)?;
        if current_build != verified_build {
            return Err(ModuleStaticDistributionReleaseError::BuildEvidenceChanged);
        }
        revalidate_promotions(&transaction, &current_build).await?;
        let release_state = lock_release_state(&transaction).await?;
        if release_state.revision != command.expected_release_revision {
            return Err(ModuleStaticDistributionReleaseError::RevisionConflict {
                expected: command.expected_release_revision,
                current: release_state.revision,
            });
        }
        let rollback_request =
            load_rollback_request_for_build(&transaction, command.distribution_build_id, true)
                .await?;
        if let Some(rollback_request) = &rollback_request {
            if rollback_request.status != "build_queued"
                || release_state.active_release_id != Some(rollback_request.from_release_id)
            {
                return Err(ModuleStaticDistributionReleaseError::RollbackTargetInvalid);
            }
            let target_release =
                load_release_record(&transaction, rollback_request.target_release_id, true).await?;
            if target_release.status != ModuleStaticDistributionReleaseStatus::Superseded {
                return Err(ModuleStaticDistributionReleaseError::RollbackTargetInvalid);
            }
            let rebuilt_artifact_digest = current_build
                .result
                .as_ref()
                .ok_or(ModuleStaticDistributionReleaseError::BuildNotSucceeded)?
                .artifact_digest
                .as_str();
            if rebuilt_artifact_digest != target_release.evidence.artifact_digest.as_str() {
                return Err(ModuleStaticDistributionReleaseError::RollbackBuildNotReproducible);
            }
        }
        let release_revision = release_state
            .revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionReleaseError::RevisionOverflow)?;
        let distribution_release_id = self.infrastructure.new_id();
        let activated_at = self.infrastructure.now();
        if let Some(active_release_id) = release_state.active_release_id {
            supersede_release(&transaction, active_release_id).await?;
        }
        insert_release(
            &transaction,
            distribution_release_id,
            release_state.active_release_id,
            release_revision,
            command.actor_id,
            activated_at.to_owned(),
            &current_build,
        )
        .await?;
        insert_admission(
            &transaction,
            self.infrastructure.new_id(),
            distribution_release_id,
            &admission,
            activated_at,
        )
        .await?;
        if let Some(rollback_request) = rollback_request {
            complete_rollback_request(
                &transaction,
                rollback_request.rollback_id,
                distribution_release_id,
            )
            .await?;
        }
        advance_release_state(
            &transaction,
            release_state.revision,
            release_revision,
            distribution_release_id,
        )
        .await?;
        complete_activation_operation(
            &transaction,
            command.idempotency_key,
            distribution_release_id,
            release_revision,
        )
        .await?;
        let evidence = current_build
            .result
            .as_ref()
            .ok_or(ModuleStaticDistributionReleaseError::BuildNotSucceeded)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticDistributionReleaseActivated {
                        distribution_release_id,
                        predecessor_release_id: release_state.active_release_id,
                        distribution_build_id: command.distribution_build_id,
                        release_revision,
                        composition_revision: current_build.composition_revision,
                        composition_digest: current_build.composition_digest,
                        artifact_digest: evidence.artifact_digest.clone(),
                        policy_revision: admission.policy_revision,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticDistributionActivationReceipt {
            distribution_release_id,
            release_revision,
            created: true,
        })
    }

    pub async fn rollback(
        &self,
        command: ModuleStaticDistributionRollbackCommand,
    ) -> Result<ModuleStaticDistributionRollbackReceipt, ModuleStaticDistributionReleaseError> {
        validate_rollback_command(&command)?;
        self.authorizer.authorize_rollback(&command).await?;
        let request_digest = digest_json(&command).map_err(promotion_error)?;
        validate_release_idempotency_key(
            &self.db,
            "rollback",
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?;
        if let Some(receipt) = load_rollback_operation(
            &self.db,
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?
        {
            return Ok(receipt);
        }

        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(receipt) = reserve_rollback_operation(
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
        let distribution_state = load_distribution_state(&transaction, true)
            .await
            .map_err(distribution_error)?;
        if distribution_state.revision != command.expected_distribution_revision {
            return Err(
                ModuleStaticDistributionReleaseError::DistributionRevisionConflict {
                    expected: command.expected_distribution_revision,
                    current: distribution_state.revision,
                },
            );
        }
        let release_state = lock_release_state(&transaction).await?;
        if release_state.revision != command.expected_release_revision {
            return Err(ModuleStaticDistributionReleaseError::RevisionConflict {
                expected: command.expected_release_revision,
                current: release_state.revision,
            });
        }
        let from_release_id = release_state
            .active_release_id
            .ok_or(ModuleStaticDistributionReleaseError::NoActiveRelease)?;
        let from_release = load_release_record(&transaction, from_release_id, true).await?;
        if from_release.status != ModuleStaticDistributionReleaseStatus::Active
            || from_release.predecessor_release_id != Some(command.target_release_id)
        {
            return Err(ModuleStaticDistributionReleaseError::RollbackTargetInvalid);
        }
        if distribution_state.current_build_id != Some(from_release.distribution_build_id)
            || distribution_state.revision != from_release.composition_revision
        {
            return Err(ModuleStaticDistributionReleaseError::PendingDistributionBuild);
        }
        let target_release =
            load_release_record(&transaction, command.target_release_id, true).await?;
        if target_release.status != ModuleStaticDistributionReleaseStatus::Superseded {
            return Err(ModuleStaticDistributionReleaseError::RollbackTargetInvalid);
        }
        validate_admission(
            &target_release.admission,
            &target_release.admission.policy_revision,
        )?;
        lock_build_for_activation(&transaction, target_release.distribution_build_id).await?;
        let target_build = load_build(&transaction, target_release.distribution_build_id)
            .await
            .map_err(distribution_error)?;
        ensure_build_ready(&target_build)?;
        ensure_release_matches_build(&target_release, &target_build)?;
        revalidate_promotions(&transaction, &target_build).await?;

        let composition_revision = distribution_state
            .revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionReleaseError::RevisionOverflow)?;
        let distribution_build_id = self.infrastructure.new_id();
        let rollback_id = self.infrastructure.new_id();
        insert_build(
            &transaction,
            distribution_build_id,
            distribution_state.current_build_id,
            composition_revision,
            &target_build.composition_digest,
            &target_build.platform_source_reference,
            &target_build.platform_source_digest,
            &target_build.toolchain_digest,
            &target_build.build_target,
            command.actor_id,
            &target_build.items,
        )
        .await
        .map_err(distribution_error)?;
        advance_distribution_state(
            &transaction,
            distribution_state.revision,
            composition_revision,
            distribution_build_id,
        )
        .await
        .map_err(distribution_error)?;
        insert_rollback_request(
            &transaction,
            rollback_id,
            from_release_id,
            command.target_release_id,
            distribution_build_id,
            release_state.revision,
            composition_revision,
            &command.reason,
            &command.policy_revision,
            command.actor_id,
        )
        .await?;
        complete_rollback_operation(
            &transaction,
            command.idempotency_key,
            rollback_id,
            distribution_build_id,
            composition_revision,
        )
        .await?;
        let selected_promotions = u32::try_from(target_build.items.len())
            .map_err(|_| ModuleStaticDistributionReleaseError::RollbackTargetInvalid)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticDistributionBuildQueued {
                        distribution_build_id,
                        predecessor_build_id: distribution_state.current_build_id,
                        composition_revision,
                        composition_digest: target_build.composition_digest.clone(),
                        selected_promotions,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticDistributionRollbackBuildQueued {
                        rollback_id,
                        from_release_id,
                        target_release_id: command.target_release_id,
                        distribution_build_id,
                        composition_revision,
                        composition_digest: target_build.composition_digest,
                        policy_revision: command.policy_revision,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticDistributionRollbackReceipt {
            rollback_id,
            distribution_build_id,
            composition_revision,
            created: true,
        })
    }

    pub async fn revoke(
        &self,
        command: ModuleStaticDistributionRevocationCommand,
    ) -> Result<ModuleStaticDistributionRevocationReceipt, ModuleStaticDistributionReleaseError>
    {
        validate_revocation_command(&command)?;
        self.authorizer.authorize_revocation(&command).await?;
        let request_digest = digest_json(&command).map_err(promotion_error)?;
        validate_release_idempotency_key(
            &self.db,
            "revoke",
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?;
        if let Some(receipt) = load_revocation_operation(
            &self.db,
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?
        {
            return Ok(receipt);
        }
        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(receipt) = reserve_revocation_operation(
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
        let release_state = lock_release_state(&transaction).await?;
        if release_state.revision != command.expected_release_revision {
            return Err(ModuleStaticDistributionReleaseError::RevisionConflict {
                expected: command.expected_release_revision,
                current: release_state.revision,
            });
        }
        let release =
            load_release_record(&transaction, command.distribution_release_id, true).await?;
        if release.status == ModuleStaticDistributionReleaseStatus::Revoked {
            return Err(ModuleStaticDistributionReleaseError::ReleaseAlreadyRevoked);
        }
        let was_active = release.status == ModuleStaticDistributionReleaseStatus::Active;
        if was_active && release_state.active_release_id != Some(command.distribution_release_id) {
            return Err(ModuleStaticDistributionReleaseError::ActiveReleaseConflict);
        }
        revoke_release(
            &transaction,
            command.distribution_release_id,
            command.actor_id,
            &command.reason,
            &command.policy_revision,
        )
        .await?;
        cancel_pending_rollbacks(&transaction, command.distribution_release_id).await?;
        let release_state_revision = release_state
            .revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionReleaseError::RevisionOverflow)?;
        advance_release_state_optional(
            &transaction,
            release_state.revision,
            release_state_revision,
            if was_active {
                None
            } else {
                release_state.active_release_id
            },
        )
        .await?;
        complete_revocation_operation(
            &transaction,
            command.idempotency_key,
            command.distribution_release_id,
            release_state_revision,
            was_active,
        )
        .await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticDistributionReleaseRevoked {
                        distribution_release_id: command.distribution_release_id,
                        distribution_build_id: release.distribution_build_id,
                        release_state_revision,
                        was_active,
                        policy_revision: command.policy_revision,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticDistributionRevocationReceipt {
            distribution_release_id: command.distribution_release_id,
            release_state_revision,
            was_active,
            created: true,
        })
    }

    pub async fn current_state(
        &self,
    ) -> Result<ModuleStaticDistributionReleaseState, ModuleStaticDistributionReleaseError> {
        load_release_state(&self.db, false).await
    }

    pub async fn load_release(
        &self,
        distribution_release_id: Uuid,
    ) -> Result<ModuleStaticDistributionRelease, ModuleStaticDistributionReleaseError> {
        if distribution_release_id.is_nil() {
            return Err(ModuleStaticDistributionReleaseError::InvalidCommand);
        }
        load_release_record(&self.db, distribution_release_id, false).await
    }

    pub async fn load_rollback(
        &self,
        rollback_id: Uuid,
    ) -> Result<ModuleStaticDistributionRollback, ModuleStaticDistributionReleaseError> {
        if rollback_id.is_nil() {
            return Err(ModuleStaticDistributionReleaseError::InvalidCommand);
        }
        load_rollback_record(&self.db, rollback_id).await
    }
}

#[derive(Debug, Error)]
pub enum ModuleStaticDistributionReleaseError {
    #[error("static distribution release command is invalid")]
    InvalidCommand,
    #[error("static distribution release command was not authorized")]
    AuthorizationDenied,
    #[error("static distribution release verification failed: {0}")]
    VerificationFailed(String),
    #[error("static distribution release verification decision was not admitted")]
    VerificationDenied,
    #[error("static distribution build is not successfully completed")]
    BuildNotSucceeded,
    #[error("static distribution build is no longer the current desired composition")]
    BuildIsNotCurrent,
    #[error("static distribution build evidence changed after verification")]
    BuildEvidenceChanged,
    #[error("a selected promotion no longer matches approved release/build evidence")]
    PromotionEvidenceChanged,
    #[error("static distribution build already has a release")]
    BuildAlreadyActivated,
    #[error("static distribution release head no longer identifies an active release")]
    ActiveReleaseConflict,
    #[error("static distribution release head is empty")]
    NoActiveRelease,
    #[error("static distribution rollback target must be the non-revoked direct predecessor")]
    RollbackTargetInvalid,
    #[error("static distribution rollback was not found")]
    RollbackNotFound,
    #[error("static distribution rollback cannot replace a pending desired build")]
    PendingDistributionBuild,
    #[error("static distribution rollback build did not reproduce the target artifact digest")]
    RollbackBuildNotReproducible,
    #[error("static distribution release is already revoked")]
    ReleaseAlreadyRevoked,
    #[error("static distribution release was not found")]
    ReleaseNotFound,
    #[error(
        "static distribution release revision conflict: expected {expected}, current {current}"
    )]
    RevisionConflict { expected: u64, current: u64 },
    #[error("static distribution revision conflict: expected {expected}, current {current}")]
    DistributionRevisionConflict { expected: u64, current: u64 },
    #[error("static distribution release revision overflowed")]
    RevisionOverflow,
    #[error("static distribution release idempotency conflict")]
    IdempotencyConflict,
    #[error("static distribution release store error: {0}")]
    Store(String),
}

fn validate_activation_command(
    command: &ModuleStaticDistributionActivationCommand,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    if command.distribution_build_id.is_nil()
        || command.actor_id.is_nil()
        || command.idempotency_key.is_nil()
        || command.verification_policy_revision.trim().is_empty()
        || command.verification_policy_revision.trim() != command.verification_policy_revision
        || command.verification_policy_revision.len() > MAX_POLICY_REVISION_BYTES
        || command
            .verification_policy_revision
            .chars()
            .any(char::is_control)
    {
        return Err(ModuleStaticDistributionReleaseError::InvalidCommand);
    }
    Ok(())
}

fn validate_rollback_command(
    command: &ModuleStaticDistributionRollbackCommand,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    if command.target_release_id.is_nil()
        || command.expected_release_revision == 0
        || command.expected_distribution_revision == 0
        || command.actor_id.is_nil()
        || command.idempotency_key.is_nil()
        || !valid_policy_revision(&command.policy_revision)
        || !valid_reason(&command.reason)
    {
        return Err(ModuleStaticDistributionReleaseError::InvalidCommand);
    }
    Ok(())
}

fn validate_revocation_command(
    command: &ModuleStaticDistributionRevocationCommand,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    if command.distribution_release_id.is_nil()
        || command.expected_release_revision == 0
        || command.actor_id.is_nil()
        || command.idempotency_key.is_nil()
        || !valid_policy_revision(&command.policy_revision)
        || !valid_reason(&command.reason)
    {
        return Err(ModuleStaticDistributionReleaseError::InvalidCommand);
    }
    Ok(())
}

fn valid_policy_revision(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= MAX_POLICY_REVISION_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_reason(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= MAX_REASON_BYTES
        && !value.chars().any(char::is_control)
}

fn validate_admission(
    admission: &ModuleStaticDistributionReleaseAdmission,
    expected_policy_revision: &str,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    if admission.policy_revision != expected_policy_revision
        || admission.verifier_identity.trim().is_empty()
        || admission.verifier_identity.trim() != admission.verifier_identity
        || admission.verifier_identity.len() > MAX_VERIFIER_IDENTITY_BYTES
        || admission.verifier_identity.chars().any(char::is_control)
        || !valid_reference(&admission.evidence_reference)
        || admission.evidence_reference.trim() != admission.evidence_reference
        || !valid_digest(&admission.evidence_digest)
        || !admission.admitted()
    {
        return Err(ModuleStaticDistributionReleaseError::VerificationDenied);
    }
    Ok(())
}

fn ensure_build_ready(
    build: &ModuleStaticDistributionBuild,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let evidence = build
        .result
        .as_ref()
        .ok_or(ModuleStaticDistributionReleaseError::BuildNotSucceeded)?;
    if build.distribution_build_id.is_nil()
        || build
            .predecessor_build_id
            .is_some_and(|value| value.is_nil() || value == build.distribution_build_id)
        || build.composition_revision == 0
        || !valid_digest(&build.composition_digest)
        || !valid_cas_source_reference(
            &build.platform_source_reference,
            &build.platform_source_digest,
        )
        || !valid_digest(&build.toolchain_digest)
        || !valid_reference(&build.build_target)
        || build.build_target.trim() != build.build_target
        || build.status != ModuleStaticDistributionBuildStatus::Succeeded
        || build.failure.is_some()
        || build.requested_by.is_nil()
        || build.attempt_count == 0
        || build.active_claim_id.map_or(true, |value| value.is_nil())
        || build.claimed_by.as_deref().map_or(true, |value| {
            value.trim().is_empty() || value.trim() != value || value.chars().any(char::is_control)
        })
        || build
            .completion_digest
            .as_deref()
            .map_or(true, |value| !valid_digest(value))
    {
        return Err(ModuleStaticDistributionReleaseError::BuildNotSucceeded);
    }
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
        if !valid_reference(reference) || reference.trim() != reference || !valid_digest(digest) {
            return Err(ModuleStaticDistributionReleaseError::BuildNotSucceeded);
        }
    }
    let expected_composition_digest = distribution_composition_digest(
        &build.platform_source_reference,
        &build.platform_source_digest,
        &build.toolchain_digest,
        &build.build_target,
        &build.items,
    )
    .map_err(distribution_error)?;
    if build.composition_digest != expected_composition_digest {
        return Err(ModuleStaticDistributionReleaseError::BuildNotSucceeded);
    }
    let completion_command = ModuleStaticDistributionCompletionCommand {
        claim_id: build
            .active_claim_id
            .ok_or(ModuleStaticDistributionReleaseError::BuildNotSucceeded)?,
        runner_id: build
            .claimed_by
            .clone()
            .ok_or(ModuleStaticDistributionReleaseError::BuildNotSucceeded)?,
        distribution_build_id: build.distribution_build_id,
        composition_revision: build.composition_revision,
        composition_digest: build.composition_digest.clone(),
        outcome: ModuleStaticDistributionCompletionOutcome::Succeeded {
            evidence: evidence.clone(),
        },
    };
    let expected_completion_digest = digest_json(&completion_command).map_err(promotion_error)?;
    if build.completion_digest.as_deref() != Some(expected_completion_digest.as_str()) {
        return Err(ModuleStaticDistributionReleaseError::BuildNotSucceeded);
    }
    Ok(())
}

fn ensure_release_matches_build(
    release: &ModuleStaticDistributionRelease,
    build: &ModuleStaticDistributionBuild,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let evidence = build
        .result
        .as_ref()
        .ok_or(ModuleStaticDistributionReleaseError::BuildNotSucceeded)?;
    if release.distribution_build_id != build.distribution_build_id
        || release.composition_revision != build.composition_revision
        || release.composition_digest != build.composition_digest
        || release.evidence != *evidence
    {
        return Err(ModuleStaticDistributionReleaseError::BuildEvidenceChanged);
    }
    Ok(())
}

async fn revalidate_promotions(
    transaction: &DatabaseTransaction,
    build: &ModuleStaticDistributionBuild,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    for item in &build.items {
        let promotion = load_promotion(transaction, item.promotion_id)
            .await
            .map_err(promotion_evidence_error)?;
        if promotion.status != ModuleStaticPromotionStatus::Approved
            || promotion.revision != item.promotion_revision
            || promotion.release_id != item.release_id
            || promotion.module_slug != item.module_slug
            || promotion.module_version != item.module_version
            || promotion.cargo_package != item.cargo_package
            || promotion.entry_type != item.entry_type
            || promotion.source_reference != item.source_reference
            || promotion.source_digest != item.source_digest
            || promotion.dependency_lock_digest != item.dependency_lock_digest
        {
            return Err(ModuleStaticDistributionReleaseError::PromotionEvidenceChanged);
        }
        validate_promotion_review(transaction, &promotion)
            .await
            .map_err(promotion_evidence_error)?;
        let pinned = load_platform_build_evidence(transaction, &promotion.release_id)
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
            return Err(ModuleStaticDistributionReleaseError::PromotionEvidenceChanged);
        }
    }
    Ok(())
}

async fn lock_build_for_activation(
    transaction: &DatabaseTransaction,
    distribution_build_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
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
                "SELECT status FROM module_static_distribution_builds
                 WHERE distribution_build_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_build_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionReleaseError::BuildNotSucceeded)?;
    let status: String = row.try_get("", "status").map_err(store_error)?;
    if status != "succeeded" {
        return Err(ModuleStaticDistributionReleaseError::BuildNotSucceeded);
    }
    Ok(())
}

async fn insert_release(
    transaction: &DatabaseTransaction,
    distribution_release_id: Uuid,
    predecessor_release_id: Option<Uuid>,
    release_revision: u64,
    actor_id: Uuid,
    activated_at: chrono::DateTime<chrono::Utc>,
    build: &ModuleStaticDistributionBuild,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let evidence = build
        .result
        .as_ref()
        .ok_or(ModuleStaticDistributionReleaseError::BuildNotSucceeded)?;
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_releases
                 (distribution_release_id, distribution_build_id, predecessor_release_id,
                  release_revision, composition_revision, composition_digest,
                  artifact_reference, artifact_digest, sbom_reference, sbom_digest,
                  provenance_reference, provenance_digest, signature_reference,
                  signature_digest, test_evidence_reference, test_evidence_digest,
                  status, activated_by, activated_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {},
                         'active', {}, {})
                 ON CONFLICT (distribution_build_id) DO NOTHING",
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
                placeholder(backend, 15),
                placeholder(backend, 16),
                placeholder(backend, 17),
                placeholder(backend, 18),
            ),
            vec![
                uuid_value(distribution_release_id, backend),
                uuid_value(build.distribution_build_id, backend),
                optional_uuid_value(predecessor_release_id, backend),
                revision_value(release_revision)?,
                revision_value(build.composition_revision)?,
                build.composition_digest.clone().into(),
                evidence.artifact_reference.clone().into(),
                evidence.artifact_digest.clone().into(),
                evidence.sbom_reference.clone().into(),
                evidence.sbom_digest.clone().into(),
                evidence.provenance_reference.clone().into(),
                evidence.provenance_digest.clone().into(),
                evidence.signature_reference.clone().into(),
                evidence.signature_digest.clone().into(),
                evidence.test_evidence_reference.clone().into(),
                evidence.test_evidence_digest.clone().into(),
                uuid_value(actor_id, backend),
                datetime_value(activated_at, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if inserted.rows_affected() != 1 {
        return Err(ModuleStaticDistributionReleaseError::BuildAlreadyActivated);
    }
    Ok(())
}

async fn insert_admission(
    transaction: &DatabaseTransaction,
    admission_id: Uuid,
    distribution_release_id: Uuid,
    admission: &ModuleStaticDistributionReleaseAdmission,
    verified_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_release_admissions
                 (admission_id, distribution_release_id, verifier_identity, policy_revision,
                  evidence_reference, evidence_digest, signature_verified,
                  provenance_verified, sbom_verified, test_evidence_verified,
                  dependency_policy_verified, verified_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
            ),
            vec![
                uuid_value(admission_id, backend),
                uuid_value(distribution_release_id, backend),
                admission.verifier_identity.clone().into(),
                admission.policy_revision.clone().into(),
                admission.evidence_reference.clone().into(),
                admission.evidence_digest.clone().into(),
                admission.signature_verified.into(),
                admission.provenance_verified.into(),
                admission.sbom_verified.into(),
                admission.test_evidence_verified.into(),
                admission.dependency_policy_verified.into(),
                datetime_value(verified_at, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn supersede_release(
    transaction: &DatabaseTransaction,
    distribution_release_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_releases
                 SET status = 'superseded', superseded_at = {}
                 WHERE distribution_release_id = {} AND status = 'active'",
                now_expression(backend),
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_release_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionReleaseError::ActiveReleaseConflict);
    }
    Ok(())
}

async fn lock_release_state(
    transaction: &DatabaseTransaction,
) -> Result<ModuleStaticDistributionReleaseState, ModuleStaticDistributionReleaseError> {
    load_release_state(transaction, true).await
}

async fn load_release_state<C: ConnectionTrait>(
    connection: &C,
    lock_row: bool,
) -> Result<ModuleStaticDistributionReleaseState, ModuleStaticDistributionReleaseError> {
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
                "SELECT revision, active_release_id
                 FROM module_static_distribution_release_state WHERE state_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![DISTRIBUTION_RELEASE_STATE_ID.into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or_else(|| {
            ModuleStaticDistributionReleaseError::Store(
                "static distribution release state is unavailable".to_string(),
            )
        })?;
    Ok(ModuleStaticDistributionReleaseState {
        revision: revision_from_row(&row, "revision", true)?,
        active_release_id: optional_uuid_from_row(&row, "active_release_id", backend)?,
    })
}

async fn advance_release_state(
    transaction: &DatabaseTransaction,
    expected_revision: u64,
    next_revision: u64,
    distribution_release_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    advance_release_state_optional(
        transaction,
        expected_revision,
        next_revision,
        Some(distribution_release_id),
    )
    .await
}

async fn advance_release_state_optional(
    transaction: &DatabaseTransaction,
    expected_revision: u64,
    next_revision: u64,
    distribution_release_id: Option<Uuid>,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_release_state
                 SET revision = {}, active_release_id = {}, updated_at = {}
                 WHERE state_id = {} AND revision = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                now_expression(backend),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            vec![
                revision_value(next_revision)?,
                optional_uuid_value(distribution_release_id, backend),
                DISTRIBUTION_RELEASE_STATE_ID.into(),
                revision_value_allow_zero(expected_revision)?,
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        let current = load_release_state(transaction, false).await?;
        return Err(ModuleStaticDistributionReleaseError::RevisionConflict {
            expected: expected_revision,
            current: current.revision,
        });
    }
    Ok(())
}

async fn validate_release_idempotency_key<C: ConnectionTrait>(
    connection: &C,
    operation_kind: &str,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_kind, request_digest, actor_id
                 FROM module_static_distribution_release_idempotency_keys
                 WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?;
    if let Some(row) = row {
        let stored_kind: String = row.try_get("", "operation_kind").map_err(store_error)?;
        let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
        let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
        if stored_kind != operation_kind
            || stored_digest != request_digest
            || stored_actor != actor_id
        {
            return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
        }
    }
    Ok(())
}

async fn reserve_release_idempotency_key(
    transaction: &DatabaseTransaction,
    operation_kind: &str,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_release_idempotency_keys
                 (idempotency_key, operation_kind, request_digest, actor_id, created_at)
                 VALUES ({}, {}, {}, {}, {}) ON CONFLICT (idempotency_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                now_expression(backend),
            ),
            vec![
                uuid_value(idempotency_key, backend),
                operation_kind.to_owned().into(),
                request_digest.to_owned().into(),
                uuid_value(actor_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if inserted.rows_affected() == 1 {
        return Ok(());
    }
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_kind, request_digest, actor_id
                 FROM module_static_distribution_release_idempotency_keys
                 WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?;
    let stored_kind: String = row.try_get("", "operation_kind").map_err(store_error)?;
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
    if stored_kind != operation_kind || stored_digest != request_digest || stored_actor != actor_id
    {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(())
}

async fn reserve_activation_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionActivationReceipt>, ModuleStaticDistributionReleaseError>
{
    let backend = transaction.get_database_backend();
    reserve_release_idempotency_key(
        transaction,
        "activate",
        idempotency_key,
        request_digest,
        actor_id,
    )
    .await?;
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_release_operations
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
                "SELECT request_digest, actor_id, distribution_release_id,
                        release_revision,
                        CASE WHEN completed_at IS NULL THEN 0 ELSE 1 END AS completed
                 FROM module_static_distribution_release_operations WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?;
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
    if stored_digest != request_digest || stored_actor != actor_id {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    replay_activation_receipt(&row, backend).map(Some)
}

async fn load_activation_operation<C: ConnectionTrait>(
    connection: &C,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionActivationReceipt>, ModuleStaticDistributionReleaseError>
{
    let backend = connection.get_database_backend();
    let Some(row) = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_digest, actor_id, distribution_release_id,
                        release_revision,
                        CASE WHEN completed_at IS NULL THEN 0 ELSE 1 END AS completed
                 FROM module_static_distribution_release_operations WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?
    else {
        return Ok(None);
    };
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
    if stored_digest != request_digest || stored_actor != actor_id {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    replay_activation_receipt(&row, backend).map(Some)
}

async fn complete_activation_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    distribution_release_id: Uuid,
    release_revision: u64,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_release_operations
                 SET distribution_release_id = {}, release_revision = {}, completed_at = {}
                 WHERE idempotency_key = {} AND distribution_release_id IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
                now_expression(backend),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(distribution_release_id, backend),
                revision_value(release_revision)?,
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(())
}

fn replay_activation_receipt(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ModuleStaticDistributionActivationReceipt, ModuleStaticDistributionReleaseError> {
    let distribution_release_id = optional_uuid_from_row(row, "distribution_release_id", backend)?
        .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?;
    let release_revision = optional_revision_from_row(row, "release_revision")?
        .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?;
    let completed: i64 = row.try_get("", "completed").map_err(store_error)?;
    if completed != 1 {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(ModuleStaticDistributionActivationReceipt {
        distribution_release_id,
        release_revision,
        created: false,
    })
}

async fn insert_rollback_request(
    transaction: &DatabaseTransaction,
    rollback_id: Uuid,
    from_release_id: Uuid,
    target_release_id: Uuid,
    distribution_build_id: Uuid,
    release_state_revision: u64,
    composition_revision: u64,
    reason: &str,
    policy_revision: &str,
    actor_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_rollback_requests
                 (rollback_id, from_release_id, target_release_id, distribution_build_id,
                  release_state_revision, composition_revision, reason, policy_revision,
                  requested_by, status, requested_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, 'build_queued', {})",
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
                uuid_value(rollback_id, backend),
                uuid_value(from_release_id, backend),
                uuid_value(target_release_id, backend),
                uuid_value(distribution_build_id, backend),
                revision_value(release_state_revision)?,
                revision_value(composition_revision)?,
                reason.to_owned().into(),
                policy_revision.to_owned().into(),
                uuid_value(actor_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn load_rollback_request_for_build<C: ConnectionTrait>(
    connection: &C,
    distribution_build_id: Uuid,
    lock_row: bool,
) -> Result<Option<RollbackRequestRecord>, ModuleStaticDistributionReleaseError> {
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
                "SELECT rollback_id, from_release_id, target_release_id, status
                 FROM module_static_distribution_rollback_requests
                 WHERE distribution_build_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_build_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    row.map(|row| {
        Ok(RollbackRequestRecord {
            rollback_id: uuid_from_row(&row, "rollback_id", backend).map_err(store_error)?,
            from_release_id: uuid_from_row(&row, "from_release_id", backend)
                .map_err(store_error)?,
            target_release_id: uuid_from_row(&row, "target_release_id", backend)
                .map_err(store_error)?,
            status: row.try_get("", "status").map_err(store_error)?,
        })
    })
    .transpose()
}

async fn complete_rollback_request(
    transaction: &DatabaseTransaction,
    rollback_id: Uuid,
    distribution_release_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollback_requests
                 SET status = 'released', resulting_release_id = {}, completed_at = {}
                 WHERE rollback_id = {} AND status = 'build_queued'",
                placeholder(backend, 1),
                now_expression(backend),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(distribution_release_id, backend),
                uuid_value(rollback_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionReleaseError::RollbackTargetInvalid);
    }
    Ok(())
}

async fn reserve_rollback_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionRollbackReceipt>, ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    reserve_release_idempotency_key(
        transaction,
        "rollback",
        idempotency_key,
        request_digest,
        actor_id,
    )
    .await?;
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_rollback_operations
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
    query_rollback_operation(transaction, idempotency_key, request_digest, actor_id).await
}

async fn load_rollback_operation<C: ConnectionTrait>(
    connection: &C,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionRollbackReceipt>, ModuleStaticDistributionReleaseError> {
    query_rollback_operation(connection, idempotency_key, request_digest, actor_id).await
}

async fn query_rollback_operation<C: ConnectionTrait>(
    connection: &C,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionRollbackReceipt>, ModuleStaticDistributionReleaseError> {
    let backend = connection.get_database_backend();
    let Some(row) = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_digest, actor_id, rollback_id, distribution_build_id,
                        composition_revision,
                        CASE WHEN completed_at IS NULL THEN 0 ELSE 1 END AS completed
                 FROM module_static_distribution_rollback_operations
                 WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?
    else {
        return Ok(None);
    };
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
    if stored_digest != request_digest || stored_actor != actor_id {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    let completed: i64 = row.try_get("", "completed").map_err(store_error)?;
    if completed != 1 {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(Some(ModuleStaticDistributionRollbackReceipt {
        rollback_id: optional_uuid_from_row(&row, "rollback_id", backend)?
            .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?,
        distribution_build_id: optional_uuid_from_row(&row, "distribution_build_id", backend)?
            .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?,
        composition_revision: optional_revision_from_row(&row, "composition_revision")?
            .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?,
        created: false,
    }))
}

async fn complete_rollback_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    rollback_id: Uuid,
    distribution_build_id: Uuid,
    composition_revision: u64,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollback_operations
                 SET rollback_id = {}, distribution_build_id = {}, composition_revision = {},
                     completed_at = {}
                 WHERE idempotency_key = {} AND rollback_id IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
                placeholder(backend, 4),
            ),
            vec![
                uuid_value(rollback_id, backend),
                uuid_value(distribution_build_id, backend),
                revision_value(composition_revision)?,
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(())
}

async fn revoke_release(
    transaction: &DatabaseTransaction,
    distribution_release_id: Uuid,
    actor_id: Uuid,
    reason: &str,
    policy_revision: &str,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_releases
                 SET status = 'revoked', revoked_by = {}, revoked_at = {},
                     revocation_reason = {}, revocation_policy_revision = {}
                 WHERE distribution_release_id = {} AND status IN ('active', 'superseded')",
                placeholder(backend, 1),
                now_expression(backend),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            vec![
                uuid_value(actor_id, backend),
                reason.to_owned().into(),
                policy_revision.to_owned().into(),
                uuid_value(distribution_release_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionReleaseError::ReleaseAlreadyRevoked);
    }
    Ok(())
}

async fn cancel_pending_rollbacks(
    transaction: &DatabaseTransaction,
    release_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollback_requests
                 SET status = 'cancelled', completed_at = {}
                 WHERE (target_release_id = {} OR from_release_id = {})
                   AND status = 'build_queued'",
                now_expression(backend),
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(release_id, backend),
                uuid_value(release_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

pub(crate) async fn cancel_pending_rollback_for_build(
    transaction: &DatabaseTransaction,
    distribution_build_id: Uuid,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollback_requests
                 SET status = 'cancelled', completed_at = {}
                 WHERE distribution_build_id = {} AND status = 'build_queued'",
                now_expression(backend),
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_build_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn reserve_revocation_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionRevocationReceipt>, ModuleStaticDistributionReleaseError>
{
    let backend = transaction.get_database_backend();
    reserve_release_idempotency_key(
        transaction,
        "revoke",
        idempotency_key,
        request_digest,
        actor_id,
    )
    .await?;
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_revocation_operations
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
    query_revocation_operation(transaction, idempotency_key, request_digest, actor_id).await
}

async fn load_revocation_operation<C: ConnectionTrait>(
    connection: &C,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionRevocationReceipt>, ModuleStaticDistributionReleaseError>
{
    query_revocation_operation(connection, idempotency_key, request_digest, actor_id).await
}

async fn query_revocation_operation<C: ConnectionTrait>(
    connection: &C,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticDistributionRevocationReceipt>, ModuleStaticDistributionReleaseError>
{
    let backend = connection.get_database_backend();
    let Some(row) = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_digest, actor_id, distribution_release_id,
                        release_state_revision, was_active,
                        CASE WHEN completed_at IS NULL THEN 0 ELSE 1 END AS completed
                 FROM module_static_distribution_revocation_operations
                 WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?
    else {
        return Ok(None);
    };
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
    if stored_digest != request_digest || stored_actor != actor_id {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    let completed: i64 = row.try_get("", "completed").map_err(store_error)?;
    if completed != 1 {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(Some(ModuleStaticDistributionRevocationReceipt {
        distribution_release_id: optional_uuid_from_row(&row, "distribution_release_id", backend)?
            .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?,
        release_state_revision: optional_revision_from_row(&row, "release_state_revision")?
            .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?,
        was_active: optional_bool_from_row(&row, "was_active", backend)?
            .ok_or(ModuleStaticDistributionReleaseError::IdempotencyConflict)?,
        created: false,
    }))
}

async fn complete_revocation_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    distribution_release_id: Uuid,
    release_state_revision: u64,
    was_active: bool,
) -> Result<(), ModuleStaticDistributionReleaseError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_revocation_operations
                 SET distribution_release_id = {}, release_state_revision = {}, was_active = {},
                     completed_at = {}
                 WHERE idempotency_key = {} AND distribution_release_id IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
                placeholder(backend, 4),
            ),
            vec![
                uuid_value(distribution_release_id, backend),
                revision_value(release_state_revision)?,
                was_active.into(),
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionReleaseError::IdempotencyConflict);
    }
    Ok(())
}

async fn load_rollback_record<C: ConnectionTrait>(
    connection: &C,
    rollback_id: Uuid,
) -> Result<ModuleStaticDistributionRollback, ModuleStaticDistributionReleaseError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT rollback_id, from_release_id, target_release_id,
                        distribution_build_id, release_state_revision, composition_revision,
                        reason, policy_revision, requested_by, status,
                        resulting_release_id, requested_at, completed_at
                 FROM module_static_distribution_rollback_requests
                 WHERE rollback_id = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(rollback_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionReleaseError::RollbackNotFound)?;
    Ok(ModuleStaticDistributionRollback {
        rollback_id: uuid_from_row(&row, "rollback_id", backend).map_err(store_error)?,
        from_release_id: uuid_from_row(&row, "from_release_id", backend).map_err(store_error)?,
        target_release_id: uuid_from_row(&row, "target_release_id", backend)
            .map_err(store_error)?,
        distribution_build_id: uuid_from_row(&row, "distribution_build_id", backend)
            .map_err(store_error)?,
        release_state_revision: revision_from_row(&row, "release_state_revision", false)?,
        composition_revision: revision_from_row(&row, "composition_revision", false)?,
        reason: row.try_get("", "reason").map_err(store_error)?,
        policy_revision: row.try_get("", "policy_revision").map_err(store_error)?,
        requested_by: uuid_from_row(&row, "requested_by", backend).map_err(store_error)?,
        status: ModuleStaticDistributionRollbackStatus::parse(
            &row.try_get::<String>("", "status").map_err(store_error)?,
        )?,
        resulting_release_id: optional_uuid_from_row(&row, "resulting_release_id", backend)?,
        requested_at: datetime_from_row(&row, "requested_at", backend)?,
        completed_at: optional_datetime_from_row(&row, "completed_at", backend)?,
    })
}

async fn load_release_record<C: ConnectionTrait>(
    connection: &C,
    distribution_release_id: Uuid,
    lock_row: bool,
) -> Result<ModuleStaticDistributionRelease, ModuleStaticDistributionReleaseError> {
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
                "SELECT release.distribution_release_id, release.distribution_build_id,
                        release.predecessor_release_id, release.release_revision,
                        release.composition_revision, release.composition_digest,
                        release.artifact_reference, release.artifact_digest,
                        release.sbom_reference, release.sbom_digest,
                        release.provenance_reference, release.provenance_digest,
                        release.signature_reference, release.signature_digest,
                        release.test_evidence_reference, release.test_evidence_digest,
                        release.status, release.activated_by, release.activated_at,
                        release.superseded_at, release.revoked_by, release.revoked_at,
                        release.revocation_reason, release.revocation_policy_revision,
                        admission.verifier_identity, admission.policy_revision,
                        admission.evidence_reference, admission.evidence_digest,
                        admission.signature_verified, admission.provenance_verified,
                        admission.sbom_verified, admission.test_evidence_verified,
                        admission.dependency_policy_verified, admission.verified_at
                 FROM module_static_distribution_releases AS release
                 JOIN module_static_distribution_release_admissions AS admission
                   ON admission.distribution_release_id = release.distribution_release_id
                 WHERE release.distribution_release_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(distribution_release_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionReleaseError::ReleaseNotFound)?;
    Ok(ModuleStaticDistributionRelease {
        distribution_release_id: uuid_from_row(&row, "distribution_release_id", backend)
            .map_err(store_error)?,
        distribution_build_id: uuid_from_row(&row, "distribution_build_id", backend)
            .map_err(store_error)?,
        predecessor_release_id: optional_uuid_from_row(&row, "predecessor_release_id", backend)?,
        release_revision: revision_from_row(&row, "release_revision", false)?,
        composition_revision: revision_from_row(&row, "composition_revision", false)?,
        composition_digest: row.try_get("", "composition_digest").map_err(store_error)?,
        evidence: ModuleStaticDistributionBuildEvidence {
            artifact_reference: row.try_get("", "artifact_reference").map_err(store_error)?,
            artifact_digest: row.try_get("", "artifact_digest").map_err(store_error)?,
            sbom_reference: row.try_get("", "sbom_reference").map_err(store_error)?,
            sbom_digest: row.try_get("", "sbom_digest").map_err(store_error)?,
            provenance_reference: row
                .try_get("", "provenance_reference")
                .map_err(store_error)?,
            provenance_digest: row.try_get("", "provenance_digest").map_err(store_error)?,
            signature_reference: row
                .try_get("", "signature_reference")
                .map_err(store_error)?,
            signature_digest: row.try_get("", "signature_digest").map_err(store_error)?,
            test_evidence_reference: row
                .try_get("", "test_evidence_reference")
                .map_err(store_error)?,
            test_evidence_digest: row
                .try_get("", "test_evidence_digest")
                .map_err(store_error)?,
        },
        status: ModuleStaticDistributionReleaseStatus::parse(
            &row.try_get::<String>("", "status").map_err(store_error)?,
        )?,
        activated_by: uuid_from_row(&row, "activated_by", backend).map_err(store_error)?,
        activated_at: datetime_from_row(&row, "activated_at", backend)?,
        superseded_at: optional_datetime_from_row(&row, "superseded_at", backend)?,
        revoked_by: optional_uuid_from_row(&row, "revoked_by", backend)?,
        revoked_at: optional_datetime_from_row(&row, "revoked_at", backend)?,
        revocation_reason: row.try_get("", "revocation_reason").map_err(store_error)?,
        revocation_policy_revision: row
            .try_get("", "revocation_policy_revision")
            .map_err(store_error)?,
        verified_at: datetime_from_row(&row, "verified_at", backend)?,
        admission: ModuleStaticDistributionReleaseAdmission {
            verifier_identity: row.try_get("", "verifier_identity").map_err(store_error)?,
            policy_revision: row.try_get("", "policy_revision").map_err(store_error)?,
            evidence_reference: row.try_get("", "evidence_reference").map_err(store_error)?,
            evidence_digest: row.try_get("", "evidence_digest").map_err(store_error)?,
            signature_verified: row.try_get("", "signature_verified").map_err(store_error)?,
            provenance_verified: row
                .try_get("", "provenance_verified")
                .map_err(store_error)?,
            sbom_verified: row.try_get("", "sbom_verified").map_err(store_error)?,
            test_evidence_verified: row
                .try_get("", "test_evidence_verified")
                .map_err(store_error)?,
            dependency_policy_verified: row
                .try_get("", "dependency_policy_verified")
                .map_err(store_error)?,
        },
    })
}

fn optional_uuid_value(value: Option<Uuid>, backend: DbBackend) -> sea_orm::Value {
    match (backend, value) {
        (DbBackend::Postgres, value) => sea_orm::Value::Uuid(value.map(Box::new)),
        (_, Some(value)) => value.to_string().into(),
        (_, None) => sea_orm::Value::String(None),
    }
}

fn datetime_value(value: chrono::DateTime<chrono::Utc>, backend: DbBackend) -> sea_orm::Value {
    match backend {
        DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(Box::new(value))),
        _ => value.to_rfc3339().into(),
    }
}

fn datetime_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<chrono::DateTime<chrono::Utc>, ModuleStaticDistributionReleaseError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(store_error),
        _ => parse_sqlite_datetime(&row.try_get::<String>("", column).map_err(store_error)?),
    }
}

fn optional_datetime_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ModuleStaticDistributionReleaseError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(store_error),
        _ => row
            .try_get::<Option<String>>("", column)
            .map_err(store_error)?
            .as_deref()
            .map(parse_sqlite_datetime)
            .transpose(),
    }
}

fn parse_sqlite_datetime(
    value: &str,
) -> Result<chrono::DateTime<chrono::Utc>, ModuleStaticDistributionReleaseError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .map(|timestamp| timestamp.and_utc())
        })
        .map_err(store_error)
}

fn optional_uuid_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<Uuid>, ModuleStaticDistributionReleaseError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(store_error),
        _ => row
            .try_get::<Option<String>>("", column)
            .map_err(store_error)?
            .map(|value| Uuid::parse_str(&value).map_err(store_error))
            .transpose(),
    }
}

fn optional_bool_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<bool>, ModuleStaticDistributionReleaseError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(store_error),
        _ => row
            .try_get::<Option<i64>>("", column)
            .map_err(store_error)
            .and_then(|value| match value {
                Some(0) => Ok(Some(false)),
                Some(1) => Ok(Some(true)),
                Some(_) => Err(ModuleStaticDistributionReleaseError::Store(
                    "stored boolean is invalid".to_string(),
                )),
                None => Ok(None),
            }),
    }
}

fn revision_value(value: u64) -> Result<sea_orm::Value, ModuleStaticDistributionReleaseError> {
    i64::try_from(value)
        .map(Into::into)
        .map_err(|_| ModuleStaticDistributionReleaseError::RevisionOverflow)
}

fn revision_value_allow_zero(
    value: u64,
) -> Result<sea_orm::Value, ModuleStaticDistributionReleaseError> {
    revision_value(value)
}

fn revision_from_row(
    row: &QueryResult,
    column: &str,
    allow_zero: bool,
) -> Result<u64, ModuleStaticDistributionReleaseError> {
    let value: i64 = row.try_get("", column).map_err(store_error)?;
    u64::try_from(value)
        .ok()
        .filter(|value| allow_zero || *value > 0)
        .ok_or_else(|| {
            ModuleStaticDistributionReleaseError::Store("release revision is invalid".to_string())
        })
}

fn optional_revision_from_row(
    row: &QueryResult,
    column: &str,
) -> Result<Option<u64>, ModuleStaticDistributionReleaseError> {
    let value: Option<i64> = row.try_get("", column).map_err(store_error)?;
    value
        .map(|value| {
            u64::try_from(value)
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    ModuleStaticDistributionReleaseError::Store(
                        "release revision is invalid".to_string(),
                    )
                })
        })
        .transpose()
}

fn promotion_evidence_error(
    error: ModuleStaticPromotionError,
) -> ModuleStaticDistributionReleaseError {
    match error {
        ModuleStaticPromotionError::Store(message) => {
            ModuleStaticDistributionReleaseError::Store(message)
        }
        _ => ModuleStaticDistributionReleaseError::PromotionEvidenceChanged,
    }
}

fn promotion_error(error: impl std::fmt::Display) -> ModuleStaticDistributionReleaseError {
    ModuleStaticDistributionReleaseError::Store(error.to_string())
}

fn distribution_error(error: impl std::fmt::Display) -> ModuleStaticDistributionReleaseError {
    ModuleStaticDistributionReleaseError::Store(error.to_string())
}

fn store_error(error: impl std::fmt::Display) -> ModuleStaticDistributionReleaseError {
    ModuleStaticDistributionReleaseError::Store(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_requires_every_independent_verification_fact() {
        let mut admission = ModuleStaticDistributionReleaseAdmission {
            verifier_identity: "distribution-verifier".to_string(),
            policy_revision: "policy-current".to_string(),
            evidence_reference: "evidence://distribution/admission".to_string(),
            evidence_digest: format!("sha256:{}", "a".repeat(64)),
            signature_verified: true,
            provenance_verified: true,
            sbom_verified: true,
            test_evidence_verified: true,
            dependency_policy_verified: true,
        };
        assert!(admission.admitted());
        admission.dependency_policy_verified = false;
        assert!(!admission.admitted());
    }
}
