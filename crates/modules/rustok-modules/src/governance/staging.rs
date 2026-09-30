//! Platform build staging and publication sources.

use sea_orm::{ConnectionTrait, Statement, TransactionTrait, Value};
use uuid::Uuid;

use super::*;
use super::helpers::*;
use super::mapping::*;
use super::staging_alloy::*;
use super::validation_evidence::*;
use crate::build::{
    ModuleBuildOutcome, ModuleBuildValidationOutcome, ModuleBuildValidationProfile, SeaOrmModuleBuildService,
};

impl SeaOrmModuleGovernanceService {

    /// Stages one immutable completed platform build for a submitted registry
    /// artifact. The owner reloads the durable build pair under tenant RLS and
    /// binds its source, payload, and OCI receipt identities to this request.
    pub async fn stage_platform_build(
        &self,
        command: ModulePublishPlatformBuildStageCommand,
    ) -> Result<ModulePublishPlatformBuildStageResult, ModuleGovernanceError> {
        command.validate()?;
        let tenant_id = command
            .context
            .tenant_id
            .ok_or(ModuleGovernanceError::InvalidPlatformBuildStageCommand)?;
        let completed = SeaOrmModuleBuildService::new(self.db.clone())
            .load_completed(tenant_id, command.build_request_id)
            .await
            .map_err(|_| ModuleGovernanceError::InvalidPlatformBuildStageCommand)?;
        let component_digest = completed
            .result
            .component_digest
            .as_deref()
            .ok_or(ModuleGovernanceError::InvalidPlatformBuildStageCommand)?;
        let receipt = completed
            .result
            .publication
            .as_ref()
            .ok_or(ModuleGovernanceError::InvalidPlatformBuildStageCommand)?;
        let required_validation_profiles = [
            ModuleBuildValidationProfile::Check,
            ModuleBuildValidationProfile::Test,
            ModuleBuildValidationProfile::DependencyPolicy,
            ModuleBuildValidationProfile::Vulnerability,
        ];
        if !matches!(&completed.result.outcome, ModuleBuildOutcome::Succeeded)
            || completed.request.expected_module_slug.trim().is_empty()
            || completed.request.expected_version.trim().is_empty()
            || !platform_build_artifact_identities_valid(component_digest, receipt)
            || required_validation_profiles.iter().any(|profile| {
                !completed.request.validation_profiles.contains(profile)
                    || !completed
                        .result
                        .evidence
                        .validation_results
                        .iter()
                        .any(|result| {
                            result.profile == *profile
                                && result.outcome == ModuleBuildValidationOutcome::Passed
                        })
            })
        {
            return Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand);
        }

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
            return Err(ModuleGovernanceError::PublishRequestPlatformBuildStagingUnauthorized);
        }
        if artifact_origin != ModulePublicationArtifactOrigin::PlatformBuilt.as_str()
            || !matches!(status.as_str(), "submitted" | "validating" | "approved")
            || slug != completed.request.expected_module_slug
            || version != completed.request.expected_version
            || checksum
                .as_deref()
                .is_none_or(|value| !is_sha256_hex(value))
        {
            return Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand);
        }
        if let Some(parent_release) = &completed.request.parent_release
            && !active_published_rhai_parent_exists(&tx, backend, parent_release).await?
        {
            return Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand);
        }

        let staging_id = self.infrastructure.prefixed_id("rpbs");
        let inserted = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publish_build_staging \
                     (id, request_id, expected_revision, tenant_id, build_request_id, source_reference, source_digest, \
                      parent_release_slug, parent_release_version, parent_release_digest, component_digest, \
                      artifact_manifest_digest, \
                      signature_manifest_digest, staged_by_principal, actor_id, trace_id, correlation_id, \
                      actor_can_manage_modules, idempotency_key, staged_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now}) \
                     ON CONFLICT (request_id, idempotency_key) DO NOTHING",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8),
                    mark(9), mark(10), mark(11), mark(12), mark(13), mark(14), mark(15),
                    mark(16), mark(17), mark(18), mark(19),
                ),
                vec![
                    staging_id.clone().into(),
                    command.request_id.clone().into(),
                    command.expected_revision.into(),
                    registry_uuid_value(tenant_id, backend),
                    registry_uuid_value(command.build_request_id, backend),
                    completed.request.source.reference.clone().into(),
                    completed.request.source.digest.clone().into(),
                    completed.request.parent_release.as_ref().map(|parent| parent.slug.clone()).into(),
                    completed.request.parent_release.as_ref().map(|parent| parent.version.clone()).into(),
                    completed.request.parent_release.as_ref().map(|parent| parent.digest.clone()).into(),
                    component_digest.to_string().into(),
                    receipt.artifact.digest.clone().into(),
                    receipt.signature_manifest.digest.clone().into(),
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
                        "SELECT id, expected_revision, CAST(tenant_id AS TEXT) AS tenant_id, \
                         CAST(build_request_id AS TEXT) AS build_request_id, \
                         source_reference, source_digest, \
                         parent_release_slug, parent_release_version, parent_release_digest, component_digest, \
                         CAST(staged_by_principal AS TEXT) AS staged_by_principal, \
                         CAST(actor_id AS TEXT) AS actor_id, trace_id, \
                         CAST(correlation_id AS TEXT) AS correlation_id, actor_can_manage_modules \
                         FROM registry_publish_build_staging \
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
                    ModuleGovernanceError::Store("build-stage conflict lost its row".to_string())
                })?;
            let existing_id: String = existing.try_get("", "id").map_err(store_error)?;
            let existing_expected_revision: i64 = existing
                .try_get("", "expected_revision")
                .map_err(store_error)?;
            let existing_tenant_id: String =
                existing.try_get("", "tenant_id").map_err(store_error)?;
            let existing_build_request_id: String = existing
                .try_get("", "build_request_id")
                .map_err(store_error)?;
            let existing_source_reference: String = existing
                .try_get("", "source_reference")
                .map_err(store_error)?;
            let existing_source_digest: String =
                existing.try_get("", "source_digest").map_err(store_error)?;
            let existing_parent_release = parent_release_from_stage_row(&existing)?;
            let existing_component_digest: String = existing
                .try_get("", "component_digest")
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
                || existing_tenant_id != tenant_id.to_string()
                || existing_build_request_id != command.build_request_id.to_string()
                || existing_source_reference != completed.request.source.reference
                || existing_source_digest != completed.request.source.digest
                || existing_parent_release != completed.request.parent_release
                || existing_component_digest != component_digest
                || existing_actor != command.actor_principal
                || existing_actor_id != command.context.actor_id.to_string()
                || existing_trace_id != command.context.trace_id
                || existing_correlation_id != command.context.correlation_id.to_string()
                || existing_actor_can_manage_modules != command.actor_can_manage_modules
            {
                return Err(ModuleGovernanceError::PlatformBuildStageIdempotencyConflict);
            }
            for stage in PLATFORM_BUILT_FOLLOW_UP_STAGES {
                pass_owner_evidence_validation_stage(
                    &self.infrastructure,
                    &tx,
                    backend,
                    OwnerEvidenceStage {
                        request_id: &command.request_id,
                        slug: &slug,
                        version: &version,
                        stage_key: stage.key,
                        actor_principal: &command.actor_principal,
                        detail: &format!(
                            "Platform build evidence satisfied validation stage '{}'.",
                            stage.key
                        ),
                        reason_code: "platform_build_evidence_passed",
                    },
                )
                .await?;
            }
            tx.commit().await.map_err(store_error)?;
            return Ok(ModulePublishPlatformBuildStageResult {
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
        for stage in PLATFORM_BUILT_FOLLOW_UP_STAGES {
            pass_owner_evidence_validation_stage(
                &self.infrastructure,
                &tx,
                backend,
                OwnerEvidenceStage {
                    request_id: &command.request_id,
                    slug: &slug,
                    version: &version,
                    stage_key: stage.key,
                    actor_principal: &command.actor_principal,
                    detail: &format!(
                        "Platform build evidence satisfied validation stage '{}'.",
                        stage.key
                    ),
                    reason_code: "platform_build_evidence_passed",
                },
            )
            .await?;
        }
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'platform_build_staged', {}, NULL, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                self.infrastructure.prefixed_id("rge").into(),
                slug.into(),
                command.request_id.into(),
                Value::Json(Some(Box::new(command.actor_principal))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "build_request_id": command.build_request_id,
                    "source_reference": completed.request.source.reference,
                    "source_digest": completed.request.source.digest,
                    "parent_release": completed.request.parent_release,
                    "component_digest": component_digest,
                    "artifact_manifest_digest": receipt.artifact.digest,
                    "command_context": {
                        "actor_id": command.context.actor_id,
                        "tenant_id": tenant_id,
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
        tx.commit().await.map_err(store_error)?;
        Ok(ModulePublishPlatformBuildStageResult {
            staging_id,
            created: true,
            request_revision: request_revision + 1,
        })
    }

    /// Reloads the current platform-build staging selection and proves it
    /// still matches the immutable completed build owned by the same tenant.
    /// Registry and verification workers use this read instead of accepting a
    /// caller-supplied OCI receipt or descriptor.
    pub async fn load_platform_publication_source(
        &self,
        request_id: &str,
    ) -> Result<ModulePlatformPublicationSource, ModuleGovernanceError> {
        if request_id.trim().is_empty() || request_id.len() > MAX_PUBLICATION_REQUEST_ID_BYTES {
            return Err(ModuleGovernanceError::InvalidPlatformPublicationEvidenceRequest);
        }
        let backend = self.db.get_database_backend();
        let mark = |position| placeholder(backend, position);
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request.slug, request.version, request.revision, request.status, request.artifact_origin, \
                     CAST(stage.tenant_id AS TEXT) AS tenant_id, \
                     CAST(stage.build_request_id AS TEXT) AS build_request_id, \
                     stage.source_reference, stage.source_digest, stage.component_digest, \
                     stage.artifact_manifest_digest, stage.signature_manifest_digest \
                     FROM registry_publish_requests AS request \
                     INNER JOIN registry_publish_build_staging AS stage \
                       ON stage.request_id = request.id AND stage.staged_at >= request.submitted_at \
                     WHERE request.id = {} \
                     ORDER BY stage.staged_at DESC, stage.id DESC LIMIT 1",
                    mark(1),
                ),
                vec![request_id.to_string().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PlatformPublicationEvidenceSourceUnavailable)?;
        let status: String = row.try_get("", "status").map_err(store_error)?;
        let artifact_origin: String = row.try_get("", "artifact_origin").map_err(store_error)?;
        if artifact_origin != ModulePublicationArtifactOrigin::PlatformBuilt.as_str()
            || !matches!(status.as_str(), "validating" | "approved" | "published")
        {
            return Err(ModuleGovernanceError::PlatformPublicationEvidenceSourceUnavailable);
        }
        let tenant_id = Uuid::parse_str(
            &row.try_get::<String>("", "tenant_id")
                .map_err(store_error)?,
        )
        .map_err(|_| ModuleGovernanceError::PlatformPublicationEvidenceSourceUnavailable)?;
        let build_request_id = Uuid::parse_str(
            &row.try_get::<String>("", "build_request_id")
                .map_err(store_error)?,
        )
        .map_err(|_| ModuleGovernanceError::PlatformPublicationEvidenceSourceUnavailable)?;
        let completed = SeaOrmModuleBuildService::new(self.db.clone())
            .load_completed(tenant_id, build_request_id)
            .await
            .map_err(|_| ModuleGovernanceError::PlatformPublicationEvidenceSourceUnavailable)?;
        let component_digest = completed
            .result
            .component_digest
            .clone()
            .ok_or(ModuleGovernanceError::PlatformPublicationEvidenceSourceUnavailable)?;
        let receipt = completed
            .result
            .publication
            .clone()
            .ok_or(ModuleGovernanceError::PlatformPublicationEvidenceSourceUnavailable)?;
        let slug: String = row.try_get("", "slug").map_err(store_error)?;
        let version: String = row.try_get("", "version").map_err(store_error)?;
        let request_revision: i64 = row.try_get("", "revision").map_err(store_error)?;
        let staged_source_reference: String =
            row.try_get("", "source_reference").map_err(store_error)?;
        let staged_source_digest: String = row.try_get("", "source_digest").map_err(store_error)?;
        let staged_component_digest: String =
            row.try_get("", "component_digest").map_err(store_error)?;
        let staged_artifact_manifest_digest: String = row
            .try_get("", "artifact_manifest_digest")
            .map_err(store_error)?;
        let staged_signature_manifest_digest: String = row
            .try_get("", "signature_manifest_digest")
            .map_err(store_error)?;
        if !matches!(completed.result.outcome, ModuleBuildOutcome::Succeeded)
            || completed.request.expected_module_slug != slug
            || completed.request.expected_version != version
            || completed.request.source.reference != staged_source_reference
            || completed.request.source.digest != staged_source_digest
            || component_digest != staged_component_digest
            || receipt.artifact.digest != staged_artifact_manifest_digest
            || receipt.signature_manifest.digest != staged_signature_manifest_digest
            || !platform_build_artifact_identities_valid(&component_digest, &receipt)
        {
            return Err(ModuleGovernanceError::PlatformPublicationEvidenceSourceUnavailable);
        }
        Ok(ModulePlatformPublicationSource {
            request_id: request_id.to_string(),
            request_revision,
            tenant_id,
            build_request_id,
            slug,
            version,
            component_digest,
            receipt,
        })
    }

    /// Reloads the immutable Alloy staging receipt used to package a canonical
    /// Rhai workspace. This owner read never reaches Alloy tables: every fact
    /// is durable registry state captured at stage time and must still match
    /// the current uploaded artifact before a worker can publish it.
    pub async fn load_alloy_publication_source(
        &self,
        request_id: &str,
    ) -> Result<ModuleAlloyPublicationSource, ModuleGovernanceError> {
        if request_id.trim().is_empty() || request_id.len() > MAX_PUBLICATION_REQUEST_ID_BYTES {
            return Err(ModuleGovernanceError::InvalidAlloyPublicationEvidenceRequest);
        }
        let backend = self.db.get_database_backend();
        let mark = |position| placeholder(backend, position);
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request.slug, request.version, request.revision, request.status, \
                     request.artifact_origin, request.license, request.artifact_checksum_sha256, \
                     CAST(stage.alloy_tenant_id AS TEXT) AS alloy_tenant_id, \
                     CAST(stage.alloy_script_id AS TEXT) AS alloy_script_id, \
                     stage.artifact_digest, stage.source_digest, stage.source_revision, \
                     CAST(stage.descriptor AS TEXT) AS descriptor, stage.descriptor_digest, \
                     stage.review_digest \
                     FROM registry_publish_requests AS request \
                     INNER JOIN registry_publish_alloy_staging AS stage \
                       ON stage.request_id = request.id AND stage.staged_at >= request.submitted_at \
                     WHERE request.id = {} \
                     ORDER BY stage.staged_at DESC, stage.id DESC LIMIT 1",
                    mark(1),
                ),
                vec![request_id.to_string().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::AlloyPublicationEvidenceSourceUnavailable)?;
        let status: String = row.try_get("", "status").map_err(store_error)?;
        let artifact_origin: String = row.try_get("", "artifact_origin").map_err(store_error)?;
        if artifact_origin != ModulePublicationArtifactOrigin::AlloyAuthored.as_str()
            || !matches!(status.as_str(), "validating" | "approved" | "published")
        {
            return Err(ModuleGovernanceError::AlloyPublicationEvidenceSourceUnavailable);
        }
        let slug: String = row.try_get("", "slug").map_err(store_error)?;
        let version: String = row.try_get("", "version").map_err(store_error)?;
        let request_revision: i64 = row.try_get("", "revision").map_err(store_error)?;
        let license: String = row.try_get("", "license").map_err(store_error)?;
        let checksum: String = row
            .try_get("", "artifact_checksum_sha256")
            .map_err(store_error)?;
        if !is_sha256_hex(&checksum) {
            return Err(ModuleGovernanceError::AlloyPublicationEvidenceSourceUnavailable);
        }
        let artifact_digest = format!("sha256:{checksum}");
        let alloy_tenant_id = Uuid::parse_str(
            &row.try_get::<String>("", "alloy_tenant_id")
                .map_err(store_error)?,
        )
        .map_err(|_| ModuleGovernanceError::AlloyPublicationEvidenceSourceUnavailable)?;
        let alloy_script_id = Uuid::parse_str(
            &row.try_get::<String>("", "alloy_script_id")
                .map_err(store_error)?,
        )
        .map_err(|_| ModuleGovernanceError::AlloyPublicationEvidenceSourceUnavailable)?;
        let source_revision: i64 = row.try_get("", "source_revision").map_err(store_error)?;
        let source_revision = u32::try_from(source_revision)
            .ok()
            .filter(|revision| *revision > 0)
            .ok_or(ModuleGovernanceError::AlloyPublicationEvidenceSourceUnavailable)?;
        let stage_artifact_digest: String =
            row.try_get("", "artifact_digest").map_err(store_error)?;
        let source_digest: String = row.try_get("", "source_digest").map_err(store_error)?;
        let review_digest: String = row.try_get("", "review_digest").map_err(store_error)?;
        let descriptor: crate::ModuleArtifactDescriptor = serde_json::from_str(
            &row.try_get::<String>("", "descriptor")
                .map_err(store_error)?,
        )
        .map_err(store_error)?;
        let descriptor_digest: String =
            row.try_get("", "descriptor_digest").map_err(store_error)?;
        if stage_artifact_digest != artifact_digest
            || source_digest != artifact_digest
            || !prefixed_sha256_digest(&review_digest)
            || descriptor_digest != crate::canonical_artifact_descriptor_digest(&descriptor)
            || descriptor.validate().is_err()
            || descriptor.slug != slug
            || descriptor.version != version
            || descriptor.artifact_digest != artifact_digest
            || descriptor.payload_kind != crate::ArtifactPayloadKind::Rhai
            || descriptor.module_kind != crate::ArtifactModuleKind::Optional
            || descriptor.runtime_abi != rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI
            || license.trim().is_empty()
        {
            return Err(ModuleGovernanceError::AlloyPublicationEvidenceSourceUnavailable);
        }
        Ok(ModuleAlloyPublicationSource {
            request_id: request_id.to_string(),
            request_revision,
            slug,
            version,
            license,
            alloy_tenant_id,
            alloy_script_id,
            source_revision,
            source_digest,
            review_digest,
            descriptor,
            descriptor_digest,
        })
    }

}
