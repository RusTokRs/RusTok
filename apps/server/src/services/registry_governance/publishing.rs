use super::*;
use rustok_modules::{
    ModuleExternalPrebuiltStageCommand, ModuleExternalPrebuiltStageResult,
    ModulePublicationArtifactOrigin, ModulePublishApprovalOverride,
    ModulePublishArtifactAttachCommand, ModulePublishPlatformBuildStageCommand,
    ModulePublishPlatformBuildStageResult, ModulePublishRequestChangesCommand,
    ModulePublishRequestCreateCommand, ModulePublishRequestHoldCommand,
    ModulePublishRequestPublicationCommand, ModulePublishRequestRejectCommand,
    ModulePublishRequestResumeCommand,
};

impl RegistryGovernanceService {
    pub async fn create_publish_request(
        &self,
        request: &RegistryPublishRequest,
        authority: &RegistryAuthority,
    ) -> anyhow::Result<RegistryPublishRequestSnapshot> {
        let command = module_publish_request_create_command(
            request,
            authority.principal.to_json_value(),
            authority.can_manage_modules,
        )?;
        let request_id = self
            .publication_service()
            .create_publish_request(command)
            .await?;
        self.publish_request_status_snapshot_for_authority(&request_id, Some(authority))
            .await?
            .map(|snapshot| snapshot.request)
            .ok_or_else(|| anyhow!("owner-created registry publish request disappeared"))
    }

    /// Validates platform-domain request facts and returns only owner-derived
    /// warnings. HTTP schema and authentication remain controller concerns.
    pub fn publish_request_warnings(
        request: &RegistryPublishRequest,
    ) -> anyhow::Result<Vec<String>> {
        module_publish_request_create_command(request, serde_json::json!({}), false)?
            .validation_warnings()
            .map_err(anyhow::Error::new)
    }

    pub async fn upload_publish_artifact(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        artifact: RegistryArtifactUpload,
    ) -> anyhow::Result<RegistryPublishRequestStatusSnapshot> {
        if artifact.bytes.len() > MODULE_PUBLISH_ARTIFACT_MAX_BYTES {
            return Err(malformed_error(format!(
                "Registry publish artifact exceeds the {} byte maximum size",
                MODULE_PUBLISH_ARTIFACT_MAX_BYTES
            )));
        }
        let checksum = hex::encode(Sha256::digest(&artifact.bytes));
        let artifact_size = i64::try_from(artifact.bytes.len())
            .map_err(|_| anyhow!("registry publish artifact size exceeds supported range"))?;
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Manage,
                "upload an artifact for",
            )
            .await?;
        let command = ModulePublishArtifactAttachCommand {
            request_id: request.request.id.clone(),
            expected_revision: request.request.revision,
            actor_principal: authority.principal.to_json_value(),
            actor_can_manage_modules: authority.can_manage_modules,
            checksum_sha256: checksum,
            artifact_size,
            content_type: artifact.content_type.clone(),
        };
        let slot = self
            .publication_service()
            .prepare_publish_artifact_upload(&command)
            .await?;
        if !slot.artifact_already_attached {
            self.store_registry_artifact(
                &slot.artifact_storage_key,
                &artifact,
                &command.checksum_sha256,
            )
            .await
            .context("failed to persist registry artifact")?;
        }
        let result = self
            .publication_service()
            .attach_publish_artifact(command)
            .await?;
        if result.artifact_storage_key != slot.artifact_storage_key {
            return Err(anyhow!(
                "owner attached a registry artifact outside its issued upload slot"
            ));
        }
        self.publish_request_status_snapshot_for_authority(&result.request_id, Some(authority))
            .await?
            .ok_or_else(|| anyhow!("owner-attached registry artifact request disappeared"))
    }

    /// Transport adapter for external prebuilt staging. The owner remains the
    /// sole writer and verifies the platform command context, authenticated
    /// operator capability, and quarantine approver identity together.
    pub async fn stage_external_prebuilt(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        input: RegistryExternalPrebuiltStageInput,
    ) -> anyhow::Result<ModuleExternalPrebuiltStageResult> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Manage,
                "stage an external prebuilt artifact for",
            )
            .await?;
        self.publication_service()
            .stage_external_prebuilt(ModuleExternalPrebuiltStageCommand {
                request_id: request.request.id.clone(),
                expected_revision: request.request.revision,
                context: input.context,
                artifact_digest: input.artifact_digest,
                source_evidence: input.source_evidence,
                provenance_reference: input.provenance_reference,
                provenance_digest: input.provenance_digest,
                provenance_policy_revision: input.provenance_policy_revision,
                quarantine_review_reference: input.quarantine_review_reference,
                quarantine_policy_revision: input.quarantine_policy_revision,
                quarantine_approved_by_principal: authority.principal.to_json_value(),
                actor_principal: authority.principal.to_json_value(),
                actor_can_manage_modules: authority.can_manage_modules,
            })
            .await
            .map_err(anyhow::Error::new)
    }

    /// Transport adapter for staging an immutable completed platform build. It
    /// receives a complete tenant command context only from the
    /// session-authenticated controller; the owner derives both
    /// request-management permission and build identity.
    pub async fn stage_platform_build(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        input: RegistryPlatformBuildStageInput,
    ) -> anyhow::Result<ModulePublishPlatformBuildStageResult> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Manage,
                "stage a platform build for",
            )
            .await?;
        self.publication_service()
            .stage_platform_build(ModulePublishPlatformBuildStageCommand {
                request_id: request.request.id.clone(),
                expected_revision: request.request.revision,
                context: input.context,
                build_request_id: input.build_request_id,
                actor_principal: authority.principal.to_json_value(),
                actor_can_manage_modules: authority.can_manage_modules,
            })
            .await
            .map_err(anyhow::Error::new)
    }

    pub async fn approve_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        idempotency_key: Uuid,
        reason: Option<&str>,
        reason_code: Option<&str>,
    ) -> anyhow::Result<RegistryPublishRequestStatusSnapshot> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Review,
                "approve",
            )
            .await?;
        if !matches!(request.request.status.as_str(), "approved" | "published") {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and cannot be approved",
                request_id, request.request.status
            )));
        }

        let override_stages = request
            .validation_stages
            .iter()
            .filter(|stage| stage.status != "passed")
            .collect::<Vec<_>>();
        let effective_publisher = request
            .effective_publisher_principal
            .clone()
            .unwrap_or_else(|| authority.principal.to_json_value());
        let approval_override = if !override_stages.is_empty() {
            let reason = reason
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    malformed_error(format!(
                        "Registry publish request '{}' still has non-passed follow-up validation stages; approval override requires a non-empty reason",
                        request_id
                    ))
                })?;
            let reason_code = reason_code
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    malformed_error(format!(
                        "Registry publish request '{}' still has non-passed follow-up validation stages; approval override requires a non-empty reason_code",
                        request_id
                    ))
                })?;
            if !REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
            {
                return Err(malformed_error(format!(
                    "Registry publish approval override reason_code '{}' is not supported; expected one of {}",
                    reason_code,
                    REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
                )));
            }
            Some(ModulePublishApprovalOverride {
                reason: reason.to_string(),
                reason_code: reason_code.to_ascii_lowercase(),
                validation_stages: serde_json::Value::Array(
                    override_stages
                        .iter()
                        .copied()
                        .map(validation_stage_snapshot_details_value)
                        .collect(),
                ),
            })
        } else {
            None
        };
        self.publication_service()
            .publish_request(ModulePublishRequestPublicationCommand {
                request_id: request.request.id.clone(),
                expected_revision: request.request.revision,
                idempotency_key,
                actor_principal: authority.principal.to_json_value(),
                publisher_principal: effective_publisher,
                allow_owner_rebind: authority.can_manage_modules,
                approval_override,
            })
            .await
            .map_err(anyhow::Error::new)?;
        self.publish_request_status_snapshot_for_authority(&request.request.id, Some(authority))
            .await?
            .ok_or_else(|| anyhow!("published registry publish request disappeared"))
    }

    pub async fn reject_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<RegistryPublishRequestStatusSnapshot> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Review,
                "reject",
            )
            .await?;
        let normalized_reason = normalize_required_reason(reason, "Registry publish reject")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_REJECT_REASON_CODES,
            "Registry publish reject",
        )?;

        self.publication_service()
            .reject_publish_request(ModulePublishRequestRejectCommand {
                request_id: request.request.id.clone(),
                expected_revision: request.request.revision,
                actor_principal: authority.principal.to_json_value(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)?;
        self.publish_request_status_snapshot_for_authority(&request.request.id, Some(authority))
            .await?
            .ok_or_else(|| anyhow!("rejected registry publish request disappeared"))
    }

    pub async fn request_changes_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<RegistryPublishRequestStatusSnapshot> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Review,
                "request changes for",
            )
            .await?;
        let normalized_reason =
            normalize_required_reason(reason, "Registry publish request-changes")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
            "Registry publish request-changes",
        )?;
        self.publication_service()
            .request_publish_request_changes(ModulePublishRequestChangesCommand {
                request_id: request.request.id.clone(),
                expected_revision: request.request.revision,
                actor_principal: authority.principal.to_json_value(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)?;
        self.publish_request_status_snapshot_for_authority(&request.request.id, Some(authority))
            .await?
            .ok_or_else(|| anyhow!("changed registry publish request disappeared"))
    }

    pub async fn hold_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<RegistryPublishRequestStatusSnapshot> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Review,
                "hold",
            )
            .await?;
        let normalized_reason = normalize_required_reason(reason, "Registry publish hold")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_HOLD_REASON_CODES,
            "Registry publish hold",
        )?;
        self.publication_service()
            .hold_publish_request(ModulePublishRequestHoldCommand {
                request_id: request.request.id.clone(),
                expected_revision: request.request.revision,
                actor_principal: authority.principal.to_json_value(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)?;
        self.publish_request_status_snapshot_for_authority(&request.request.id, Some(authority))
            .await?
            .ok_or_else(|| anyhow!("held registry publish request disappeared"))
    }

    pub async fn resume_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<RegistryPublishRequestStatusSnapshot> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Review,
                "resume",
            )
            .await?;
        let normalized_reason = normalize_required_reason(reason, "Registry publish resume")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_RESUME_REASON_CODES,
            "Registry publish resume",
        )?;
        self.publication_service()
            .resume_publish_request(ModulePublishRequestResumeCommand {
                request_id: request.request.id.clone(),
                expected_revision: request.request.revision,
                actor_principal: authority.principal.to_json_value(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)?;
        self.publish_request_status_snapshot_for_authority(&request.request.id, Some(authority))
            .await?
            .ok_or_else(|| anyhow!("resumed registry publish request disappeared"))
    }
}

fn validation_stage_snapshot_details_value(
    stage: &RegistryValidationStageSnapshot,
) -> serde_json::Value {
    serde_json::json!({
        "stage_key": stage.key,
        "status": stage.status,
        "detail": stage.detail,
        "attempt_number": stage.attempt_number,
        "started_at": stage.started_at,
        "finished_at": stage.finished_at,
        "updated_at": stage.updated_at,
    })
}

fn module_publish_request_create_command(
    request: &RegistryPublishRequest,
    actor_principal: serde_json::Value,
    actor_can_manage_modules: bool,
) -> anyhow::Result<ModulePublishRequestCreateCommand> {
    Ok(ModulePublishRequestCreateCommand {
        slug: request.module.slug.clone(),
        version: request.module.version.clone(),
        crate_name: request.module.crate_name.clone(),
        default_locale: request.module.default_locale.clone(),
        ownership: request.module.ownership.clone(),
        trust_level: request.module.trust_level.clone(),
        license: request.module.license.clone(),
        entry_type: request.module.entry_type.clone(),
        artifact_origin: match request.module.artifact_origin {
            RegistryPublishArtifactOrigin::PlatformBuilt => {
                ModulePublicationArtifactOrigin::PlatformBuilt
            }
            RegistryPublishArtifactOrigin::ExternalPrebuilt => {
                ModulePublicationArtifactOrigin::ExternalPrebuilt
            }
            RegistryPublishArtifactOrigin::AlloyAuthored => {
                ModulePublicationArtifactOrigin::AlloyAuthored
            }
        },
        marketplace: serde_json::to_value(&request.module.marketplace)
            .context("failed to serialize registry publish marketplace metadata")?,
        ui_packages: serde_json::to_value(&request.module.ui_packages)
            .context("failed to serialize registry publish ui_packages metadata")?,
        name: request.module.name.clone(),
        description: request.module.description.clone(),
        actor_principal,
        actor_can_manage_modules,
    })
}
