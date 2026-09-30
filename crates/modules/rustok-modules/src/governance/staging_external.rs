//! External prebuilt artifact staging and security stage reconciliation.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement, TransactionTrait, Value};

use super::*;
use super::helpers::*;
use super::validation_evidence::*;

impl SeaOrmModuleGovernanceService {

    /// Stages an externally built payload only after the owner has recorded
    /// its provenance-policy decision, source-evidence classification, and a
    /// separate quarantine review. This is intentionally distinct from the
    /// platform build path: it never manufactures a build-worker attestation.
    pub async fn stage_external_prebuilt(
        &self,
        command: ModuleExternalPrebuiltStageCommand,
    ) -> Result<ModuleExternalPrebuiltStageResult, ModuleGovernanceError> {
        command.validate()?;
        if !command.actor_can_manage_modules
            || command.actor_principal != command.quarantine_approved_by_principal
        {
            return Err(ModuleGovernanceError::PublishRequestExternalPrebuiltStagingUnauthorized);
        }
        let (source_evidence_kind, source_reference, source_digest, source_absence_reason) =
            match &command.source_evidence {
                ModuleExternalSourceEvidence::Reproducible { reference, digest } => (
                    "reproducible",
                    Some(reference.clone()),
                    Some(digest.clone()),
                    None,
                ),
                ModuleExternalSourceEvidence::Unavailable { reason_code } => {
                    ("unavailable", None, None, Some(reason_code.clone()))
                }
            };
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
                    "SELECT slug, revision, status, artifact_origin, artifact_checksum_sha256 \
                     FROM registry_publish_requests WHERE id = {}{request_lock}",
                    mark(1),
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request.try_get("", "slug").map_err(store_error)?;
        let status: String = request.try_get("", "status").map_err(store_error)?;
        let request_revision: i64 = request.try_get("", "revision").map_err(store_error)?;
        let artifact_origin: String = request
            .try_get("", "artifact_origin")
            .map_err(store_error)?;
        let checksum: Option<String> = request
            .try_get("", "artifact_checksum_sha256")
            .map_err(store_error)?;
        if artifact_origin != ModulePublicationArtifactOrigin::ExternalPrebuilt.as_str()
            || !matches!(status.as_str(), "submitted" | "validating" | "approved")
            || checksum.as_deref() != receipt_digest_sha256(&command.artifact_digest).ok()
        {
            return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
        }

        let staging_id = self.infrastructure.prefixed_id("rpes");
        let inserted = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publish_external_staging \
                     (id, request_id, expected_revision, artifact_digest, source_evidence_kind, source_reference, \
                      source_digest, source_absence_reason, provenance_reference, provenance_digest, \
                      provenance_policy_revision, quarantine_review_reference, quarantine_policy_revision, \
                      quarantine_approved_by_principal, staged_by_principal, actor_id, trace_id, correlation_id, \
                      actor_can_manage_modules, idempotency_key, staged_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now}) \
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
                ),
                vec![
                    staging_id.clone().into(),
                    command.request_id.clone().into(),
                    command.expected_revision.into(),
                    command.artifact_digest.clone().into(),
                    source_evidence_kind.into(),
                    source_reference.clone().into(),
                    source_digest.clone().into(),
                    source_absence_reason.clone().into(),
                    command.provenance_reference.clone().into(),
                    command.provenance_digest.clone().into(),
                    command.provenance_policy_revision.clone().into(),
                    command.quarantine_review_reference.clone().into(),
                    command.quarantine_policy_revision.clone().into(),
                    Value::Json(Some(Box::new(
                        command.quarantine_approved_by_principal.clone(),
                    ))),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    registry_uuid_value(command.context.actor_id, backend),
                    command.context.trace_id.clone().into(),
                    registry_uuid_value(command.context.correlation_id, backend),
                    Value::Bool(Some(command.actor_can_manage_modules)),
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
                        "SELECT id, expected_revision, artifact_digest, source_evidence_kind, source_reference, source_digest, \
                         source_absence_reason, provenance_reference, provenance_digest, \
                         provenance_policy_revision, quarantine_review_reference, \
                         quarantine_policy_revision, \
                         CAST(quarantine_approved_by_principal AS TEXT) AS quarantine_approved_by_principal, \
                         CAST(staged_by_principal AS TEXT) AS staged_by_principal, \
                         CAST(actor_id AS TEXT) AS actor_id, trace_id, \
                         CAST(correlation_id AS TEXT) AS correlation_id, actor_can_manage_modules \
                         FROM registry_publish_external_staging \
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
                    ModuleGovernanceError::Store("external-stage conflict lost its row".to_string())
                })?;
            let existing_id: String = existing.try_get("", "id").map_err(store_error)?;
            let existing_expected_revision: i64 = existing
                .try_get("", "expected_revision")
                .map_err(store_error)?;
            let existing_artifact_digest: String = existing
                .try_get("", "artifact_digest")
                .map_err(store_error)?;
            let existing_source_evidence_kind: String = existing
                .try_get("", "source_evidence_kind")
                .map_err(store_error)?;
            let existing_source_reference: Option<String> = existing
                .try_get("", "source_reference")
                .map_err(store_error)?;
            let existing_source_digest: Option<String> =
                existing.try_get("", "source_digest").map_err(store_error)?;
            let existing_source_absence_reason: Option<String> = existing
                .try_get("", "source_absence_reason")
                .map_err(store_error)?;
            let existing_provenance_reference: String = existing
                .try_get("", "provenance_reference")
                .map_err(store_error)?;
            let existing_provenance_digest: String = existing
                .try_get("", "provenance_digest")
                .map_err(store_error)?;
            let existing_policy_revision: String = existing
                .try_get("", "provenance_policy_revision")
                .map_err(store_error)?;
            let existing_quarantine_reference: String = existing
                .try_get("", "quarantine_review_reference")
                .map_err(store_error)?;
            let existing_quarantine_policy_revision: String = existing
                .try_get("", "quarantine_policy_revision")
                .map_err(store_error)?;
            let existing_quarantine_approver: serde_json::Value = serde_json::from_str(
                &existing
                    .try_get::<String>("", "quarantine_approved_by_principal")
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
            let existing_actor_can_manage_modules: bool = existing
                .try_get("", "actor_can_manage_modules")
                .map_err(store_error)?;
            if existing_expected_revision != command.expected_revision
                || existing_artifact_digest != command.artifact_digest
                || existing_source_evidence_kind != source_evidence_kind
                || existing_source_reference != source_reference
                || existing_source_digest != source_digest
                || existing_source_absence_reason != source_absence_reason
                || existing_provenance_reference != command.provenance_reference
                || existing_provenance_digest != command.provenance_digest
                || existing_policy_revision != command.provenance_policy_revision
                || existing_quarantine_reference != command.quarantine_review_reference
                || existing_quarantine_policy_revision != command.quarantine_policy_revision
                || existing_quarantine_approver != command.quarantine_approved_by_principal
                || existing_actor != command.actor_principal
                || existing_actor_id != command.context.actor_id.to_string()
                || existing_trace_id != command.context.trace_id
                || existing_correlation_id != command.context.correlation_id.to_string()
                || existing_actor_can_manage_modules != command.actor_can_manage_modules
            {
                return Err(ModuleGovernanceError::ExternalPrebuiltStageIdempotencyConflict);
            }
            reconcile_external_prebuilt_security_stage(
                &self.infrastructure,
                &tx,
                backend,
                &command.request_id,
                &command.actor_principal,
            )
            .await?;
            tx.commit().await.map_err(store_error)?;
            return Ok(ModuleExternalPrebuiltStageResult {
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
                 VALUES ({}, {}, {}, NULL, 'external_prebuilt_staged', {}, NULL, {}, {now})",
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
                    "source_evidence_kind": source_evidence_kind,
                    "source_absence_reason": source_absence_reason,
                    "provenance_digest": command.provenance_digest.clone(),
                    "provenance_policy_revision": command.provenance_policy_revision.clone(),
                    "quarantine_policy_revision": command.quarantine_policy_revision.clone(),
                    "command_context": {
                        "scope": "platform",
                        "actor_id": command.context.actor_id,
                        "trace_id": command.context.trace_id,
                        "correlation_id": command.context.correlation_id,
                        "idempotency_key": command.context.idempotency_key,
                        "actor_can_manage_modules": command.actor_can_manage_modules,
                    },
                })))),
            ],
        ))
        .await
        .map_err(store_error)?;
        reconcile_external_prebuilt_security_stage(
            &self.infrastructure,
            &tx,
            backend,
            &command.request_id,
            &command.actor_principal,
        )
        .await?;
        tx.commit().await.map_err(store_error)?;
        Ok(ModuleExternalPrebuiltStageResult {
            staging_id,
            created: true,
            request_revision: request_revision + 1,
        })
    }

}

pub(crate) async fn external_prebuilt_supply_chain_evidence(
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
                 EXISTS (SELECT 1 FROM registry_publish_external_staging AS stage \
                    WHERE stage.request_id = request.id \
                      AND stage.artifact_digest = ('sha256:' || request.artifact_checksum_sha256) \
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
            ),
            vec![request_id.to_string().into()],
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
    if artifact_origin != ModulePublicationArtifactOrigin::ExternalPrebuilt.as_str()
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

pub(crate) async fn reconcile_external_prebuilt_security_stage(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    request_id: &str,
    actor_principal: &serde_json::Value,
) -> Result<(), ModuleGovernanceError> {
    let Some((slug, version)) =
        external_prebuilt_supply_chain_evidence(tx, backend, request_id).await?
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
            detail: "External provenance, quarantine, author-signature, and admitted signature/SBOM/SLSA/vulnerability evidence satisfied the security policy stage.",
            reason_code: "external_supply_chain_evidence_passed",
        },
    )
    .await
}

