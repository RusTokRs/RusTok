//! Alloy authored artifact staging and sandbox security stage reconciliation.

use sea_orm::{
    ConnectionTrait, DatabaseTransaction, DbBackend, Statement, TransactionTrait, Value,
};

use super::helpers::*;
use super::mapping::*;
use super::validation_evidence::*;
use super::*;

impl SeaOrmModuleGovernanceService {
    /// Stages one reviewed immutable Alloy source revision for an already
    /// submitted registry artifact. This path is deliberately neither a
    /// platform build nor an external prebuilt: it records the exact Alloy
    /// source/review pair without manufacturing build provenance.
    pub async fn stage_alloy_authored(
        &self,
        command: ModuleAlloyAuthoredStageCommand,
    ) -> Result<ModuleAlloyAuthoredStageResult, ModuleGovernanceError> {
        command.validate()?;
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let request_lock = if backend == sea_orm::DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, revision, status, artifact_origin, artifact_checksum_sha256, \
                            CAST(requested_by_principal AS TEXT) AS requested_by_principal, \
                            CAST(publisher_principal AS TEXT) AS publisher_principal \
                     FROM registry_publish_requests WHERE id = {}{request_lock}",
                    mark(1),
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request.try_get("", "slug").map_err(store_error)?;
        let version: String = request.try_get("", "version").map_err(store_error)?;
        let status: String = request.try_get("", "status").map_err(store_error)?;
        let request_revision: i64 = request.try_get("", "revision").map_err(store_error)?;
        let artifact_origin: String = request
            .try_get("", "artifact_origin")
            .map_err(store_error)?;
        let checksum: Option<String> = request
            .try_get("", "artifact_checksum_sha256")
            .map_err(store_error)?;
        let requested_by_principal = required_json_text(&request, "requested_by_principal")?;
        let publisher_principal = optional_json_text(&request, "publisher_principal")?;
        let owner_principal = if command.actor_can_manage_modules {
            None
        } else {
            governance_owner_principal_for_slug(&tx, backend, &slug, true).await?
        };
        if !governance_actor_can_manage_request_principals(
            &requested_by_principal,
            publisher_principal.as_ref(),
            owner_principal.as_ref(),
            &command.actor_principal,
            command.actor_can_manage_modules,
        ) {
            return Err(ModuleGovernanceError::PublishRequestAlloyAuthoredStagingUnauthorized);
        }
        if artifact_origin != ModulePublicationArtifactOrigin::AlloyAuthored.as_str()
            || !matches!(status.as_str(), "submitted" | "validating" | "approved")
            || checksum.as_deref() != receipt_digest_sha256(&command.artifact_digest).ok()
            || command.descriptor.slug != slug
            || command.descriptor.version != version
        {
            return Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand);
        }
        if let Some(parent_release) = &command.parent_release {
            if parent_release.slug != slug
                || parent_release.digest == command.source_digest
                || !active_published_rhai_parent_exists(&tx, backend, parent_release).await?
            {
                return Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand);
            }
            let parent_version = semver::Version::parse(&parent_release.version)
                .expect("validated artifact release version must parse");
            let child_version = semver::Version::parse(&version)
                .map_err(|_| ModuleGovernanceError::InvalidAlloyAuthoredStageCommand)?;
            if child_version <= parent_version {
                return Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand);
            }
        }

        let staging_id = self.infrastructure.prefixed_id("rpas");
        let descriptor_digest = crate::canonical_artifact_descriptor_digest(&command.descriptor);
        let descriptor = serde_json::to_value(&command.descriptor).map_err(store_error)?;
        let inserted = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publish_alloy_staging \
                     (id, request_id, expected_revision, alloy_tenant_id, alloy_script_id, artifact_digest, source_digest, source_revision, \
                      descriptor, descriptor_digest, parent_release_slug, parent_release_version, parent_release_digest, \
                      review_reference, review_digest, review_policy_revision, \
                      reviewed_by_principal, sandbox_execution_id, sandbox_test_path, sandbox_executor, \
                      sandbox_scenario_digest, sandbox_runtime_abi, sandbox_policy_digest, sandbox_capability_grants, \
                      staged_by_principal, actor_id, trace_id, correlation_id, idempotency_key, staged_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now}) \
                     ON CONFLICT (request_id, idempotency_key) DO NOTHING",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                    mark(8),
                    mark(9),
                    mark(10),
                    mark(11),
                    mark(12),
                    mark(13),
                    mark(14),
                    mark(15),
                    mark(16),
                    mark(17),
                    mark(18),
                    mark(19),
                    mark(20),
                    mark(21),
                    mark(22),
                    mark(23),
                    mark(24),
                    mark(25),
                    mark(26),
                    mark(27),
                    mark(28),
                    mark(29),
                ),
                vec![
                    staging_id.clone().into(),
                    command.request_id.clone().into(),
                    command.expected_revision.into(),
                    registry_uuid_value(command.alloy_tenant_id, backend),
                    registry_uuid_value(command.alloy_script_id, backend),
                    command.artifact_digest.clone().into(),
                    command.source_digest.clone().into(),
                    i64::from(command.source_revision).into(),
                    Value::Json(Some(Box::new(descriptor))),
                    descriptor_digest.clone().into(),
                    command
                        .parent_release
                        .as_ref()
                        .map(|parent| parent.slug.clone())
                        .into(),
                    command
                        .parent_release
                        .as_ref()
                        .map(|parent| parent.version.clone())
                        .into(),
                    command
                        .parent_release
                        .as_ref()
                        .map(|parent| parent.digest.clone())
                        .into(),
                    command.review_reference.clone().into(),
                    command.review_digest.clone().into(),
                    command.review_policy_revision.clone().into(),
                    Value::Json(Some(Box::new(command.reviewed_by_principal.clone()))),
                    registry_uuid_value(command.sandbox_execution_id, backend),
                    command.sandbox_test_path.clone().into(),
                    command.sandbox_executor.clone().into(),
                    command.sandbox_scenario_digest.clone().into(),
                    command.sandbox_runtime_abi.clone().into(),
                    command.sandbox_policy_digest.clone().into(),
                    i64::from(command.sandbox_capability_grants).into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    registry_uuid_value(command.context.actor_id, backend),
                    command.context.trace_id.clone().into(),
                    registry_uuid_value(command.context.correlation_id, backend),
                    registry_uuid_value(command.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(store_error)?;
        if inserted.rows_affected() == 0 {
            let existing = tx
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id, expected_revision, CAST(alloy_tenant_id AS TEXT) AS alloy_tenant_id, \
                         CAST(alloy_script_id AS TEXT) AS alloy_script_id, \
                         artifact_digest, source_digest, source_revision, \
                         CAST(descriptor AS TEXT) AS descriptor, descriptor_digest, \
                         parent_release_slug, parent_release_version, parent_release_digest, \
                         review_reference, \
                         review_digest, review_policy_revision, \
                         CAST(sandbox_execution_id AS TEXT) AS sandbox_execution_id, \
                         sandbox_test_path, sandbox_executor, sandbox_scenario_digest, sandbox_runtime_abi, sandbox_policy_digest, \
                         sandbox_capability_grants, \
                         CAST(reviewed_by_principal AS TEXT) AS reviewed_by_principal, \
                         CAST(staged_by_principal AS TEXT) AS staged_by_principal, \
                         CAST(actor_id AS TEXT) AS actor_id, trace_id, \
                         CAST(correlation_id AS TEXT) AS correlation_id \
                         FROM registry_publish_alloy_staging \
                         WHERE request_id = {} AND idempotency_key = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        registry_uuid_value(command.context.idempotency_key, backend),
                    ],
                ))
                .await
                .map_err(store_error)?
                .ok_or_else(|| {
                    ModuleGovernanceError::Store("Alloy stage conflict lost its row".to_string())
                })?;
            let existing_id: String = existing.try_get("", "id").map_err(store_error)?;
            let existing_expected_revision: i64 = existing
                .try_get("", "expected_revision")
                .map_err(store_error)?;
            let existing_tenant_id: String = existing
                .try_get("", "alloy_tenant_id")
                .map_err(store_error)?;
            let existing_script_id: String = existing
                .try_get("", "alloy_script_id")
                .map_err(store_error)?;
            let existing_artifact_digest: String = existing
                .try_get("", "artifact_digest")
                .map_err(store_error)?;
            let existing_source_digest: String =
                existing.try_get("", "source_digest").map_err(store_error)?;
            let existing_source_revision: i64 = existing
                .try_get("", "source_revision")
                .map_err(store_error)?;
            let existing_descriptor: crate::ModuleArtifactDescriptor = serde_json::from_str(
                &existing
                    .try_get::<String>("", "descriptor")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            let existing_descriptor_digest: String = existing
                .try_get("", "descriptor_digest")
                .map_err(store_error)?;
            let existing_parent_release = parent_release_from_stage_row(&existing)?;
            let existing_review_reference: String = existing
                .try_get("", "review_reference")
                .map_err(store_error)?;
            let existing_review_digest: String =
                existing.try_get("", "review_digest").map_err(store_error)?;
            let existing_policy_revision: String = existing
                .try_get("", "review_policy_revision")
                .map_err(store_error)?;
            let existing_sandbox_execution_id: String = existing
                .try_get("", "sandbox_execution_id")
                .map_err(store_error)?;
            let existing_sandbox_test_path: String = existing
                .try_get("", "sandbox_test_path")
                .map_err(store_error)?;
            let existing_sandbox_executor: String = existing
                .try_get("", "sandbox_executor")
                .map_err(store_error)?;
            let existing_sandbox_scenario_digest: String = existing
                .try_get("", "sandbox_scenario_digest")
                .map_err(store_error)?;
            let existing_sandbox_runtime_abi: String = existing
                .try_get("", "sandbox_runtime_abi")
                .map_err(store_error)?;
            let existing_sandbox_policy_digest: String = existing
                .try_get("", "sandbox_policy_digest")
                .map_err(store_error)?;
            let existing_sandbox_capability_grants: i64 = existing
                .try_get("", "sandbox_capability_grants")
                .map_err(store_error)?;
            let existing_reviewer: serde_json::Value = serde_json::from_str(
                &existing
                    .try_get::<String>("", "reviewed_by_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            let existing_actor: serde_json::Value = serde_json::from_str(
                &existing
                    .try_get::<String>("", "staged_by_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            let existing_actor_id: String =
                existing.try_get("", "actor_id").map_err(store_error)?;
            let existing_trace_id: String =
                existing.try_get("", "trace_id").map_err(store_error)?;
            let existing_correlation_id: String = existing
                .try_get("", "correlation_id")
                .map_err(store_error)?;
            if existing_expected_revision != command.expected_revision
                || existing_tenant_id != command.alloy_tenant_id.to_string()
                || existing_script_id != command.alloy_script_id.to_string()
                || existing_artifact_digest != command.artifact_digest
                || existing_source_digest != command.source_digest
                || existing_source_revision != i64::from(command.source_revision)
                || existing_descriptor != command.descriptor
                || existing_descriptor_digest != descriptor_digest
                || existing_parent_release != command.parent_release
                || existing_review_reference != command.review_reference
                || existing_review_digest != command.review_digest
                || existing_policy_revision != command.review_policy_revision
                || existing_sandbox_execution_id != command.sandbox_execution_id.to_string()
                || existing_sandbox_test_path != command.sandbox_test_path
                || existing_sandbox_executor != command.sandbox_executor
                || existing_sandbox_scenario_digest != command.sandbox_scenario_digest
                || existing_sandbox_runtime_abi != command.sandbox_runtime_abi
                || existing_sandbox_policy_digest != command.sandbox_policy_digest
                || existing_sandbox_capability_grants
                    != i64::from(command.sandbox_capability_grants)
                || existing_reviewer != command.reviewed_by_principal
                || existing_actor != command.actor_principal
                || existing_actor_id != command.context.actor_id.to_string()
                || existing_trace_id != command.context.trace_id
                || existing_correlation_id != command.context.correlation_id.to_string()
            {
                return Err(ModuleGovernanceError::AlloyAuthoredStageIdempotencyConflict);
            }
            reconcile_alloy_authored_security_stage(
                &self.infrastructure,
                &tx,
                backend,
                &command.request_id,
                &command.actor_principal,
            )
            .await?;
            tx.commit().await.map_err(store_error)?;
            return Ok(ModuleAlloyAuthoredStageResult {
                staging_id: existing_id,
                created: false,
                request_revision,
            });
        }
        let request_updated = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests SET revision = revision + 1, updated_at = {now} \
                     WHERE id = {} AND revision = {}",
                    mark(1),
                    mark(2),
                ),
                vec![
                    command.request_id.clone().into(),
                    command.expected_revision.into(),
                ],
            ))
            .await
            .map_err(store_error)?;
        if request_updated.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &command.request_id,
                command.expected_revision,
            )
            .await?);
        }
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'alloy_authored_staged', {}, NULL, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                self.infrastructure.prefixed_id("rge").into(),
                slug.clone().into(),
                command.request_id.clone().into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "artifact_digest": command.artifact_digest.clone(),
                    "alloy_tenant_id": command.alloy_tenant_id,
                    "alloy_script_id": command.alloy_script_id,
                    "source_digest": command.source_digest.clone(),
                    "source_revision": command.source_revision,
                    "descriptor_digest": descriptor_digest,
                    "parent_release": command.parent_release.clone(),
                    "review_digest": command.review_digest.clone(),
                    "review_policy_revision": command.review_policy_revision.clone(),
                    "sandbox_execution_id": command.sandbox_execution_id,
                    "sandbox_test_path": command.sandbox_test_path.clone(),
                    "sandbox_executor": command.sandbox_executor.clone(),
                    "sandbox_scenario_digest": command.sandbox_scenario_digest.clone(),
                    "sandbox_runtime_abi": command.sandbox_runtime_abi.clone(),
                    "sandbox_policy_digest": command.sandbox_policy_digest.clone(),
                    "sandbox_capability_grants": command.sandbox_capability_grants,
                    "command_context": {
                        "scope": "tenant",
                        "tenant_id": command.context.tenant_id,
                        "actor_id": command.context.actor_id,
                        "trace_id": command.context.trace_id,
                        "correlation_id": command.context.correlation_id,
                        "idempotency_key": command.context.idempotency_key,
                    },
                })))),
            ],
        ))
        .await
        .map_err(store_error)?;
        reconcile_alloy_authored_security_stage(
            &self.infrastructure,
            &tx,
            backend,
            &command.request_id,
            &command.actor_principal,
        )
        .await?;
        tx.commit().await.map_err(store_error)?;
        Ok(ModuleAlloyAuthoredStageResult {
            staging_id,
            created: true,
            request_revision: request_revision + 1,
        })
    }
}

pub(crate) async fn alloy_authored_supply_chain_evidence(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    request_id: &str,
) -> Result<Option<(String, String)>, ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let Some(row) = tx
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request.slug, request.version, request.status, request.artifact_origin, \
                 EXISTS (SELECT 1 FROM registry_publish_alloy_staging AS stage \
                    WHERE stage.request_id = request.id \
                      AND stage.artifact_digest = ('sha256:' || request.artifact_checksum_sha256) \
                      AND stage.source_digest = stage.artifact_digest \
                      AND stage.sandbox_execution_id IS NOT NULL \
                      AND stage.sandbox_test_path = {} \
                      AND stage.sandbox_executor = 'rhai' \
                      AND stage.sandbox_scenario_digest = {} \
                      AND stage.sandbox_runtime_abi = {} \
                      AND stage.sandbox_policy_digest LIKE 'sha256:%' \
                      AND stage.sandbox_capability_grants = 0 \
                      AND stage.staged_at >= request.submitted_at) AS staging_current, \
                 EXISTS (SELECT 1 FROM registry_publication_evidence AS author \
                    WHERE author.request_id = request.id \
                      AND author.authority = 'author_signature' \
                      AND author.subject_digest_sha256 = request.artifact_checksum_sha256 \
                      AND author.signature_digest_sha256 IS NOT NULL \
                      AND author.created_at >= request.submitted_at) AS author_signature_current, \
                 EXISTS (SELECT 1 FROM registry_publication_evidence AS admission \
                    WHERE admission.request_id = request.id \
                      AND admission.authority = 'platform_admission' \
                      AND admission.subject_digest_sha256 = request.artifact_checksum_sha256 \
                      AND admission.created_at >= request.submitted_at) AS platform_admission_current \
                 FROM registry_publish_requests AS request WHERE request.id = {} \
                   AND request.submitted_at IS NOT NULL \
                   AND request.artifact_checksum_sha256 IS NOT NULL",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
            ),
            vec![
                ALLOY_PUBLICATION_SMOKE_TEST_PATH.into(),
                alloy_publication_smoke_scenario_digest().into(),
                rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.into(),
                request_id.to_string().into(),
            ],
        ))
        .await
        .map_err(store_error)?
    else {
        return Ok(None);
    };
    let artifact_origin: String = row.try_get("", "artifact_origin").map_err(store_error)?;
    let status: String = row.try_get("", "status").map_err(store_error)?;
    let staging_current: bool = row.try_get("", "staging_current").map_err(store_error)?;
    let author_signature_current: bool = row
        .try_get("", "author_signature_current")
        .map_err(store_error)?;
    let platform_admission_current: bool = row
        .try_get("", "platform_admission_current")
        .map_err(store_error)?;
    if artifact_origin != ModulePublicationArtifactOrigin::AlloyAuthored.as_str()
        || !matches!(status.as_str(), "approved" | "published")
        || !staging_current
        || !author_signature_current
        || !platform_admission_current
    {
        return Ok(None);
    }
    Ok(Some((
        row.try_get("", "slug").map_err(store_error)?,
        row.try_get("", "version").map_err(store_error)?,
    )))
}

pub(crate) fn parent_release_from_stage_row(
    row: &sea_orm::QueryResult,
) -> Result<Option<crate::ArtifactReleaseRef>, ModuleGovernanceError> {
    let slug = row
        .try_get::<Option<String>>("", "parent_release_slug")
        .map_err(store_error)?;
    let version = row
        .try_get::<Option<String>>("", "parent_release_version")
        .map_err(store_error)?;
    let digest = row
        .try_get::<Option<String>>("", "parent_release_digest")
        .map_err(store_error)?;
    match (slug, version, digest) {
        (None, None, None) => Ok(None),
        (Some(slug), Some(version), Some(digest)) => {
            let parent = crate::ArtifactReleaseRef {
                slug,
                version,
                digest,
            };
            parent.validate().map_err(|_| {
                ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract
            })?;
            Ok(Some(parent))
        }
        _ => Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract),
    }
}

pub(crate) async fn active_published_rhai_parent_exists(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    parent: &crate::ArtifactReleaseRef,
) -> Result<bool, ModuleGovernanceError> {
    let checksum = receipt_digest_sha256(&parent.digest)
        .map_err(|_| ModuleGovernanceError::InvalidAlloyAuthoredStageCommand)?;
    let mark = |position| placeholder(backend, position);
    tx.query_one_raw(Statement::from_sql_and_values(
        backend,
        format!(
            "SELECT 1 FROM registry_module_releases AS release \
             INNER JOIN registry_module_release_artifacts AS artifact \
               ON artifact.release_id = release.id \
             INNER JOIN registry_publish_platform_admissions AS admission \
               ON admission.request_id = release.request_id \
             WHERE release.slug = {} AND release.version = {} \
               AND release.checksum_sha256 = {} \
               AND release.status = 'active' \
               AND release.artifact_origin = 'alloy_authored' \
               AND admission.runtime_kind = 'rhai' \
               LIMIT 1",
            mark(1),
            mark(2),
            mark(3),
        ),
        vec![
            parent.slug.clone().into(),
            parent.version.clone().into(),
            checksum.to_string().into(),
        ],
    ))
    .await
    .map_err(store_error)
    .map(|row| row.is_some())
}

pub(crate) async fn reconcile_alloy_authored_security_stage(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    request_id: &str,
    actor_principal: &serde_json::Value,
) -> Result<(), ModuleGovernanceError> {
    let Some((slug, version)) =
        alloy_authored_supply_chain_evidence(tx, backend, request_id).await?
    else {
        return Ok(());
    };
    pass_owner_evidence_validation_stage(
        infrastructure,
        tx,
        backend,
        OwnerEvidenceStage {
            request_id,
            slug: &slug,
            version: &version,
            stage_key: "security_policy_review",
            actor_principal,
            detail: "Reviewed Alloy source, capability-free production sandbox smoke, author-signature, and exact platform-admission evidence satisfied the security policy stage.",
            reason_code: "alloy_sandbox_evidence_passed",
        },
    )
    .await
}
