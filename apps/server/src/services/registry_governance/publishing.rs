use super::*;
use object_store::ObjectStoreExt;
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
    ) -> anyhow::Result<registry_publish_request::Model> {
        self.ensure_authority_can_create_publish_request(authority, &request.module.slug)
            .await?;

        let command =
            module_publish_request_create_command(request, authority.principal.to_json_value())?;
        let request_id = self
            .publication_service()
            .create_publish_request(command)
            .await?;
        self.get_publish_request(&request_id).await?.ok_or_else(|| {
            anyhow::Error::new(RegistryGovernanceError::Internal(anyhow!(
                "registry publish request disappeared after insert"
            )))
        })
    }

    /// Validates platform-domain request facts and returns only owner-derived
    /// warnings. HTTP schema and authentication remain controller concerns.
    pub fn publish_request_warnings(
        request: &RegistryPublishRequest,
    ) -> anyhow::Result<Vec<String>> {
        module_publish_request_create_command(request, serde_json::json!({}))?
            .validation_warnings()
            .map_err(anyhow::Error::new)
    }

    pub async fn get_publish_request(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<registry_publish_request::Model>> {
        Ok(RegistryPublishRequestEntity::find_by_id(request_id)
            .one(&self.db)
            .await?)
    }

    pub async fn upload_publish_artifact(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        artifact: RegistryArtifactUpload,
    ) -> anyhow::Result<registry_publish_request::Model> {
        if artifact.bytes.len() > MODULE_PUBLISH_ARTIFACT_MAX_BYTES {
            return Err(malformed_error(format!(
                "Registry publish artifact exceeds the {} byte maximum size",
                MODULE_PUBLISH_ARTIFACT_MAX_BYTES
            )));
        }
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_manage_publish_request(
            authority,
            &request,
            "upload an artifact for",
        )
        .await?;
        let checksum = hex::encode(Sha256::digest(&artifact.bytes));
        let stored = self
            .store_registry_artifact(&request, &artifact)
            .await
            .context("failed to persist registry artifact")?;
        let result = self
            .publication_service()
            .attach_publish_artifact(ModulePublishArtifactAttachCommand {
                request_id: request_id.to_string(),
                actor_principal: authority.principal.to_json_value(),
                artifact_storage_key: stored.artifact_storage_key.clone(),
                checksum_sha256: checksum,
                artifact_size: stored.artifact_size,
                content_type: artifact.content_type,
            })
            .await?;
        if let Some(previous_storage_key) = result
            .previous_storage_key
            .filter(|value| value != &stored.artifact_storage_key)
        {
            self.require_storage()?
                .objects
                .delete(&object_store::path::Path::from(
                    previous_storage_key.as_str(),
                ))
                .await
                .with_context(|| {
                    format!(
                        "failed to delete previous registry artifact '{}'",
                        previous_storage_key
                    )
                })?;
        }
        self.get_publish_request(&result.request_id)
            .await?
            .ok_or_else(|| anyhow!("owner-attached registry artifact request disappeared"))
    }

    /// Operator-only transport adapter for external prebuilt staging. The
    /// owner service remains the sole writer of the immutable policy and
    /// quarantine fact; this adapter contributes only authenticated identity.
    pub async fn stage_external_prebuilt(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        input: RegistryExternalPrebuiltStageInput,
    ) -> anyhow::Result<ModuleExternalPrebuiltStageResult> {
        if !authority.can_manage_modules {
            return Err(forbidden_error(
                "External prebuilt staging requires modules.manage authority",
            ));
        }
        if self.get_publish_request(request_id).await?.is_none() {
            return Err(not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            )));
        }
        self.publication_service()
            .stage_external_prebuilt(ModuleExternalPrebuiltStageCommand {
                request_id: request_id.to_string(),
                artifact_digest: input.artifact_digest,
                source_evidence: input.source_evidence,
                provenance_reference: input.provenance_reference,
                provenance_digest: input.provenance_digest,
                provenance_policy_revision: input.provenance_policy_revision,
                quarantine_review_reference: input.quarantine_review_reference,
                quarantine_policy_revision: input.quarantine_policy_revision,
                quarantine_approved_by_principal: authority.principal.to_json_value(),
                idempotency_key: input.idempotency_key,
                actor_principal: authority.principal.to_json_value(),
            })
            .await
            .map_err(anyhow::Error::new)
    }

    /// Operator-only adapter for staging an immutable completed platform build.
    /// It receives a tenant only from the session-authenticated controller and
    /// leaves all build/result and artifact identity checks to the owner.
    pub async fn stage_platform_build(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        input: RegistryPlatformBuildStageInput,
    ) -> anyhow::Result<ModulePublishPlatformBuildStageResult> {
        if !authority.can_manage_modules {
            return Err(forbidden_error(
                "Platform build staging requires modules.manage authority",
            ));
        }
        if self.get_publish_request(request_id).await?.is_none() {
            return Err(not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            )));
        }
        self.publication_service()
            .stage_platform_build(ModulePublishPlatformBuildStageCommand {
                request_id: request_id.to_string(),
                tenant_id: input.tenant_id,
                build_request_id: input.build_request_id,
                idempotency_key: input.idempotency_key,
                actor_principal: authority.principal.to_json_value(),
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
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(authority, &request, "approve")
            .await?;
        if !matches!(
            request.status,
            RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
        ) {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and cannot be approved",
                request_id,
                request_status_label(request.status.clone())
            )));
        }

        let latest_validation_stages = self
            .latest_validation_stages_for_request(&request.id)
            .await?;
        let override_stages = latest_validation_stages
            .iter()
            .filter(|stage| stage.status != RegistryValidationStageStatus::Passed)
            .cloned()
            .collect::<Vec<_>>();
        let effective_publisher = self
            .resolve_effective_publisher(&request, authority)
            .await?;
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
                        .map(validation_stage_details_value)
                        .collect(),
                ),
            })
        } else {
            None
        };
        self.publication_service()
            .publish_request(ModulePublishRequestPublicationCommand {
                request_id: request.id.clone(),
                idempotency_key,
                actor_principal: authority.principal.to_json_value(),
                publisher_principal: RegistryPrincipalRef::from_legacy_value(&effective_publisher)
                    .to_json_value(),
                allow_owner_rebind: authority.can_manage_modules,
                approval_override,
            })
            .await
            .map_err(anyhow::Error::new)?;
        RegistryPublishRequestEntity::find_by_id(request.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("published registry publish request disappeared"))
    }

    pub async fn reject_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(authority, &request, "reject")
            .await?;
        let normalized_reason = normalize_required_reason(reason, "Registry publish reject")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_REJECT_REASON_CODES,
            "Registry publish reject",
        )?;

        self.publication_service()
            .reject_publish_request(ModulePublishRequestRejectCommand {
                request_id: request.id.clone(),
                actor_principal: authority.principal.to_json_value(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)?;
        RegistryPublishRequestEntity::find_by_id(request.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("rejected registry publish request disappeared"))
    }

    pub async fn request_changes_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(
            authority,
            &request,
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
                request_id: request.id.clone(),
                actor_principal: authority.principal.to_json_value(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)?;
        RegistryPublishRequestEntity::find_by_id(request.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("changed registry publish request disappeared"))
    }

    pub async fn hold_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(authority, &request, "hold")
            .await?;
        let normalized_reason = normalize_required_reason(reason, "Registry publish hold")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_HOLD_REASON_CODES,
            "Registry publish hold",
        )?;
        self.publication_service()
            .hold_publish_request(ModulePublishRequestHoldCommand {
                request_id: request.id.clone(),
                actor_principal: authority.principal.to_json_value(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)?;
        RegistryPublishRequestEntity::find_by_id(request.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("held registry publish request disappeared"))
    }

    pub async fn resume_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(authority, &request, "resume")
            .await?;
        let normalized_reason = normalize_required_reason(reason, "Registry publish resume")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_RESUME_REASON_CODES,
            "Registry publish resume",
        )?;
        self.publication_service()
            .resume_publish_request(ModulePublishRequestResumeCommand {
                request_id: request.id.clone(),
                actor_principal: authority.principal.to_json_value(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)?;
        RegistryPublishRequestEntity::find_by_id(request.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("resumed registry publish request disappeared"))
    }
}

fn module_publish_request_create_command(
    request: &RegistryPublishRequest,
    actor_principal: serde_json::Value,
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
    })
}

pub fn request_status_label(status: RegistryPublishRequestStatus) -> &'static str {
    match status {
        RegistryPublishRequestStatus::Draft => "draft",
        RegistryPublishRequestStatus::ArtifactUploaded => "artifact_uploaded",
        RegistryPublishRequestStatus::Submitted => "submitted",
        RegistryPublishRequestStatus::Validating => "validating",
        RegistryPublishRequestStatus::Approved => "approved",
        RegistryPublishRequestStatus::ChangesRequested => "changes_requested",
        RegistryPublishRequestStatus::OnHold => "on_hold",
        RegistryPublishRequestStatus::Rejected => "rejected",
        RegistryPublishRequestStatus::Published => "published",
    }
}

#[allow(dead_code)]
pub(crate) fn lifecycle_governance_actions(
    latest_request: Option<&registry_publish_request::Model>,
    latest_release: Option<&registry_module_release::Model>,
    owner_binding: Option<&registry_module_owner::Model>,
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Vec<RegistryGovernanceActionSnapshot> {
    let mut actions = latest_request
        .map(|request| {
            let approval_override_required = request.status
                == RegistryPublishRequestStatus::Approved
                && validation_stages
                    .iter()
                    .any(|stage| !stage.status.eq_ignore_ascii_case("passed"));
            publish_request_governance_actions(
                request,
                validation_stages,
                approval_override_required,
            )
        })
        .unwrap_or_default();

    if latest_request.is_some_and(|request| {
        request
            .publisher_principal
            .as_ref()
            .is_some_and(|publisher| {
                owner_binding.is_none_or(|owner| owner.owner_principal != *publisher)
            })
    }) || owner_binding.is_some()
    {
        actions.push(governance_action_snapshot(
            "owner_transfer",
            true,
            true,
            REGISTRY_OWNER_TRANSFER_REASON_CODES,
            true,
        ));
    }

    if latest_release.is_some_and(|release| release.status == RegistryModuleReleaseStatus::Active) {
        actions.push(governance_action_snapshot(
            "yank",
            true,
            true,
            REGISTRY_YANK_REASON_CODES,
            true,
        ));
    }

    dedupe_governance_actions(actions)
}

pub(crate) fn publish_request_governance_actions(
    request: &registry_publish_request::Model,
    validation_stages: &[RegistryValidationStageSnapshot],
    approval_override_required: bool,
) -> Vec<RegistryGovernanceActionSnapshot> {
    publish_request_governance_actions_for_authority(
        request,
        None,
        validation_stages,
        approval_override_required,
        &RegistryAuthority {
            principal: RegistryPrincipalRef::legacy(""),
            can_manage_modules: true,
        },
    )
}

pub(crate) fn publish_request_governance_actions_for_authority(
    request: &registry_publish_request::Model,
    owner_binding: Option<&registry_module_owner::Model>,
    _validation_stages: &[RegistryValidationStageSnapshot],
    approval_override_required: bool,
    authority: &RegistryAuthority,
) -> Vec<RegistryGovernanceActionSnapshot> {
    let mut actions = Vec::new();
    let can_manage = authority.can_manage_modules
        || authority_can_manage_publish_request(authority, request, owner_binding);
    let can_review = authority.can_manage_modules
        || authority_can_review_publish_request(authority, owner_binding);

    if can_manage
        && matches!(
            request.status,
            RegistryPublishRequestStatus::ArtifactUploaded
                | RegistryPublishRequestStatus::Submitted
        )
    {
        actions.push(governance_action_snapshot(
            "validate",
            false,
            false,
            &[],
            false,
        ));
    }

    if can_review && request.status == RegistryPublishRequestStatus::Approved {
        actions.push(governance_action_snapshot(
            "approve",
            approval_override_required,
            approval_override_required,
            if approval_override_required {
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES
            } else {
                &[]
            },
            false,
        ));
        actions.push(governance_action_snapshot(
            "request_changes",
            true,
            true,
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
            false,
        ));
    }

    if can_review
        && matches!(
            request.status,
            RegistryPublishRequestStatus::Submitted
                | RegistryPublishRequestStatus::Approved
                | RegistryPublishRequestStatus::ChangesRequested
        )
    {
        actions.push(governance_action_snapshot(
            "hold",
            true,
            true,
            REGISTRY_HOLD_REASON_CODES,
            false,
        ));
    }

    if can_review && request.status == RegistryPublishRequestStatus::OnHold {
        actions.push(governance_action_snapshot(
            "resume",
            true,
            true,
            REGISTRY_RESUME_REASON_CODES,
            false,
        ));
    }

    if can_review
        && !matches!(
            request.status,
            RegistryPublishRequestStatus::Rejected
                | RegistryPublishRequestStatus::Published
                | RegistryPublishRequestStatus::OnHold
        )
    {
        actions.push(governance_action_snapshot(
            "reject",
            true,
            true,
            REGISTRY_REJECT_REASON_CODES,
            true,
        ));
    }

    dedupe_governance_actions(actions)
}

fn governance_action_snapshot(
    key: &str,
    reason_required: bool,
    reason_code_required: bool,
    reason_codes: &[&str],
    destructive: bool,
) -> RegistryGovernanceActionSnapshot {
    RegistryGovernanceActionSnapshot {
        key: key.to_string(),
        reason_required,
        reason_code_required,
        reason_codes: reason_codes
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        destructive,
    }
}

fn dedupe_governance_actions(
    actions: Vec<RegistryGovernanceActionSnapshot>,
) -> Vec<RegistryGovernanceActionSnapshot> {
    let mut seen = std::collections::HashSet::new();

    actions
        .into_iter()
        .filter(|action| seen.insert(action.key.clone()))
        .collect()
}
