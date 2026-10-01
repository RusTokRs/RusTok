//! Validation job work item delivery contracts and invalid item terminalization.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, QueryResult, Statement, Value};

use super::helpers::*;
use super::*;

/// Loads the one immutable descriptor bound to the exact Alloy source stage
/// that a validation lease is about to process. The worker receives this
/// value in its lease and never queries mutable Alloy draft state.
pub(crate) async fn alloy_descriptor_for_validation_work_item(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    request_id: &str,
    slug: &str,
    version: &str,
    artifact_digest: &str,
) -> Result<crate::ModuleArtifactDescriptor, ModuleGovernanceError> {
    let mark = |position| placeholder(backend, position);
    let stage = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT CAST(descriptor AS TEXT) AS descriptor, descriptor_digest \
                 FROM registry_publish_alloy_staging \
                 WHERE request_id = {} AND artifact_digest = {} \
                   AND source_digest = artifact_digest \
                   AND staged_at >= ( \
                       SELECT submitted_at FROM registry_publish_requests \
                       WHERE id = registry_publish_alloy_staging.request_id \
                   ) \
                 ORDER BY staged_at DESC, id DESC LIMIT 1",
                mark(1),
                mark(2),
            ),
            vec![
                request_id.to_string().into(),
                artifact_digest.to_string().into(),
            ],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleGovernanceError::PublishRequestMissingAlloyAuthoredStage)?;
    let descriptor = serde_json::from_str::<crate::ModuleArtifactDescriptor>(
        &stage
            .try_get::<String>("", "descriptor")
            .map_err(store_error)?,
    )
    .map_err(|_| ModuleGovernanceError::InvalidAlloyAuthoredStageCommand)?;
    let descriptor_digest = stage
        .try_get::<String>("", "descriptor_digest")
        .map_err(store_error)?;
    if descriptor.validate().is_err()
        || descriptor.slug != slug
        || descriptor.version != version
        || descriptor.artifact_digest != artifact_digest
        || descriptor.payload_kind != crate::ArtifactPayloadKind::Rhai
        || descriptor.module_kind != crate::ArtifactModuleKind::Optional
        || descriptor.runtime_abi != rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI
        || crate::canonical_artifact_descriptor_digest(&descriptor) != descriptor_digest
    {
        return Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand);
    }
    Ok(descriptor)
}

pub(crate) struct InvalidValidationWorkItem<'a> {
    pub(crate) command: &'a ModuleValidationJobClaimCommand,
    pub(crate) request_id: &'a str,
    pub(crate) expected_request_revision: i64,
    pub(crate) slug: &'a str,
    pub(crate) version: &'a str,
    pub(crate) attempt_number: i32,
    pub(crate) queue_reason: &'a str,
}

pub(crate) async fn terminalize_invalid_validation_work_item(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    item: InvalidValidationWorkItem<'_>,
) -> Result<(), ModuleGovernanceError> {
    let InvalidValidationWorkItem {
        command,
        request_id,
        expected_request_revision,
        slug,
        version,
        attempt_number,
        queue_reason,
    } = item;
    let mark = |n| {
        if backend == DbBackend::Postgres {
            format!("${n}")
        } else {
            format!("?{n}")
        }
    };
    let now = if backend == DbBackend::Postgres {
        "NOW()"
    } else {
        "datetime('now')"
    };
    let errors = serde_json::json!([VALIDATION_WORK_ITEM_INVALID_ERROR]);
    let request_updated = tx
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE registry_publish_requests \
                 SET status = 'rejected', validation_errors = {}, \
                     rejected_by_principal = {}, rejection_reason = {}, \
                     validated_at = {now}, approved_by_principal = NULL, \
                     approved_at = NULL, published_at = NULL, revision = revision + 1, updated_at = {now} \
                 WHERE id = {} AND status = 'validating' AND revision = {}",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                Value::Json(Some(Box::new(errors.clone()))),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                VALIDATION_WORK_ITEM_INVALID_ERROR.to_string().into(),
                request_id.to_string().into(),
                expected_request_revision.into(),
            ],
        ))
        .await
        .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    if request_updated.rows_affected() != 1 {
        return Err(publish_request_revision_conflict(
            tx,
            backend,
            request_id,
            expected_request_revision,
        )
        .await?);
    }
    let job_updated = tx
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE registry_validation_jobs \
                 SET status = 'failed', finished_at = {now}, last_error = {}, \
                     updated_at = {now} \
                 WHERE id = {} AND status = 'running'",
                mark(1),
                mark(2),
            ),
            vec![
                VALIDATION_WORK_ITEM_INVALID_ERROR.to_string().into(),
                command.validation_job_id.clone().into(),
            ],
        ))
        .await
        .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    if job_updated.rows_affected() != 1 {
        return Err(ModuleGovernanceError::ValidationJobNotRunning(
            "concurrently changed".to_string(),
        ));
    }

    let events = [
        (
            "validation_failed",
            serde_json::json!({
                "version": version,
                "status": "rejected",
                "reason": VALIDATION_WORK_ITEM_INVALID_ERROR,
                "errors": errors,
                "automated_checks": [{"key":"delivery_work_item","status":"failed"}],
            }),
        ),
        (
            "validation_job_failed",
            serde_json::json!({
                "job_id": command.validation_job_id.clone(),
                "attempt_number": attempt_number,
                "queue_reason": queue_reason,
                "request_status": "rejected",
                "version": version,
                "error": VALIDATION_WORK_ITEM_INVALID_ERROR,
            }),
        ),
    ];
    for (event_type, details) in events {
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
            ),
            vec![
                infrastructure.prefixed_id("rge").into(),
                slug.to_string().into(),
                request_id.to_string().into(),
                event_type.into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(details))),
            ],
        ))
        .await
        .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    }
    Ok(())
}

pub(crate) fn module_publish_validation_contract_from_row(
    row: &QueryResult,
) -> Result<ModulePublishValidationContract, ModuleGovernanceError> {
    let marketplace: serde_json::Value = serde_json::from_str(
        &row.try_get::<String>("", "marketplace")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
    )
    .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    let ui_packages: serde_json::Value = serde_json::from_str(
        &row.try_get::<String>("", "ui_packages")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
    )
    .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    let marketplace_category = marketplace
        .get("category")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let marketplace_tags = marketplace
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let ui_crate_name = |surface: &str| {
        ui_packages
            .get(surface)
            .and_then(|package| package.get("crate_name"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    };
    Ok(ModulePublishValidationContract {
        slug: row
            .try_get("", "slug")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        version: row
            .try_get("", "version")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        crate_name: row
            .try_get("", "crate_name")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        module_name: row
            .try_get("", "module_name")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        module_description: row
            .try_get("", "module_description")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        ownership: row
            .try_get("", "ownership")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        trust_level: row
            .try_get("", "trust_level")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        license: row
            .try_get("", "license")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        entry_type: row
            .try_get::<Option<String>>("", "entry_type")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        marketplace_category,
        marketplace_tags,
        admin_ui_crate_name: ui_crate_name("admin"),
        storefront_ui_crate_name: ui_crate_name("storefront"),
    })
}

pub(crate) fn validation_warnings_from_row(
    row: &QueryResult,
) -> Result<Vec<String>, ModuleGovernanceError> {
    let warnings: serde_json::Value = serde_json::from_str(
        &row.try_get::<String>("", "validation_warnings")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
    )
    .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    let mut warnings = warnings
        .as_array()
        .ok_or_else(|| {
            ModuleGovernanceError::Store("validation warnings must be an array".to_string())
        })?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|warning| !warning.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    warnings.sort();
    warnings.dedup();
    Ok(warnings)
}
