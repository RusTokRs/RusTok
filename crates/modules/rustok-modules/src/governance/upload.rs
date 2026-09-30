//! Publish artifact upload slots and request initialization.

use sea_orm::{ConnectionTrait, Statement, TransactionTrait, Value};

use super::*;

impl SeaOrmModuleGovernanceService {

    /// Authorizes and derives the immutable content-addressed destination for
    /// one publish-artifact upload. The host may hash and deliver bytes, but
    /// cannot select a request state, grant itself upload authority, or
    /// construct an object key.
    pub async fn prepare_publish_artifact_upload(
        &self,
        command: &ModulePublishArtifactAttachCommand,
    ) -> Result<ModuleGovernancePublishArtifactUploadSlot, ModuleGovernanceError> {
        command.validate()?;
        let actor = ModuleGovernanceActorContext {
            principal: command.actor_principal.clone(),
            can_manage_modules: command.actor_can_manage_modules,
        };
        let snapshot = self
            .publish_request_status_snapshot(&command.request_id, Some(&actor))
            .await?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let owner_binding = self.owner_binding_snapshot(&snapshot.request.slug).await?;
        let can_manage = governance_actor_can_manage_request(
            &snapshot.request,
            owner_binding.as_ref(),
            Some(&actor),
        );
        let artifact_storage_key = registry_publish_artifact_storage_key(&command.checksum_sha256)?;

        if matches!(
            snapshot.request.status.as_str(),
            "draft" | "changes_requested"
        ) {
            if !can_manage {
                return Err(ModuleGovernanceError::PublishRequestArtifactUploadUnauthorized);
            }
            if command.expected_revision != snapshot.request.revision {
                return Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                    expected: command.expected_revision,
                    current: snapshot.request.revision,
                });
            }
            return Ok(ModuleGovernancePublishArtifactUploadSlot {
                request_id: snapshot.request.id,
                artifact_storage_key,
                artifact_already_attached: false,
            });
        }

        if !can_manage {
            return Err(ModuleGovernanceError::PublishRequestArtifactUploadUnauthorized);
        }
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT artifact_storage_key, artifact_checksum_sha256, artifact_size, artifact_content_type \
                     FROM registry_publish_requests WHERE id = {} LIMIT 1",
                    placeholder(backend, 1)
                ),
                vec![snapshot.request.id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let existing_storage_key = optional_column::<String>(&row, "artifact_storage_key")?;
        let existing_checksum = optional_column::<String>(&row, "artifact_checksum_sha256")?;
        let existing_size = optional_column::<i64>(&row, "artifact_size")?;
        let existing_content_type = optional_column::<String>(&row, "artifact_content_type")?;
        if existing_storage_key.as_deref() == Some(artifact_storage_key.as_str())
            && existing_checksum.as_deref() == Some(command.checksum_sha256.as_str())
            && existing_size == Some(command.artifact_size)
            && existing_content_type.as_deref() == Some(command.content_type.as_str())
        {
            return Ok(ModuleGovernancePublishArtifactUploadSlot {
                request_id: snapshot.request.id,
                artifact_storage_key,
                artifact_already_attached: true,
            });
        }

        Err(ModuleGovernanceError::PublishRequestArtifactReplayConflict)
    }

    /// Creates an authorized draft publish request, default-locale metadata,
    /// and audit fact atomically. The owner verifies the current durable owner
    /// binding before writing or replaying the request.
    pub async fn create_publish_request(
        &self,
        command: ModulePublishRequestCreateCommand,
    ) -> Result<String, ModuleGovernanceError> {
        let warnings = command.validation_warnings()?;
        if !valid_command_context_actor(&command.context, &command.actor_principal) {
            return Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand);
        }
        let (request_id, command_digest) = publish_request_create_identity(&command)?;
        let marketplace_content =
            ModuleMarketplaceContentProjection::try_new(&command.name, &command.description)
                .map_err(|_| ModuleGovernanceError::InvalidPublishRequestCreateCommand)?;
        let default_locale = rustok_api::normalize_locale_tag(&command.default_locale)
            .ok_or(ModuleGovernanceError::InvalidPublishRequestCreateCommand)?;
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let owner_principal = if command.actor_can_manage_modules {
            None
        } else {
            governance_owner_principal_for_slug(&tx, backend, &command.slug, true).await?
        };
        if !governance_actor_can_create_publish_request(
            owner_principal.as_ref(),
            &command.actor_principal,
            command.actor_can_manage_modules,
        ) {
            return Err(ModuleGovernanceError::PublishRequestCreationUnauthorized);
        }
        let existing = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT command_digest, CAST(actor_id AS TEXT) AS actor_id, trace_id, \
                     CAST(correlation_id AS TEXT) AS correlation_id, CAST(actor_principal AS TEXT) AS actor_principal, \
                     actor_can_manage_modules FROM registry_publish_request_create_operations \
                     WHERE request_id = {} AND idempotency_key = {}",
                    mark(1), mark(2),
                ),
                vec![request_id.clone().into(), registry_uuid_value(command.context.idempotency_key, backend)],
            ))
            .await
            .map_err(store_error)?;
        if let Some(existing) = existing {
            let actor: serde_json::Value = serde_json::from_str(
                &existing
                    .try_get::<String>("", "actor_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            if existing
                .try_get::<String>("", "command_digest")
                .map_err(store_error)?
                != command_digest
                || existing
                    .try_get::<String>("", "actor_id")
                    .map_err(store_error)?
                    != command.context.actor_id.to_string()
                || existing
                    .try_get::<String>("", "trace_id")
                    .map_err(store_error)?
                    != command.context.trace_id
                || existing
                    .try_get::<String>("", "correlation_id")
                    .map_err(store_error)?
                    != command.context.correlation_id.to_string()
                || actor != command.actor_principal
                || existing
                    .try_get::<bool>("", "actor_can_manage_modules")
                    .map_err(store_error)?
                    != command.actor_can_manage_modules
            {
                return Err(ModuleGovernanceError::PublishRequestCreationConflict);
            }
            tx.commit().await.map_err(store_error)?;
            return Ok(request_id);
        }
        let already_created = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_publish_requests WHERE id = {}",
                    mark(1)
                ),
                vec![request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?;
        if already_created.is_some() {
            return Err(ModuleGovernanceError::PublishRequestCreationConflict);
        }
        let active_release = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_module_releases WHERE slug = {} AND version = {} AND status = 'active' LIMIT 1",
                    mark(1), mark(2)
                ),
                vec![command.slug.clone().into(), command.version.clone().into()],
            ))
            .await
            .map_err(store_error)?;
        if active_release.is_some() {
            return Err(ModuleGovernanceError::PublishRequestReleaseAlreadyActive {
                slug: command.slug,
                version: command.version,
            });
        }
        let warnings = dedupe_validation_messages(warnings);
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_publish_requests (id, slug, version, crate_name, default_locale, ownership, trust_level, license, entry_type, artifact_origin, marketplace, ui_packages, status, requested_by_principal, publisher_principal, approved_by_principal, rejected_by_principal, rejection_reason, changes_requested_by_principal, changes_requested_reason, changes_requested_reason_code, changes_requested_at, held_by_principal, held_reason, held_reason_code, held_at, held_from_status, validation_warnings, validation_errors, artifact_storage_key, artifact_checksum_sha256, artifact_size, artifact_content_type, submitted_at, validated_at, approved_at, published_at, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'draft', {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9), mark(10), mark(11), mark(12), mark(13), mark(14), mark(15), mark(16)),
            vec![request_id.clone().into(), command.slug.clone().into(), command.version.clone().into(), command.crate_name.into(), default_locale.clone().into(), command.ownership.into(), command.trust_level.into(), command.license.into(), command.entry_type.into(), command.artifact_origin.as_str().into(), Value::Json(Some(Box::new(command.marketplace))), Value::Json(Some(Box::new(command.ui_packages))), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))), Value::Json(Some(Box::new(serde_json::json!([]))))],
        )).await.map_err(store_error)?;
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_publish_request_create_operations (operation_id, request_id, idempotency_key, command_digest, actor_id, trace_id, correlation_id, actor_principal, actor_can_manage_modules, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9)),
            vec![registry_uuid_value(self.infrastructure.new_id(), backend), request_id.clone().into(), registry_uuid_value(command.context.idempotency_key, backend), command_digest.clone().into(), registry_uuid_value(command.context.actor_id, backend), command.context.trace_id.clone().into(), registry_uuid_value(command.context.correlation_id, backend), Value::Json(Some(Box::new(command.actor_principal.clone()))), command.actor_can_manage_modules.into()],
        )).await.map_err(store_error)?;
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_publish_request_translations (request_id, locale, name, description, created_at, updated_at) VALUES ({}, {}, {}, {}, {now}, {now})", mark(1), mark(2), mark(3), mark(4)),
            vec![request_id.clone().into(), default_locale.into(), marketplace_content.name.into(), marketplace_content.description.into()],
        )).await.map_err(store_error)?;
        let details = serde_json::json!({
            "version": command.version,
            "status": "draft",
            "artifact_origin": command.artifact_origin.as_str(),
            "command_digest": command_digest,
            "warnings": warnings,
        });
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'request_created', {}, {}, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
            vec![self.infrastructure.prefixed_id("rge").into(), command.slug.into(), request_id.clone().into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(command.actor_principal))), Value::Json(Some(Box::new(details)))],
        )).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(request_id)
    }

    /// Attaches bytes already placed in an owner-issued slot to an eligible
    /// request. The command is re-authorized immediately before the durable
    /// state transition, so a slot preflight cannot become a write bypass.
    pub async fn attach_publish_artifact(
        &self,
        command: ModulePublishArtifactAttachCommand,
    ) -> Result<ModulePublishArtifactAttachResult, ModuleGovernanceError> {
        let upload_slot = self.prepare_publish_artifact_upload(&command).await?;
        let artifact_storage_key = upload_slot.artifact_storage_key;
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let request_lock = if backend == DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let receipt = PublishArtifactReceipt { command: &command };
        let request = tx.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT slug, version, revision, status, artifact_storage_key, artifact_checksum_sha256, artifact_size, artifact_content_type, CAST(validation_warnings AS TEXT) AS validation_warnings, CAST(requested_by_principal AS TEXT) AS requested_by_principal FROM registry_publish_requests WHERE id = {}{request_lock}", mark(1)),
            vec![command.request_id.clone().into()],
        )).await.map_err(store_error)?.ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        if let Some(result) = publish_artifact_replay(&tx, backend, &receipt).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(result);
        }
        let status: String = request.try_get("", "status").map_err(store_error)?;
        let reuploaded = status == "changes_requested";
        let existing_storage_key: Option<String> = request
            .try_get("", "artifact_storage_key")
            .map_err(store_error)?;
        if matches!(
            status.as_str(),
            "submitted" | "validating" | "approved" | "published"
        ) && existing_storage_key.as_deref() == Some(artifact_storage_key.as_str())
            && request
                .try_get::<Option<String>>("", "artifact_checksum_sha256")
                .map_err(store_error)?
                .as_deref()
                == Some(command.checksum_sha256.as_str())
            && request
                .try_get::<Option<i64>>("", "artifact_size")
                .map_err(store_error)?
                == Some(command.artifact_size)
            && request
                .try_get::<Option<String>>("", "artifact_content_type")
                .map_err(store_error)?
                .as_deref()
                == Some(command.content_type.as_str())
        {
            return Err(ModuleGovernanceError::PublishRequestArtifactReplayConflict);
        }
        if status != "draft" && !reuploaded {
            return Err(ModuleGovernanceError::PublishRequestCannotAttachArtifact(
                status,
            ));
        }
        let current_revision: i64 = request.try_get("", "revision").map_err(store_error)?;
        if command.expected_revision != current_revision {
            return Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                expected: command.expected_revision,
                current: current_revision,
            });
        }
        let slug: String = request.try_get("", "slug").map_err(store_error)?;
        let version: String = request.try_get("", "version").map_err(store_error)?;
        let previous_storage_key = existing_storage_key;
        let existing_warnings: String = request
            .try_get("", "validation_warnings")
            .map_err(store_error)?;
        let requested_by: String = request
            .try_get("", "requested_by_principal")
            .map_err(store_error)?;
        let mut warnings = if reuploaded {
            Vec::new()
        } else {
            serde_json::from_str::<serde_json::Value>(&existing_warnings)
                .ok()
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect()
        };
        let actor = validation_stage_actor_label(&command.actor_principal)?;
        let requested_by_label = serde_json::from_str::<serde_json::Value>(&requested_by)
            .ok()
            .and_then(|value| {
                value
                    .get("id")
                    .or_else(|| value.get("subject"))
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
            })
            .unwrap_or(requested_by);
        if actor != requested_by_label {
            warnings.push(format!("Artifact was uploaded by '{actor}' for publish request originally created by '{requested_by_label}'."));
        }
        let warnings = dedupe_validation_messages(warnings);
        let request_updated = tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("UPDATE registry_publish_requests SET status = 'submitted', artifact_storage_key = {}, artifact_checksum_sha256 = {}, artifact_size = {}, artifact_content_type = {}, submitted_at = {now}, validation_warnings = {}, validation_errors = {}, approved_by_principal = NULL, rejected_by_principal = NULL, rejection_reason = NULL, validated_at = NULL, approved_at = NULL, published_at = NULL, revision = revision + 1, updated_at = {now} WHERE id = {} AND status IN ('draft', 'changes_requested') AND revision = {}", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8)),
            vec![artifact_storage_key.clone().into(), command.checksum_sha256.clone().into(), command.artifact_size.into(), command.content_type.clone().into(), Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))), Value::Json(Some(Box::new(serde_json::json!([])))), command.request_id.clone().into(), command.expected_revision.into()],
        )).await.map_err(store_error)?;
        if request_updated.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &command.request_id,
                command.expected_revision,
            )
            .await?);
        }
        if reuploaded {
            for table in ["registry_validation_stages", "registry_validation_jobs"] {
                tx.execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!("DELETE FROM {table} WHERE request_id = {}", mark(1)),
                    vec![command.request_id.clone().into()],
                ))
                .await
                .map_err(store_error)?;
            }
        }
        let artifact_details = serde_json::json!({"version":version,"status":"submitted","artifact_size":command.artifact_size,"content_type":command.content_type,"checksum_sha256":command.checksum_sha256});
        for (event_type, details) in [
            ("artifact_uploaded", artifact_details.clone()),
            (
                "artifact_reuploaded_after_changes_requested",
                artifact_details,
            ),
        ] {
            if event_type == "artifact_reuploaded_after_changes_requested" && !reuploaded {
                continue;
            }
            tx.execute_raw(Statement::from_sql_and_values(backend, format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)), vec![self.infrastructure.prefixed_id("rge").into(), slug.clone().into(), command.request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(details)))] )).await.map_err(store_error)?;
        }
        let result = ModulePublishArtifactAttachResult {
            request_id: command.request_id.clone(),
            artifact_storage_key,
            previous_storage_key,
            reuploaded_after_changes_requested: reuploaded,
        };
        record_publish_artifact_receipt(&self.infrastructure, &tx, backend, now, &receipt, &result)
            .await?;
        tx.commit().await.map_err(store_error)?;
        Ok(result)
    }

    /// Stages one immutable completed platform build for a submitted registry
    /// artifact. The owner reloads the durable build pair under tenant RLS and
    /// binds its source, payload, and OCI receipt identities to this request.

}
