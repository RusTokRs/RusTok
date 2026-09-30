//! Idempotent operation receipts for releases, owners, artifacts, and validations.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement, Value};

use super::*;
use super::helpers::*;
use crate::ModuleCommandContext;

pub(crate) struct ReleaseYankReceipt<'a> {
    pub(crate) release_id: &'a str,
    pub(crate) context: &'a ModuleCommandContext,
    pub(crate) actor_principal: &'a serde_json::Value,
    pub(crate) actor_can_manage_modules: bool,
    pub(crate) reason: &'a str,
    pub(crate) reason_code: &'a str,
}

pub(crate) async fn release_yank_replay(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    receipt: &ReleaseYankReceipt<'_>,
) -> Result<bool, ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let existing = tx
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT CAST(actor_id AS TEXT) AS actor_id, trace_id, \
                 CAST(correlation_id AS TEXT) AS correlation_id, \
                 CAST(actor_principal AS TEXT) AS actor_principal, actor_can_manage_modules, \
                 reason, reason_code FROM registry_release_yank_operations \
                 WHERE release_id = {} AND idempotency_key = {}",
                mark(1),
                mark(2),
            ),
            vec![
                receipt.release_id.to_string().into(),
                registry_uuid_value(receipt.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    let Some(existing) = existing else {
        return Ok(false);
    };
    let stored_actor: serde_json::Value = serde_json::from_str(
        &existing
            .try_get::<String>("", "actor_principal")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    if existing
        .try_get::<String>("", "actor_id")
        .map_err(store_error)?
        != receipt.context.actor_id.to_string()
        || existing
            .try_get::<String>("", "trace_id")
            .map_err(store_error)?
            != receipt.context.trace_id
        || existing
            .try_get::<String>("", "correlation_id")
            .map_err(store_error)?
            != receipt.context.correlation_id.to_string()
        || stored_actor != *receipt.actor_principal
        || existing
            .try_get::<bool>("", "actor_can_manage_modules")
            .map_err(store_error)?
            != receipt.actor_can_manage_modules
        || existing
            .try_get::<String>("", "reason")
            .map_err(store_error)?
            != receipt.reason
        || existing
            .try_get::<String>("", "reason_code")
            .map_err(store_error)?
            != receipt.reason_code
    {
        return Err(ModuleGovernanceError::ReleaseYankIdempotencyConflict);
    }
    Ok(true)
}

pub(crate) async fn record_release_yank_receipt(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    now: &str,
    receipt: &ReleaseYankReceipt<'_>,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO registry_release_yank_operations \
             (operation_id, release_id, idempotency_key, actor_id, trace_id, correlation_id, \
              actor_principal, actor_can_manage_modules, reason, reason_code, resulting_status, \
              committed_at) \
             VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'yanked', {now})",
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
        ),
        vec![
            registry_uuid_value(infrastructure.new_id(), backend),
            receipt.release_id.to_string().into(),
            registry_uuid_value(receipt.context.idempotency_key, backend),
            registry_uuid_value(receipt.context.actor_id, backend),
            receipt.context.trace_id.clone().into(),
            registry_uuid_value(receipt.context.correlation_id, backend),
            Value::Json(Some(Box::new(receipt.actor_principal.clone()))),
            receipt.actor_can_manage_modules.into(),
            receipt.reason.to_string().into(),
            receipt.reason_code.to_string().into(),
        ],
    ))
    .await
    .map_err(store_error)?;
    Ok(())
}

pub(crate) struct OwnerTransferReceipt<'a> {
    pub(crate) slug: &'a str,
    pub(crate) context: &'a ModuleCommandContext,
    pub(crate) previous_owner_principal: &'a serde_json::Value,
    pub(crate) new_owner_principal: &'a serde_json::Value,
    pub(crate) actor_principal: &'a serde_json::Value,
    pub(crate) actor_can_manage_modules: bool,
    pub(crate) reason: &'a str,
    pub(crate) reason_code: &'a str,
}

pub(crate) async fn owner_transfer_replay(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    receipt: &OwnerTransferReceipt<'_>,
) -> Result<bool, ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let existing = tx.query_one_raw(Statement::from_sql_and_values(
        backend,
        format!("SELECT CAST(actor_id AS TEXT) AS actor_id, trace_id, CAST(correlation_id AS TEXT) AS correlation_id, CAST(previous_owner_principal AS TEXT) AS previous_owner_principal, CAST(new_owner_principal AS TEXT) AS new_owner_principal, CAST(actor_principal AS TEXT) AS actor_principal, actor_can_manage_modules, reason, reason_code FROM registry_owner_transfer_operations WHERE slug = {} AND idempotency_key = {}", mark(1), mark(2)),
        vec![receipt.slug.to_string().into(), registry_uuid_value(receipt.context.idempotency_key, backend)],
    )).await.map_err(store_error)?;
    let Some(existing) = existing else {
        return Ok(false);
    };
    let json = |name| -> Result<serde_json::Value, ModuleGovernanceError> {
        serde_json::from_str(&existing.try_get::<String>("", name).map_err(store_error)?)
            .map_err(store_error)
    };
    if existing
        .try_get::<String>("", "actor_id")
        .map_err(store_error)?
        != receipt.context.actor_id.to_string()
        || existing
            .try_get::<String>("", "trace_id")
            .map_err(store_error)?
            != receipt.context.trace_id
        || existing
            .try_get::<String>("", "correlation_id")
            .map_err(store_error)?
            != receipt.context.correlation_id.to_string()
        || json("new_owner_principal")? != *receipt.new_owner_principal
        || json("actor_principal")? != *receipt.actor_principal
        || existing
            .try_get::<bool>("", "actor_can_manage_modules")
            .map_err(store_error)?
            != receipt.actor_can_manage_modules
        || existing
            .try_get::<String>("", "reason")
            .map_err(store_error)?
            != receipt.reason
        || existing
            .try_get::<String>("", "reason_code")
            .map_err(store_error)?
            != receipt.reason_code
    {
        return Err(ModuleGovernanceError::OwnerTransferIdempotencyConflict);
    }
    Ok(true)
}

pub(crate) async fn record_owner_transfer_receipt(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    now: &str,
    receipt: &OwnerTransferReceipt<'_>,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    tx.execute_raw(Statement::from_sql_and_values(backend, format!("INSERT INTO registry_owner_transfer_operations (operation_id, slug, idempotency_key, actor_id, trace_id, correlation_id, previous_owner_principal, new_owner_principal, actor_principal, actor_can_manage_modules, reason, reason_code, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9), mark(10), mark(11), mark(12)), vec![registry_uuid_value(infrastructure.new_id(), backend), receipt.slug.to_string().into(), registry_uuid_value(receipt.context.idempotency_key, backend), registry_uuid_value(receipt.context.actor_id, backend), receipt.context.trace_id.clone().into(), registry_uuid_value(receipt.context.correlation_id, backend), Value::Json(Some(Box::new(receipt.previous_owner_principal.clone()))), Value::Json(Some(Box::new(receipt.new_owner_principal.clone()))), Value::Json(Some(Box::new(receipt.actor_principal.clone()))), receipt.actor_can_manage_modules.into(), receipt.reason.to_string().into(), receipt.reason_code.to_string().into()])).await.map_err(store_error)?;
    Ok(())
}

pub(crate) struct AuthorSignatureEvidenceReceipt<'a> {
    pub(crate) command: &'a ModuleAuthorSignatureEvidenceCommand,
    pub(crate) subject_digest_sha256: String,
}

pub(crate) async fn author_signature_evidence_replay(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    receipt: &AuthorSignatureEvidenceReceipt<'_>,
) -> Result<Option<ModulePublicationEvidenceResult>, ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let existing = tx
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT expected_revision, CAST(actor_id AS TEXT) AS actor_id, trace_id, \
                 CAST(correlation_id AS TEXT) AS correlation_id, CAST(actor_principal AS TEXT) AS actor_principal, \
                 subject_digest_sha256, evidence_reference, signature_digest_sha256, signer_identity, \
                 policy_revision, evidence_id, resulting_revision, recorded \
                 FROM registry_author_signature_evidence_operations \
                 WHERE request_id = {} AND idempotency_key = {}",
                mark(1),
                mark(2),
            ),
            vec![
                receipt.command.request_id.clone().into(),
                registry_uuid_value(receipt.command.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    let Some(existing) = existing else {
        return Ok(None);
    };
    let actor_principal: serde_json::Value = serde_json::from_str(
        &existing
            .try_get::<String>("", "actor_principal")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    if existing
        .try_get::<i64>("", "expected_revision")
        .map_err(store_error)?
        != receipt.command.expected_revision
        || existing
            .try_get::<String>("", "actor_id")
            .map_err(store_error)?
            != receipt.command.context.actor_id.to_string()
        || existing
            .try_get::<String>("", "trace_id")
            .map_err(store_error)?
            != receipt.command.context.trace_id
        || existing
            .try_get::<String>("", "correlation_id")
            .map_err(store_error)?
            != receipt.command.context.correlation_id.to_string()
        || actor_principal != receipt.command.actor_principal
        || existing
            .try_get::<String>("", "subject_digest_sha256")
            .map_err(store_error)?
            != receipt.subject_digest_sha256
        || existing
            .try_get::<String>("", "evidence_reference")
            .map_err(store_error)?
            != receipt.command.evidence_reference
        || existing
            .try_get::<String>("", "signature_digest_sha256")
            .map_err(store_error)?
            != receipt.command.signature_digest_sha256
        || existing
            .try_get::<String>("", "signer_identity")
            .map_err(store_error)?
            != receipt.command.signer_identity
        || existing
            .try_get::<String>("", "policy_revision")
            .map_err(store_error)?
            != receipt.command.policy_revision
    {
        return Err(ModuleGovernanceError::AuthorSignatureEvidenceIdempotencyConflict);
    }
    Ok(Some(ModulePublicationEvidenceResult {
        evidence_id: existing.try_get("", "evidence_id").map_err(store_error)?,
        // A receipt replay never creates another immutable evidence fact,
        // even when the original delivery was the one that recorded it.
        recorded: false,
        request_revision: existing
            .try_get("", "resulting_revision")
            .map_err(store_error)?,
    }))
}

pub(crate) async fn record_author_signature_evidence_receipt(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    now: &str,
    receipt: &AuthorSignatureEvidenceReceipt<'_>,
    result: &ModulePublicationEvidenceResult,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO registry_author_signature_evidence_operations \
             (operation_id, request_id, idempotency_key, expected_revision, actor_id, trace_id, \
              correlation_id, actor_principal, subject_digest_sha256, evidence_reference, \
              signature_digest_sha256, signer_identity, policy_revision, evidence_id, \
              resulting_revision, recorded, committed_at) \
             VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now})",
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
        ),
        vec![
            registry_uuid_value(infrastructure.new_id(), backend),
            receipt.command.request_id.clone().into(),
            registry_uuid_value(receipt.command.context.idempotency_key, backend),
            receipt.command.expected_revision.into(),
            registry_uuid_value(receipt.command.context.actor_id, backend),
            receipt.command.context.trace_id.clone().into(),
            registry_uuid_value(receipt.command.context.correlation_id, backend),
            Value::Json(Some(Box::new(receipt.command.actor_principal.clone()))),
            receipt.subject_digest_sha256.clone().into(),
            receipt.command.evidence_reference.clone().into(),
            receipt.command.signature_digest_sha256.clone().into(),
            receipt.command.signer_identity.clone().into(),
            receipt.command.policy_revision.clone().into(),
            result.evidence_id.clone().into(),
            result.request_revision.into(),
            result.recorded.into(),
        ],
    ))
    .await
    .map_err(store_error)?;
    Ok(())
}

pub(crate) struct PublishArtifactReceipt<'a> {
    pub(crate) command: &'a ModulePublishArtifactAttachCommand,
}

pub(crate) async fn publish_artifact_replay(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    receipt: &PublishArtifactReceipt<'_>,
) -> Result<Option<ModulePublishArtifactAttachResult>, ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let existing = tx
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT expected_revision, CAST(actor_id AS TEXT) AS actor_id, trace_id, \
                 CAST(correlation_id AS TEXT) AS correlation_id, \
                 CAST(actor_principal AS TEXT) AS actor_principal, actor_can_manage_modules, \
                 checksum_sha256, artifact_size, content_type, artifact_storage_key, \
                 previous_storage_key, reuploaded_after_changes_requested \
                 FROM registry_publish_artifact_operations \
                 WHERE request_id = {} AND idempotency_key = {}",
                mark(1),
                mark(2),
            ),
            vec![
                receipt.command.request_id.clone().into(),
                registry_uuid_value(receipt.command.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    let Some(existing) = existing else {
        return Ok(None);
    };
    let stored_actor: serde_json::Value = serde_json::from_str(
        &existing
            .try_get::<String>("", "actor_principal")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    if existing
        .try_get::<i64>("", "expected_revision")
        .map_err(store_error)?
        != receipt.command.expected_revision
        || existing
            .try_get::<String>("", "actor_id")
            .map_err(store_error)?
            != receipt.command.context.actor_id.to_string()
        || existing
            .try_get::<String>("", "trace_id")
            .map_err(store_error)?
            != receipt.command.context.trace_id
        || existing
            .try_get::<String>("", "correlation_id")
            .map_err(store_error)?
            != receipt.command.context.correlation_id.to_string()
        || stored_actor != receipt.command.actor_principal
        || existing
            .try_get::<bool>("", "actor_can_manage_modules")
            .map_err(store_error)?
            != receipt.command.actor_can_manage_modules
        || existing
            .try_get::<String>("", "checksum_sha256")
            .map_err(store_error)?
            != receipt.command.checksum_sha256
        || existing
            .try_get::<i64>("", "artifact_size")
            .map_err(store_error)?
            != receipt.command.artifact_size
        || existing
            .try_get::<String>("", "content_type")
            .map_err(store_error)?
            != receipt.command.content_type
    {
        return Err(ModuleGovernanceError::PublishRequestArtifactIdempotencyConflict);
    }
    Ok(Some(ModulePublishArtifactAttachResult {
        request_id: receipt.command.request_id.clone(),
        artifact_storage_key: existing
            .try_get("", "artifact_storage_key")
            .map_err(store_error)?,
        previous_storage_key: existing
            .try_get("", "previous_storage_key")
            .map_err(store_error)?,
        reuploaded_after_changes_requested: existing
            .try_get("", "reuploaded_after_changes_requested")
            .map_err(store_error)?,
    }))
}

pub(crate) async fn record_publish_artifact_receipt(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    now: &str,
    receipt: &PublishArtifactReceipt<'_>,
    result: &ModulePublishArtifactAttachResult,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let command = receipt.command;
    tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO registry_publish_artifact_operations \
             (operation_id, request_id, idempotency_key, expected_revision, actor_id, trace_id, \
              correlation_id, actor_principal, actor_can_manage_modules, checksum_sha256, \
              artifact_size, content_type, artifact_storage_key, previous_storage_key, \
              reuploaded_after_changes_requested, committed_at) \
             VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now})",
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
        ),
        vec![
            registry_uuid_value(infrastructure.new_id(), backend),
            command.request_id.clone().into(),
            registry_uuid_value(command.context.idempotency_key, backend),
            command.expected_revision.into(),
            registry_uuid_value(command.context.actor_id, backend),
            command.context.trace_id.clone().into(),
            registry_uuid_value(command.context.correlation_id, backend),
            Value::Json(Some(Box::new(command.actor_principal.clone()))),
            command.actor_can_manage_modules.into(),
            command.checksum_sha256.clone().into(),
            command.artifact_size.into(),
            command.content_type.clone().into(),
            result.artifact_storage_key.clone().into(),
            result.previous_storage_key.clone().into(),
            result.reuploaded_after_changes_requested.into(),
        ],
    ))
    .await
    .map_err(store_error)?;
    Ok(())
}

pub(crate) async fn validation_job_enqueue_replay(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    command: &ModuleValidationJobEnqueueCommand,
) -> Result<Option<ModuleValidationJobEnqueueResult>, ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let Some(row) = tx.query_one_raw(Statement::from_sql_and_values(
        backend,
        format!("SELECT expected_revision, CAST(actor_id AS TEXT) AS actor_id, trace_id, CAST(correlation_id AS TEXT) AS correlation_id, CAST(actor_principal AS TEXT) AS actor_principal, allow_rejected_retry, request_status, queued, validation_job_id FROM registry_validation_job_enqueue_operations WHERE request_id = {} AND idempotency_key = {}", mark(1), mark(2)),
        vec![command.request_id.clone().into(), registry_uuid_value(command.context.idempotency_key, backend)],
    )).await.map_err(store_error)? else { return Ok(None); };
    let actor: serde_json::Value = serde_json::from_str(
        &row.try_get::<String>("", "actor_principal")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    if row
        .try_get::<i64>("", "expected_revision")
        .map_err(store_error)?
        != command.expected_revision
        || row.try_get::<String>("", "actor_id").map_err(store_error)?
            != command.context.actor_id.to_string()
        || row.try_get::<String>("", "trace_id").map_err(store_error)? != command.context.trace_id
        || row
            .try_get::<String>("", "correlation_id")
            .map_err(store_error)?
            != command.context.correlation_id.to_string()
        || actor != command.actor_principal
        || row
            .try_get::<bool>("", "allow_rejected_retry")
            .map_err(store_error)?
            != command.allow_rejected_retry
    {
        return Err(ModuleGovernanceError::ValidationJobEnqueueIdempotencyConflict);
    }
    Ok(Some(ModuleValidationJobEnqueueResult {
        request_id: command.request_id.clone(),
        request_status: row.try_get("", "request_status").map_err(store_error)?,
        queued: row.try_get("", "queued").map_err(store_error)?,
        validation_job_id: row.try_get("", "validation_job_id").map_err(store_error)?,
    }))
}

pub(crate) async fn record_validation_job_enqueue_receipt(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    now: &str,
    command: &ModuleValidationJobEnqueueCommand,
    result: &ModuleValidationJobEnqueueResult,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!("INSERT INTO registry_validation_job_enqueue_operations (operation_id, request_id, idempotency_key, expected_revision, actor_id, trace_id, correlation_id, actor_principal, allow_rejected_retry, request_status, queued, validation_job_id, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9), mark(10), mark(11), mark(12)),
        vec![registry_uuid_value(infrastructure.new_id(), backend), command.request_id.clone().into(), registry_uuid_value(command.context.idempotency_key, backend), command.expected_revision.into(), registry_uuid_value(command.context.actor_id, backend), command.context.trace_id.clone().into(), registry_uuid_value(command.context.correlation_id, backend), Value::Json(Some(Box::new(command.actor_principal.clone()))), command.allow_rejected_retry.into(), result.request_status.clone().into(), result.queued.into(), result.validation_job_id.clone().into()],
    )).await.map_err(store_error)?;
    Ok(())
}

pub(crate) struct ValidationStageReportReceipt<'a> {
    pub(crate) request_id: &'a str,
    pub(crate) expected_revision: i64,
    pub(crate) context: &'a ModuleCommandContext,
    pub(crate) actor_principal: &'a serde_json::Value,
    pub(crate) stage_key: &'a str,
    pub(crate) status: &'a str,
    pub(crate) reason_code: Option<&'a str>,
    pub(crate) requeue: bool,
}

pub(crate) async fn validation_stage_report_replay(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    receipt: &ValidationStageReportReceipt<'_>,
) -> Result<bool, ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let existing = tx
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT expected_revision, CAST(actor_id AS TEXT) AS actor_id, trace_id, \\
                 CAST(correlation_id AS TEXT) AS correlation_id, \\
                 CAST(actor_principal AS TEXT) AS actor_principal, stage_key, status, reason_code, requeue \\
                 FROM registry_validation_stage_report_operations \\
                 WHERE request_id = {} AND idempotency_key = {}",
                mark(1),
                mark(2),
            ),
            vec![
                receipt.request_id.to_string().into(),
                registry_uuid_value(receipt.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    let Some(existing) = existing else {
        return Ok(false);
    };
    let stored_actor: serde_json::Value = serde_json::from_str(
        &existing
            .try_get::<String>("", "actor_principal")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    let stored_reason_code: Option<String> =
        existing.try_get("", "reason_code").map_err(store_error)?;
    if existing
        .try_get::<i64>("", "expected_revision")
        .map_err(store_error)?
        != receipt.expected_revision
        || existing
            .try_get::<String>("", "actor_id")
            .map_err(store_error)?
            != receipt.context.actor_id.to_string()
        || existing
            .try_get::<String>("", "trace_id")
            .map_err(store_error)?
            != receipt.context.trace_id
        || existing
            .try_get::<String>("", "correlation_id")
            .map_err(store_error)?
            != receipt.context.correlation_id.to_string()
        || stored_actor != *receipt.actor_principal
        || existing
            .try_get::<String>("", "stage_key")
            .map_err(store_error)?
            != receipt.stage_key
        || existing
            .try_get::<String>("", "status")
            .map_err(store_error)?
            != receipt.status
        || stored_reason_code.as_deref() != receipt.reason_code
        || existing
            .try_get::<bool>("", "requeue")
            .map_err(store_error)?
            != receipt.requeue
    {
        return Err(ModuleGovernanceError::ValidationStageReportIdempotencyConflict);
    }
    Ok(true)
}

pub(crate) async fn record_validation_stage_report_receipt(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    now: &str,
    receipt: &ValidationStageReportReceipt<'_>,
    stage_id: &str,
    resulting_request_revision: i64,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO registry_validation_stage_report_operations \\
             (operation_id, request_id, idempotency_key, expected_revision, actor_id, trace_id, \\
              correlation_id, actor_principal, stage_key, status, reason_code, requeue, stage_id, \\
              resulting_request_revision, committed_at) \\
             VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now})",
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
        ),
        vec![
            registry_uuid_value(infrastructure.new_id(), backend),
            receipt.request_id.to_string().into(),
            registry_uuid_value(receipt.context.idempotency_key, backend),
            receipt.expected_revision.into(),
            registry_uuid_value(receipt.context.actor_id, backend),
            receipt.context.trace_id.clone().into(),
            registry_uuid_value(receipt.context.correlation_id, backend),
            Value::Json(Some(Box::new(receipt.actor_principal.clone()))),
            receipt.stage_key.to_string().into(),
            receipt.status.to_string().into(),
            receipt.reason_code.map(ToString::to_string).into(),
            receipt.requeue.into(),
            stage_id.to_string().into(),
            resulting_request_revision.into(),
        ],
    ))
    .await
    .map_err(store_error)?;
    Ok(())
}

