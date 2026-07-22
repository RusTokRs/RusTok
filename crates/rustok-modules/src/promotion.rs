//! Owner-reviewed static promotion for future distribution builds.
//!
//! This service never compiles code and never mutates the running composition.
//! It binds one published platform-built release to its immutable source/build
//! evidence, then records a platform-team approval for later build tooling.

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::{
    data::{configure_tenant_scope, now_expression, placeholder, uuid_from_row, uuid_value},
    ControlPlaneInfrastructure, ModuleBuildOutcome, ModuleBuildRequest, ModuleBuildResult,
};

const MAX_REFERENCE_BYTES: usize = 512;
const MAX_POLICY_REVISION_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStaticPromotionStatus {
    Requested,
    Approved,
}

impl ModuleStaticPromotionStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Approved => "approved",
        }
    }

    fn parse(value: &str) -> Result<Self, ModuleStaticPromotionError> {
        match value {
            "requested" => Ok(Self::Requested),
            "approved" => Ok(Self::Approved),
            _ => Err(ModuleStaticPromotionError::Store(
                "static promotion status is invalid".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticPromotion {
    pub promotion_id: Uuid,
    pub release_id: String,
    pub publish_request_id: String,
    pub module_slug: String,
    pub module_version: String,
    pub cargo_package: String,
    pub entry_type: String,
    pub source_reference: String,
    pub source_digest: String,
    pub dependency_lock_digest: String,
    pub status: ModuleStaticPromotionStatus,
    pub revision: u64,
    pub requested_by: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticPromotionReceipt {
    pub promotion_id: Uuid,
    pub status: ModuleStaticPromotionStatus,
    pub revision: u64,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticPromotionRequestCommand {
    pub release_id: String,
    pub source_reference: String,
    pub source_digest: String,
    pub dependency_lock_digest: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticPromotionEvidence {
    pub reference: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticPromotionApprovalEvidence {
    pub ownership: ModuleStaticPromotionEvidence,
    pub dependency_audit: ModuleStaticPromotionEvidence,
    pub tests: ModuleStaticPromotionEvidence,
    pub static_review: ModuleStaticPromotionEvidence,
    pub policy_revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticPromotionApprovalCommand {
    pub promotion_id: Uuid,
    pub expected_revision: u64,
    pub evidence: ModuleStaticPromotionApprovalEvidence,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[async_trait]
pub trait ModuleStaticPromotionAuthorizer: Send + Sync {
    async fn authorize_request(
        &self,
        command: &ModuleStaticPromotionRequestCommand,
    ) -> Result<(), ModuleStaticPromotionError>;

    async fn authorize_approval(
        &self,
        command: &ModuleStaticPromotionApprovalCommand,
    ) -> Result<(), ModuleStaticPromotionError>;
}

#[derive(Clone)]
pub struct SeaOrmModulePromotionService<A> {
    db: DatabaseConnection,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmModulePromotionService<A>
where
    A: ModuleStaticPromotionAuthorizer,
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

    pub async fn request(
        &self,
        command: ModuleStaticPromotionRequestCommand,
    ) -> Result<ModuleStaticPromotionReceipt, ModuleStaticPromotionError> {
        validate_request_command(&command)?;
        self.authorizer.authorize_request(&command).await?;
        let request_digest = digest_json(&command)?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(receipt) = reserve_operation(
            &transaction,
            "request",
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return Ok(receipt);
        }

        let evidence = load_platform_build_evidence(&transaction, &command.release_id).await?;
        if command.source_reference != evidence.source_reference
            || command.source_digest != evidence.source_digest
            || command.dependency_lock_digest != evidence.dependency_lock_digest
        {
            return Err(ModuleStaticPromotionError::BuildEvidenceMismatch);
        }
        let promotion_id = self.infrastructure.new_id();
        let backend = transaction.get_database_backend();
        let inserted = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_static_promotions
                     (promotion_id, release_id, publish_request_id, module_slug, module_version,
                      cargo_package, entry_type, source_reference, source_digest,
                      dependency_lock_digest, status, revision, requested_by, requested_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'requested', 1, {}, {})
                     ON CONFLICT (release_id) DO NOTHING",
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
                    now_expression(backend),
                ),
                vec![
                    uuid_value(promotion_id, backend),
                    command.release_id.clone().into(),
                    evidence.publish_request_id.clone().into(),
                    evidence.module_slug.clone().into(),
                    evidence.module_version.clone().into(),
                    evidence.cargo_package.clone().into(),
                    evidence.entry_type.clone().into(),
                    command.source_reference.clone().into(),
                    command.source_digest.clone().into(),
                    command.dependency_lock_digest.clone().into(),
                    uuid_value(command.actor_id, backend),
                ],
            ))
            .await
            .map_err(store_error)?;
        if inserted.rows_affected() != 1 {
            return Err(ModuleStaticPromotionError::ReleaseAlreadyRequested);
        }
        complete_operation(
            &transaction,
            command.idempotency_key,
            promotion_id,
            1,
            ModuleStaticPromotionStatus::Requested,
        )
        .await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticPromotionRequested {
                        promotion_id,
                        release_id: command.release_id,
                        module_slug: evidence.module_slug,
                        module_version: evidence.module_version,
                        source_digest: command.source_digest,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticPromotionReceipt {
            promotion_id,
            status: ModuleStaticPromotionStatus::Requested,
            revision: 1,
            created: true,
        })
    }

    pub async fn approve(
        &self,
        command: ModuleStaticPromotionApprovalCommand,
    ) -> Result<ModuleStaticPromotionReceipt, ModuleStaticPromotionError> {
        validate_approval_command(&command)?;
        self.authorizer.authorize_approval(&command).await?;
        let request_digest = digest_json(&command)?;
        let review_digest = digest_json(&command.evidence)?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(receipt) = reserve_operation(
            &transaction,
            "approve",
            command.idempotency_key,
            &request_digest,
            command.actor_id,
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return Ok(receipt);
        }
        let promotion = lock_promotion(&transaction, command.promotion_id).await?;
        if promotion.status != ModuleStaticPromotionStatus::Requested
            || promotion.revision != command.expected_revision
        {
            return Err(ModuleStaticPromotionError::RevisionConflict);
        }
        if promotion.requested_by == command.actor_id {
            return Err(ModuleStaticPromotionError::ApprovalAuthorityConflict);
        }
        let pinned = load_platform_build_evidence(&transaction, &promotion.release_id).await?;
        if pinned.publish_request_id != promotion.publish_request_id
            || pinned.module_slug != promotion.module_slug
            || pinned.module_version != promotion.module_version
            || pinned.cargo_package != promotion.cargo_package
            || pinned.entry_type != promotion.entry_type
            || pinned.source_reference != promotion.source_reference
            || pinned.source_digest != promotion.source_digest
            || pinned.dependency_lock_digest != promotion.dependency_lock_digest
        {
            return Err(ModuleStaticPromotionError::BuildEvidenceMismatch);
        }
        let next_revision = promotion
            .revision
            .checked_add(1)
            .ok_or(ModuleStaticPromotionError::RevisionConflict)?;
        let review_id = self.infrastructure.new_id();
        let backend = transaction.get_database_backend();
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_static_promotion_reviews
                     (review_id, promotion_id, ownership_evidence_reference,
                      ownership_evidence_digest, dependency_audit_reference,
                      dependency_audit_digest, test_evidence_reference, test_evidence_digest,
                      static_review_reference, static_review_digest, approval_policy_revision,
                      review_digest, reviewed_by, reviewed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
                    now_expression(backend),
                ),
                vec![
                    uuid_value(review_id, backend),
                    uuid_value(command.promotion_id, backend),
                    command.evidence.ownership.reference.clone().into(),
                    command.evidence.ownership.digest.clone().into(),
                    command.evidence.dependency_audit.reference.clone().into(),
                    command.evidence.dependency_audit.digest.clone().into(),
                    command.evidence.tests.reference.clone().into(),
                    command.evidence.tests.digest.clone().into(),
                    command.evidence.static_review.reference.clone().into(),
                    command.evidence.static_review.digest.clone().into(),
                    command.evidence.policy_revision.clone().into(),
                    review_digest.into(),
                    uuid_value(command.actor_id, backend),
                ],
            ))
            .await
            .map_err(store_error)?;
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_static_promotions
                     SET status = 'approved', revision = {}, approved_by = {}, approved_at = {}
                     WHERE promotion_id = {} AND status = 'requested' AND revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    now_expression(backend),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    revision_value(next_revision)?,
                    uuid_value(command.actor_id, backend),
                    uuid_value(command.promotion_id, backend),
                    revision_value(command.expected_revision)?,
                ],
            ))
            .await
            .map_err(store_error)?;
        if updated.rows_affected() != 1 {
            return Err(ModuleStaticPromotionError::RevisionConflict);
        }
        complete_operation(
            &transaction,
            command.idempotency_key,
            command.promotion_id,
            next_revision,
            ModuleStaticPromotionStatus::Approved,
        )
        .await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticPromotionApproved {
                        promotion_id: command.promotion_id,
                        release_id: promotion.release_id,
                        module_slug: promotion.module_slug,
                        module_version: promotion.module_version,
                        revision: next_revision,
                        policy_revision: command.evidence.policy_revision,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticPromotionReceipt {
            promotion_id: command.promotion_id,
            status: ModuleStaticPromotionStatus::Approved,
            revision: next_revision,
            created: true,
        })
    }

    pub async fn load(
        &self,
        promotion_id: Uuid,
    ) -> Result<ModuleStaticPromotion, ModuleStaticPromotionError> {
        if promotion_id.is_nil() {
            return Err(ModuleStaticPromotionError::InvalidCommand);
        }
        load_promotion(&self.db, promotion_id).await
    }
}

#[derive(Debug, Error)]
pub enum ModuleStaticPromotionError {
    #[error("static promotion command is invalid")]
    InvalidCommand,
    #[error("static promotion evidence is invalid")]
    InvalidEvidence,
    #[error("static promotion command was not authorized")]
    AuthorizationDenied,
    #[error("static promotion requester cannot approve the same request")]
    ApprovalAuthorityConflict,
    #[error("published release was not found")]
    ReleaseNotFound,
    #[error("only active platform-built releases can request static promotion")]
    ReleaseNotEligible,
    #[error("published release does not retain exact platform build evidence")]
    BuildEvidenceMissing,
    #[error("promotion command does not match the immutable platform build evidence")]
    BuildEvidenceMismatch,
    #[error("the release already has a static promotion request")]
    ReleaseAlreadyRequested,
    #[error("static promotion was not found")]
    PromotionNotFound,
    #[error("static promotion revision or state conflict")]
    RevisionConflict,
    #[error("static promotion idempotency key was reused for another command")]
    IdempotencyConflict,
    #[error("static promotion store error: {0}")]
    Store(String),
}

#[derive(Debug)]
pub(crate) struct PinnedBuildEvidence {
    pub(crate) publish_request_id: String,
    pub(crate) module_slug: String,
    pub(crate) module_version: String,
    pub(crate) cargo_package: String,
    pub(crate) entry_type: String,
    pub(crate) source_reference: String,
    pub(crate) source_digest: String,
    pub(crate) dependency_lock_digest: String,
}

pub(crate) async fn load_platform_build_evidence<C: ConnectionTrait>(
    connection: &C,
    release_id: &str,
) -> Result<PinnedBuildEvidence, ModuleStaticPromotionError> {
    let backend = connection.get_database_backend();
    let lock = if backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let release = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_id, slug, version, crate_name, entry_type, status,
                        artifact_origin, checksum_sha256
                 FROM registry_module_releases WHERE id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![release_id.to_owned().into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticPromotionError::ReleaseNotFound)?;
    let publish_request_id: Option<String> =
        release.try_get("", "request_id").map_err(store_error)?;
    let module_slug: String = release.try_get("", "slug").map_err(store_error)?;
    let module_version: String = release.try_get("", "version").map_err(store_error)?;
    let cargo_package: String = release.try_get("", "crate_name").map_err(store_error)?;
    let entry_type: Option<String> = release.try_get("", "entry_type").map_err(store_error)?;
    let status: String = release.try_get("", "status").map_err(store_error)?;
    let artifact_origin: String = release
        .try_get("", "artifact_origin")
        .map_err(store_error)?;
    let checksum: Option<String> = release
        .try_get("", "checksum_sha256")
        .map_err(store_error)?;
    let publish_request_id = publish_request_id
        .filter(|value| !value.trim().is_empty())
        .ok_or(ModuleStaticPromotionError::ReleaseNotEligible)?;
    let checksum = checksum
        .filter(|value| valid_sha256_hex(value))
        .ok_or(ModuleStaticPromotionError::ReleaseNotEligible)?;
    let entry_type = entry_type
        .as_deref()
        .and_then(normalize_native_entry_type)
        .ok_or(ModuleStaticPromotionError::ReleaseNotEligible)?;
    if status != "active"
        || artifact_origin != "platform_built"
        || Version::parse(&module_version).is_err()
        || module_slug.trim().is_empty()
        || !valid_cargo_package(&cargo_package)
    {
        return Err(ModuleStaticPromotionError::ReleaseNotEligible);
    }
    let component_digest = format!("sha256:{checksum}");
    let stage = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT tenant_id, build_request_id, source_digest, component_digest,
                        artifact_manifest_digest
                 FROM registry_publish_build_staging
                 WHERE request_id = {} AND component_digest = {}
                 ORDER BY staged_at DESC LIMIT 1{lock}",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                publish_request_id.clone().into(),
                component_digest.clone().into(),
            ],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticPromotionError::BuildEvidenceMissing)?;
    let tenant_id = uuid_from_row(&stage, "tenant_id", backend).map_err(store_error)?;
    let build_request_id =
        uuid_from_row(&stage, "build_request_id", backend).map_err(store_error)?;
    let staged_source_digest: String = stage.try_get("", "source_digest").map_err(store_error)?;
    let staged_component_digest: String =
        stage.try_get("", "component_digest").map_err(store_error)?;
    let artifact_manifest_digest: String = stage
        .try_get("", "artifact_manifest_digest")
        .map_err(store_error)?;
    configure_tenant_scope(connection, tenant_id)
        .await
        .map_err(store_error)?;
    let build = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request, result, status FROM module_build_requests
                 WHERE request_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(build_request_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticPromotionError::BuildEvidenceMissing)?;
    let build_status: String = build.try_get("", "status").map_err(store_error)?;
    let request_json: serde_json::Value = build.try_get("", "request").map_err(store_error)?;
    let result_json: Option<serde_json::Value> =
        build.try_get("", "result").map_err(store_error)?;
    let request: ModuleBuildRequest = serde_json::from_value(request_json).map_err(store_error)?;
    let result: ModuleBuildResult = serde_json::from_value(
        result_json.ok_or(ModuleStaticPromotionError::BuildEvidenceMissing)?,
    )
    .map_err(store_error)?;
    result
        .validate_against(&request)
        .map_err(|_| ModuleStaticPromotionError::BuildEvidenceMissing)?;
    let publication = result
        .publication
        .as_ref()
        .ok_or(ModuleStaticPromotionError::BuildEvidenceMissing)?;
    if build_status != "completed"
        || !matches!(&result.outcome, ModuleBuildOutcome::Succeeded)
        || request.context.tenant_id != Some(tenant_id)
        || request.request_id != build_request_id
        || request.expected_module_slug != module_slug
        || request.expected_version != module_version
        || !valid_cas_source_reference(&request.source.reference, &request.source.digest)
        || request.source.digest != staged_source_digest
        || result.component_digest.as_deref() != Some(staged_component_digest.as_str())
        || staged_component_digest != component_digest
        || publication.artifact.digest != artifact_manifest_digest
    {
        return Err(ModuleStaticPromotionError::BuildEvidenceMissing);
    }
    Ok(PinnedBuildEvidence {
        publish_request_id,
        module_slug,
        module_version,
        cargo_package,
        entry_type,
        source_reference: request.source.reference,
        source_digest: request.source.digest,
        dependency_lock_digest: request.dependency_policy.lock_digest,
    })
}

async fn reserve_operation(
    transaction: &DatabaseTransaction,
    operation_kind: &str,
    idempotency_key: Uuid,
    request_digest: &str,
    actor_id: Uuid,
) -> Result<Option<ModuleStaticPromotionReceipt>, ModuleStaticPromotionError> {
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_promotion_operations
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
        return Ok(None);
    }
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_kind, request_digest, actor_id, promotion_id,
                        result_revision, result_status,
                        CASE WHEN completed_at IS NULL THEN 0 ELSE 1 END AS completed
                 FROM module_static_promotion_operations WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticPromotionError::IdempotencyConflict)?;
    let stored_kind: String = row.try_get("", "operation_kind").map_err(store_error)?;
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor = uuid_from_row(&row, "actor_id", backend).map_err(store_error)?;
    if stored_kind != operation_kind || stored_digest != request_digest || stored_actor != actor_id
    {
        return Err(ModuleStaticPromotionError::IdempotencyConflict);
    }
    replay_receipt(&row, backend).map(Some)
}

async fn complete_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    promotion_id: Uuid,
    revision: u64,
    status: ModuleStaticPromotionStatus,
) -> Result<(), ModuleStaticPromotionError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_promotion_operations
                 SET promotion_id = {}, result_revision = {}, result_status = {}, completed_at = {}
                 WHERE idempotency_key = {} AND promotion_id IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
                placeholder(backend, 4),
            ),
            vec![
                uuid_value(promotion_id, backend),
                revision_value(revision)?,
                status.as_str().into(),
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticPromotionError::IdempotencyConflict);
    }
    Ok(())
}

fn replay_receipt(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ModuleStaticPromotionReceipt, ModuleStaticPromotionError> {
    let promotion_id = match backend {
        DbBackend::Postgres => row
            .try_get::<Option<Uuid>>("", "promotion_id")
            .map_err(store_error)?,
        _ => row
            .try_get::<Option<String>>("", "promotion_id")
            .map_err(store_error)?
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok()),
    };
    let revision: Option<i64> = row.try_get("", "result_revision").map_err(store_error)?;
    let status: Option<String> = row.try_get("", "result_status").map_err(store_error)?;
    let completed: i64 = row.try_get("", "completed").map_err(store_error)?;
    let promotion_id = promotion_id.ok_or(ModuleStaticPromotionError::IdempotencyConflict)?;
    let revision = revision
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or(ModuleStaticPromotionError::IdempotencyConflict)?;
    let status = ModuleStaticPromotionStatus::parse(
        status
            .as_deref()
            .ok_or(ModuleStaticPromotionError::IdempotencyConflict)?,
    )?;
    if completed != 1 {
        return Err(ModuleStaticPromotionError::IdempotencyConflict);
    }
    Ok(ModuleStaticPromotionReceipt {
        promotion_id,
        status,
        revision,
        created: false,
    })
}

async fn lock_promotion<C: ConnectionTrait>(
    connection: &C,
    promotion_id: Uuid,
) -> Result<ModuleStaticPromotion, ModuleStaticPromotionError> {
    let backend = connection.get_database_backend();
    let lock = if backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT promotion_id, release_id, publish_request_id, module_slug,
                        module_version, cargo_package, entry_type, source_reference, source_digest,
                        dependency_lock_digest, status, revision, requested_by
                 FROM module_static_promotions WHERE promotion_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(promotion_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticPromotionError::PromotionNotFound)?;
    promotion_from_row(&row, backend)
}

pub(crate) async fn load_promotion<C: ConnectionTrait>(
    connection: &C,
    promotion_id: Uuid,
) -> Result<ModuleStaticPromotion, ModuleStaticPromotionError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT promotion_id, release_id, publish_request_id, module_slug,
                        module_version, cargo_package, entry_type, source_reference, source_digest,
                        dependency_lock_digest, status, revision, requested_by
                 FROM module_static_promotions WHERE promotion_id = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(promotion_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticPromotionError::PromotionNotFound)?;
    promotion_from_row(&row, backend)
}

pub(crate) async fn validate_promotion_review<C: ConnectionTrait>(
    connection: &C,
    promotion: &ModuleStaticPromotion,
) -> Result<(), ModuleStaticPromotionError> {
    if promotion.status != ModuleStaticPromotionStatus::Approved {
        return Err(ModuleStaticPromotionError::InvalidEvidence);
    }
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT promotion.approved_by,
                        review.ownership_evidence_reference,
                        review.ownership_evidence_digest,
                        review.dependency_audit_reference,
                        review.dependency_audit_digest,
                        review.test_evidence_reference,
                        review.test_evidence_digest,
                        review.static_review_reference,
                        review.static_review_digest,
                        review.approval_policy_revision,
                        review.review_digest,
                        review.reviewed_by
                 FROM module_static_promotions AS promotion
                 JOIN module_static_promotion_reviews AS review
                   ON review.promotion_id = promotion.promotion_id
                 WHERE promotion.promotion_id = {} AND promotion.status = 'approved'",
                placeholder(backend, 1),
            ),
            vec![uuid_value(promotion.promotion_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticPromotionError::InvalidEvidence)?;
    let approved_by = uuid_from_row(&row, "approved_by", backend).map_err(store_error)?;
    let reviewed_by = uuid_from_row(&row, "reviewed_by", backend).map_err(store_error)?;
    if approved_by.is_nil() || approved_by != reviewed_by || reviewed_by == promotion.requested_by {
        return Err(ModuleStaticPromotionError::InvalidEvidence);
    }
    let evidence = ModuleStaticPromotionApprovalEvidence {
        ownership: ModuleStaticPromotionEvidence {
            reference: row
                .try_get("", "ownership_evidence_reference")
                .map_err(store_error)?,
            digest: row
                .try_get("", "ownership_evidence_digest")
                .map_err(store_error)?,
        },
        dependency_audit: ModuleStaticPromotionEvidence {
            reference: row
                .try_get("", "dependency_audit_reference")
                .map_err(store_error)?,
            digest: row
                .try_get("", "dependency_audit_digest")
                .map_err(store_error)?,
        },
        tests: ModuleStaticPromotionEvidence {
            reference: row
                .try_get("", "test_evidence_reference")
                .map_err(store_error)?,
            digest: row
                .try_get("", "test_evidence_digest")
                .map_err(store_error)?,
        },
        static_review: ModuleStaticPromotionEvidence {
            reference: row
                .try_get("", "static_review_reference")
                .map_err(store_error)?,
            digest: row
                .try_get("", "static_review_digest")
                .map_err(store_error)?,
        },
        policy_revision: row
            .try_get("", "approval_policy_revision")
            .map_err(store_error)?,
    };
    validate_approval_evidence(&evidence)?;
    let stored_digest: String = row.try_get("", "review_digest").map_err(store_error)?;
    if digest_json(&evidence)? != stored_digest {
        return Err(ModuleStaticPromotionError::InvalidEvidence);
    }
    Ok(())
}

fn promotion_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ModuleStaticPromotion, ModuleStaticPromotionError> {
    Ok(ModuleStaticPromotion {
        promotion_id: uuid_from_row(row, "promotion_id", backend).map_err(store_error)?,
        release_id: row.try_get("", "release_id").map_err(store_error)?,
        publish_request_id: row.try_get("", "publish_request_id").map_err(store_error)?,
        module_slug: row.try_get("", "module_slug").map_err(store_error)?,
        module_version: row.try_get("", "module_version").map_err(store_error)?,
        cargo_package: row.try_get("", "cargo_package").map_err(store_error)?,
        entry_type: row.try_get("", "entry_type").map_err(store_error)?,
        source_reference: row.try_get("", "source_reference").map_err(store_error)?,
        source_digest: row.try_get("", "source_digest").map_err(store_error)?,
        dependency_lock_digest: row
            .try_get("", "dependency_lock_digest")
            .map_err(store_error)?,
        status: ModuleStaticPromotionStatus::parse(
            &row.try_get::<String>("", "status").map_err(store_error)?,
        )?,
        revision: positive_revision(row, "revision")?,
        requested_by: uuid_from_row(row, "requested_by", backend).map_err(store_error)?,
    })
}

fn validate_request_command(
    command: &ModuleStaticPromotionRequestCommand,
) -> Result<(), ModuleStaticPromotionError> {
    if command.release_id.trim().is_empty()
        || command.release_id.trim() != command.release_id
        || command.release_id.len() > 256
        || command.release_id.chars().any(char::is_control)
        || !valid_cas_source_reference(&command.source_reference, &command.source_digest)
        || !valid_digest(&command.dependency_lock_digest)
        || command.actor_id.is_nil()
        || command.idempotency_key.is_nil()
    {
        return Err(ModuleStaticPromotionError::InvalidCommand);
    }
    Ok(())
}

fn validate_approval_command(
    command: &ModuleStaticPromotionApprovalCommand,
) -> Result<(), ModuleStaticPromotionError> {
    if command.promotion_id.is_nil()
        || command.expected_revision == 0
        || command.actor_id.is_nil()
        || command.idempotency_key.is_nil()
        || command.evidence.policy_revision.trim().is_empty()
        || command.evidence.policy_revision.trim() != command.evidence.policy_revision
        || command.evidence.policy_revision.len() > MAX_POLICY_REVISION_BYTES
        || command
            .evidence
            .policy_revision
            .chars()
            .any(char::is_control)
    {
        return Err(ModuleStaticPromotionError::InvalidCommand);
    }
    validate_approval_evidence(&command.evidence)
}

fn validate_approval_evidence(
    evidence: &ModuleStaticPromotionApprovalEvidence,
) -> Result<(), ModuleStaticPromotionError> {
    if evidence.policy_revision.trim().is_empty()
        || evidence.policy_revision.trim() != evidence.policy_revision
        || evidence.policy_revision.len() > MAX_POLICY_REVISION_BYTES
        || evidence.policy_revision.chars().any(char::is_control)
    {
        return Err(ModuleStaticPromotionError::InvalidEvidence);
    }
    for evidence in [
        &evidence.ownership,
        &evidence.dependency_audit,
        &evidence.tests,
        &evidence.static_review,
    ] {
        if !valid_reference(&evidence.reference)
            || evidence.reference.trim() != evidence.reference
            || !valid_digest(&evidence.digest)
        {
            return Err(ModuleStaticPromotionError::InvalidEvidence);
        }
    }
    Ok(())
}

pub(crate) fn valid_reference(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_REFERENCE_BYTES
        && !value.chars().any(char::is_control)
}

pub(crate) fn valid_cas_source_reference(reference: &str, digest: &str) -> bool {
    valid_digest(digest) && reference == format!("cas://{digest}")
}

pub(crate) fn valid_cargo_package(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(crate) fn normalize_native_entry_type(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return None;
    }
    let value = value.strip_prefix("crate::").unwrap_or(value);
    if value.is_empty()
        || value.split("::").any(|segment| {
            segment.is_empty()
                || segment == "_"
                || is_rust_keyword(segment)
                || !segment
                    .bytes()
                    .enumerate()
                    .all(|(index, byte)| match index {
                        0 => byte.is_ascii_alphabetic() || byte == b'_',
                        _ => byte.is_ascii_alphanumeric() || byte == b'_',
                    })
        })
    {
        return None;
    }
    Some(value.to_string())
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "Self"
            | "abstract"
            | "as"
            | "async"
            | "await"
            | "become"
            | "box"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "do"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "final"
            | "fn"
            | "for"
            | "gen"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "macro"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "override"
            | "priv"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "try"
            | "type"
            | "typeof"
            | "union"
            | "unsafe"
            | "unsized"
            | "use"
            | "virtual"
            | "where"
            | "while"
            | "yield"
    )
}

pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn revision_value(value: u64) -> Result<sea_orm::Value, ModuleStaticPromotionError> {
    i64::try_from(value)
        .map(Into::into)
        .map_err(|_| ModuleStaticPromotionError::InvalidCommand)
}

fn positive_revision(row: &QueryResult, column: &str) -> Result<u64, ModuleStaticPromotionError> {
    let value: i64 = row.try_get("", column).map_err(store_error)?;
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| ModuleStaticPromotionError::Store("revision is invalid".to_string()))
}

pub(crate) fn digest_json(value: &impl Serialize) -> Result<String, ModuleStaticPromotionError> {
    let bytes = serde_json::to_vec(value).map_err(store_error)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

pub(crate) fn store_error(error: impl std::fmt::Display) -> ModuleStaticPromotionError {
    ModuleStaticPromotionError::Store(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    #[test]
    fn approval_requires_every_review_evidence_digest() {
        let command = ModuleStaticPromotionApprovalCommand {
            promotion_id: Uuid::new_v4(),
            expected_revision: 1,
            evidence: ModuleStaticPromotionApprovalEvidence {
                ownership: ModuleStaticPromotionEvidence {
                    reference: "evidence://ownership".to_string(),
                    digest: digest('a'),
                },
                dependency_audit: ModuleStaticPromotionEvidence {
                    reference: "evidence://dependency-audit".to_string(),
                    digest: digest('b'),
                },
                tests: ModuleStaticPromotionEvidence {
                    reference: "evidence://tests".to_string(),
                    digest: digest('c'),
                },
                static_review: ModuleStaticPromotionEvidence {
                    reference: "evidence://static-review".to_string(),
                    digest: "sha256:UPPERCASE".to_string(),
                },
                policy_revision: digest('e'),
            },
            actor_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        };

        assert!(matches!(
            validate_approval_command(&command),
            Err(ModuleStaticPromotionError::InvalidEvidence)
        ));
    }

    #[test]
    fn native_package_identity_is_strict_and_normalized() {
        assert!(valid_cargo_package("rustok-example_module"));
        assert!(!valid_cargo_package("../example"));
        assert_eq!(
            normalize_native_entry_type("crate::runtime::ExampleModule").as_deref(),
            Some("runtime::ExampleModule")
        );
        assert!(normalize_native_entry_type("runtime::type").is_none());
        assert!(normalize_native_entry_type("runtime::gen").is_none());
        assert!(normalize_native_entry_type("runtime::_").is_none());
        assert!(normalize_native_entry_type("super::ExampleModule").is_none());
        assert!(normalize_native_entry_type("runtime::<ExampleModule>").is_none());
    }

    #[test]
    fn static_promotion_source_must_be_the_exact_cas_digest() {
        let source_digest = digest('a');
        assert!(valid_cas_source_reference(
            &format!("cas://{source_digest}"),
            &source_digest,
        ));
        assert!(!valid_cas_source_reference(
            "git+https://example.invalid/module",
            &source_digest,
        ));
    }
}
