//! Governance command validations.

use semver::Version;

use super::*;
use super::helpers::*;
use super::mapping::*;
use crate::build::ModuleBuildSignatureAuthority;
use crate::marketplace_content::ModuleMarketplaceContentProjection;
use crate::ModuleCommandContext;

impl ModuleReleaseYankCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.slug.trim().is_empty()
            || self.version.trim().is_empty()
            || self.reason.trim().is_empty()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
        {
            return Err(ModuleGovernanceError::InvalidYankCommand);
        }
        if !REGISTRY_YANK_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(ModuleGovernanceError::InvalidYankReasonCode(
                self.reason_code.clone(),
            ));
        }
        Ok(())
    }
}

impl ModuleOwnerTransferCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.slug.trim().is_empty()
            || self.reason.trim().is_empty()
            || self.new_owner_principal.is_null()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
            || governance_principal_user_id(&self.new_owner_principal).is_none()
        {
            return Err(ModuleGovernanceError::InvalidOwnerTransferCommand);
        }
        if !REGISTRY_OWNER_TRANSFER_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(ModuleGovernanceError::InvalidOwnerTransferReasonCode(
                self.reason_code.clone(),
            ));
        }
        Ok(())
    }
}

impl ModulePublishRequestRejectCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || self.request_id.len() > MAX_PUBLICATION_REQUEST_ID_BYTES
            || self.request_id.chars().any(char::is_control)
            || self.reason.trim().is_empty()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestRejectCommand);
        }
        if !REGISTRY_REJECT_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(
                ModuleGovernanceError::InvalidPublishRequestRejectReasonCode(
                    self.reason_code.clone(),
                ),
            );
        }
        Ok(())
    }
}

impl ModulePublishRequestChangesCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || self.reason.trim().is_empty()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestChangesCommand);
        }
        if !REGISTRY_REQUEST_CHANGES_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(
                ModuleGovernanceError::InvalidPublishRequestChangesReasonCode(
                    self.reason_code.clone(),
                ),
            );
        }
        Ok(())
    }
}

impl ModulePublishRequestHoldCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || self.reason.trim().is_empty()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestHoldCommand);
        }
        if !REGISTRY_HOLD_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(ModuleGovernanceError::InvalidPublishRequestHoldReasonCode(
                self.reason_code.clone(),
            ));
        }
        Ok(())
    }
}

impl ModulePublishRequestResumeCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || self.reason.trim().is_empty()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestResumeCommand);
        }
        if !REGISTRY_RESUME_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(
                ModuleGovernanceError::InvalidPublishRequestResumeReasonCode(
                    self.reason_code.clone(),
                ),
            );
        }
        Ok(())
    }
}

pub(crate) fn valid_command_context_actor(
    context: &ModuleCommandContext,
    actor_principal: &serde_json::Value,
) -> bool {
    context.validate().is_ok()
        && matches!(governance_principal_user_id(actor_principal), Some(id) if id == context.actor_id.to_string())
}

pub(crate) fn valid_platform_registry_command_context(
    context: &ModuleCommandContext,
    actor_principal: &serde_json::Value,
) -> bool {
    context.validate().is_ok()
        && context.tenant_id.is_none()
        && matches!(
            governance_principal_user_id(actor_principal),
            Some(principal_actor_id) if principal_actor_id == context.actor_id.to_string()
        )
}

impl ModulePublishRequestPublicationCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        let actor_id = self.context.actor_id.to_string();
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || !self.actor_principal.is_object()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
            || !self.publisher_principal.is_object()
            || self.context.validate().is_err()
            || self.context.tenant_id.is_some()
            || !matches!(
                governance_principal_user_id(&self.actor_principal),
                Some(principal_actor_id) if principal_actor_id == actor_id.as_str()
            )
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestPublicationCommand);
        }
        if let Some(override_evidence) = &self.approval_override {
            if override_evidence.reason.trim().is_empty()
                || !override_evidence.validation_stages.is_array()
            {
                return Err(ModuleGovernanceError::InvalidPublishApprovalOverride);
            }
            if !REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .contains(&override_evidence.reason_code.as_str())
            {
                return Err(
                    ModuleGovernanceError::InvalidPublishApprovalOverrideReasonCode(
                        override_evidence.reason_code.clone(),
                    ),
                );
            }
        }
        Ok(())
    }
}

impl ModuleValidationStageReportCommand {
    pub(crate) fn normalized(mut self) -> Result<Self, ModuleGovernanceError> {
        self.request_id = self.request_id.trim().to_string();
        self.stage_key = self.stage_key.trim().to_ascii_lowercase();
        self.status = self.status.trim().to_ascii_lowercase();
        self.reason_code = self
            .reason_code
            .take()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || self.expected_revision == i64::MAX
            || self.stage_key.trim().is_empty()
            || !self.actor_principal.is_object()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
            || !matches!(
                self.status.as_str(),
                "queued" | "running" | "passed" | "failed" | "blocked"
            )
        {
            return Err(ModuleGovernanceError::InvalidValidationStageReportCommand);
        }
        if self.requeue != (self.status == "queued") {
            return Err(ModuleGovernanceError::InvalidValidationStageRequeue);
        }
        if let Some(reason_code) = &self.reason_code
            && !REGISTRY_VALIDATION_STAGE_REASON_CODES.contains(&reason_code.as_str())
        {
            return Err(ModuleGovernanceError::InvalidValidationStageReasonCode(
                reason_code.clone(),
            ));
        }
        Ok(())
    }
}

impl ModuleRemoteValidationHeartbeatCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.claim_id.trim().is_empty() || self.runner_id.trim().is_empty() {
            return Err(ModuleGovernanceError::InvalidRemoteValidationLeaseCommand);
        }
        Ok(())
    }
}

impl ModuleRemoteValidationTerminalCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.claim_id.trim().is_empty()
            || self.runner_id.trim().is_empty()
            || self.expected_request_revision < 1
        {
            return Err(ModuleGovernanceError::InvalidRemoteValidationLeaseCommand);
        }
        if let Some(reason_code) = &self.reason_code
            && !REGISTRY_VALIDATION_STAGE_REASON_CODES.contains(&reason_code.as_str())
        {
            return Err(ModuleGovernanceError::InvalidValidationStageReasonCode(
                reason_code.clone(),
            ));
        }
        Ok(())
    }
}

impl ModuleRemoteValidationClaimCommand {
    pub(crate) fn normalized_supported_stages(&self) -> Result<Vec<String>, ModuleGovernanceError> {
        if self.runner_id.trim().is_empty() {
            return Err(ModuleGovernanceError::InvalidRemoteValidationLeaseCommand);
        }
        let mut normalized = Vec::new();
        for stage in &self.supported_stages {
            let candidate = stage.trim().to_ascii_lowercase();
            if candidate.is_empty() {
                return Err(ModuleGovernanceError::InvalidRemoteValidationClaimStage(
                    stage.clone(),
                ));
            }
            if !REMOTE_VALIDATION_FOLLOW_UP_STAGES.contains(&candidate.as_str()) {
                return Err(ModuleGovernanceError::InvalidRemoteValidationClaimStage(
                    stage.clone(),
                ));
            }
            if !normalized.contains(&candidate) {
                normalized.push(candidate);
            }
        }
        Ok(normalized)
    }
}

impl ModuleValidationJobEnqueueCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || !self.actor_principal.is_object()
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
        {
            return Err(ModuleGovernanceError::InvalidValidationJobEnqueueCommand);
        }
        Ok(())
    }
}

impl ModuleValidationJobClaimCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.validation_job_id.trim().is_empty() || !self.actor_principal.is_object() {
            return Err(ModuleGovernanceError::InvalidValidationJobClaimCommand);
        }
        Ok(())
    }
}

impl ModuleValidationJobResultCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        let mut automated_check_keys = std::collections::HashSet::new();
        let invalid_automated_checks = self.automated_checks.iter().any(|check| {
            let key = check.key.trim();
            key.is_empty()
                || check.status.trim().is_empty()
                || !automated_check_keys.insert(key.to_ascii_lowercase())
        });
        if self.validation_job_id.trim().is_empty()
            || self.expected_request_revision < 1
            || !self.actor_principal.is_object()
            || invalid_automated_checks
            || self
                .actor_principal
                .get("id")
                .or_else(|| self.actor_principal.get("subject"))
                .and_then(serde_json::Value::as_str)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(ModuleGovernanceError::InvalidValidationJobResultCommand);
        }
        let has_errors = self.errors.iter().any(|error| !error.trim().is_empty());
        match self.outcome {
            ModuleValidationJobResultOutcome::Passed if has_errors => {
                Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
            }
            ModuleValidationJobResultOutcome::Failed if !has_errors => {
                Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
            }
            _ => Ok(()),
        }
    }
}

impl ModuleValidationJobRetryCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.validation_job_id.trim().is_empty()
            || !self.actor_principal.is_object()
            || self.attempt == 0
            || self.error.trim().is_empty()
        {
            return Err(ModuleGovernanceError::InvalidValidationJobRetryCommand);
        }
        Ok(())
    }
}

impl ModulePublishRequestCreateCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        let slug = self.slug.trim();
        let marketplace_content =
            ModuleMarketplaceContentProjection::try_new(&self.name, &self.description)
                .map_err(|_| ModuleGovernanceError::InvalidPublishRequestCreateCommand)?;
        if slug.is_empty()
            || !rustok_api::is_valid_module_slug(slug)
            || Version::parse(self.version.trim()).is_err()
            || self.crate_name.trim().is_empty()
            || rustok_api::normalize_locale_tag(&self.default_locale).is_none()
            || self.ownership.trim().is_empty()
            || self.trust_level.trim().is_empty()
            || self.license.trim().is_empty()
            || marketplace_content.description.chars().count() < 20
            || !valid_marketplace_taxonomy(&self.marketplace)
            || !self.ui_packages.is_object()
            || !self.actor_principal.is_object()
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand);
        }
        Ok(())
    }

    /// Returns owner-derived, content-free publication warnings. Transport
    /// adapters must not invent governance policy or persist caller-provided
    /// warning text.
    pub fn validation_warnings(&self) -> Result<Vec<String>, ModuleGovernanceError> {
        self.validate()?;
        let ui_packages = self
            .ui_packages
            .as_object()
            .expect("validated publish request UI packages must be an object");
        let has_admin = ui_packages
            .get("admin")
            .is_some_and(|value| !value.is_null());
        let has_storefront = ui_packages
            .get("storefront")
            .is_some_and(|value| !value.is_null());

        let mut warnings = Vec::new();
        if !has_admin && !has_storefront {
            warnings.push(
                "No publishable admin/storefront UI packages declared; only backend contract would be published."
                    .to_string(),
            );
        }
        if !self.ownership.eq_ignore_ascii_case("first_party") {
            warnings.push(
                "Third-party publishing requires the configured governance and evidence gates before release."
                    .to_string(),
            );
        }
        Ok(warnings)
    }
}

impl ModuleExternalPrebuiltStageCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        let actor_id = self.context.actor_id.to_string();
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || receipt_digest_sha256(&self.artifact_digest).is_err()
            || receipt_digest_sha256(&self.provenance_digest).is_err()
            || !self.quarantine_approved_by_principal.is_object()
            || !self.actor_principal.is_object()
            || self.context.validate().is_err()
            || self.context.tenant_id.is_some()
            || !matches!(
                governance_principal_user_id(&self.actor_principal),
                Some(principal_actor_id) if principal_actor_id == actor_id.as_str()
            )
            || !matches!(
                governance_principal_user_id(&self.quarantine_approved_by_principal),
                Some(principal_actor_id) if principal_actor_id == actor_id.as_str()
            )
        {
            return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
        }
        for value in [
            &self.provenance_reference,
            &self.quarantine_review_reference,
        ] {
            if value.trim().is_empty()
                || value.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
                || value.contains(char::is_whitespace)
            {
                return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
            }
        }
        for value in [
            &self.provenance_policy_revision,
            &self.quarantine_policy_revision,
        ] {
            if value.trim().is_empty()
                || value.len() > MAX_PUBLICATION_EVIDENCE_POLICY_REVISION_BYTES
                || value.contains(char::is_whitespace)
            {
                return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
            }
        }
        match &self.source_evidence {
            ModuleExternalSourceEvidence::Reproducible { reference, digest } => {
                if reference.trim().is_empty()
                    || reference.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
                    || reference.contains(char::is_whitespace)
                    || receipt_digest_sha256(digest).is_err()
                {
                    return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
                }
            }
            ModuleExternalSourceEvidence::Unavailable { reason_code }
                if !REGISTRY_EXTERNAL_SOURCE_ABSENCE_REASON_CODES
                    .contains(&reason_code.as_str()) =>
            {
                return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
            }
            ModuleExternalSourceEvidence::Unavailable { .. } => {}
        }
        Ok(())
    }
}

impl ModuleAlloyAuthoredStageCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        let actor_id = self.context.actor_id.to_string();
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || self.context.validate().is_err()
            || self.context.tenant_id != Some(self.alloy_tenant_id)
            || self.alloy_tenant_id.is_nil()
            || self.alloy_script_id.is_nil()
            || receipt_digest_sha256(&self.artifact_digest).is_err()
            || receipt_digest_sha256(&self.source_digest).is_err()
            || self.artifact_digest != self.source_digest
            || self.descriptor.validate().is_err()
            || self.descriptor.payload_kind != crate::ArtifactPayloadKind::Rhai
            || self.descriptor.module_kind != crate::ArtifactModuleKind::Optional
            || self.descriptor.artifact_digest != self.artifact_digest
            || self.descriptor.runtime_abi != rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI
            || receipt_digest_sha256(&self.review_digest).is_err()
            || receipt_digest_sha256(&self.sandbox_policy_digest).is_err()
            || receipt_digest_sha256(&self.sandbox_scenario_digest).is_err()
            || self.source_revision == 0
            || self.sandbox_execution_id.is_nil()
            || !self.reviewed_by_principal.is_object()
            || !self.actor_principal.is_object()
            || !matches!(
                governance_principal_user_id(&self.actor_principal),
                Some(principal_actor_id) if principal_actor_id == actor_id.as_str()
            )
            || self
                .parent_release
                .as_ref()
                .is_some_and(|parent| parent.validate().is_err())
        {
            return Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand);
        }
        if self.review_reference.trim().is_empty()
            || self.review_reference.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
            || self.review_reference.contains(char::is_whitespace)
            || self.review_policy_revision.trim().is_empty()
            || self.review_policy_revision.len() > MAX_PUBLICATION_EVIDENCE_POLICY_REVISION_BYTES
            || self.review_policy_revision.contains(char::is_whitespace)
            || self.sandbox_test_path != ALLOY_PUBLICATION_SMOKE_TEST_PATH
            || self.sandbox_scenario_digest != alloy_publication_smoke_scenario_digest()
            || self.sandbox_executor != "rhai"
            || self.sandbox_runtime_abi != rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI
            || self.sandbox_capability_grants != 0
        {
            return Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand);
        }
        Ok(())
    }
}

impl ModulePublishArtifactAttachCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || !is_sha256_hex(&self.checksum_sha256)
            || self.artifact_size < 0
            || self.content_type.trim().is_empty()
            || !valid_command_context_actor(&self.context, &self.actor_principal)
        {
            return Err(ModuleGovernanceError::InvalidPublishArtifactAttachCommand);
        }
        Ok(())
    }
}

impl ModuleAuthorSignatureEvidenceCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.expected_revision < 1
            || !valid_platform_registry_command_context(&self.context, &self.actor_principal)
        {
            return Err(ModuleGovernanceError::InvalidAuthorSignatureEvidenceCommand);
        }
        if self.request_id.trim().is_empty()
            || self.evidence_reference.trim().is_empty()
            || self.evidence_reference.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
            || self.signer_identity.trim().is_empty()
            || self.signer_identity.len() > MAX_PUBLICATION_EVIDENCE_IDENTITY_BYTES
            || self.policy_revision.trim().is_empty()
            || self.policy_revision.len() > MAX_PUBLICATION_EVIDENCE_POLICY_REVISION_BYTES
            || !is_sha256_hex(&self.signature_digest_sha256)
        {
            return Err(ModuleGovernanceError::InvalidAuthorSignatureEvidenceCommand);
        }
        Ok(())
    }

    pub(crate) fn publication_evidence(
        &self,
        subject_digest_sha256: String,
    ) -> ModulePublicationEvidenceCommand {
        ModulePublicationEvidenceCommand {
            request_id: self.request_id.clone(),
            expected_revision: self.expected_revision,
            authority: ModulePublicationEvidenceAuthority::AuthorSignature,
            subject_digest_sha256,
            evidence_reference: self.evidence_reference.clone(),
            issuer_identity: self.signer_identity.clone(),
            policy_revision: self.policy_revision.clone(),
            signature_digest_sha256: Some(self.signature_digest_sha256.clone()),
            actor_principal: self.actor_principal.clone(),
        }
    }
}

impl ModuleBuildServiceAttestationCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.expected_revision < 1
            || self.receipt.signature_authority != ModuleBuildSignatureAuthority::BuildService
        {
            return Err(ModuleGovernanceError::InvalidBuildServiceAttestationCommand);
        }
        let references = [&self.receipt.artifact, &self.receipt.signature_manifest];
        if references
            .iter()
            .any(|reference| reference.validate().is_err())
            || references.iter().skip(1).any(|reference| {
                reference.registry != self.receipt.artifact.registry
                    || reference.repository != self.receipt.artifact.repository
            })
        {
            return Err(ModuleGovernanceError::InvalidBuildServiceAttestationCommand);
        }
        validate_publication_evidence_fields(
            &self.request_id,
            receipt_subject_digest_sha256(&self.receipt)?,
            &format!("oci://{}", self.receipt.signature_manifest.canonical()),
            &self.issuer_identity,
            &self.policy_revision,
            &self.actor_principal,
        )
    }

    pub(crate) fn publication_evidence(
        &self,
    ) -> Result<ModulePublicationEvidenceCommand, ModuleGovernanceError> {
        Ok(ModulePublicationEvidenceCommand {
            request_id: self.request_id.clone(),
            expected_revision: self.expected_revision,
            authority: ModulePublicationEvidenceAuthority::BuildServiceAttestation,
            subject_digest_sha256: receipt_subject_digest_sha256(&self.receipt)?.to_string(),
            evidence_reference: format!("oci://{}", self.receipt.signature_manifest.canonical()),
            issuer_identity: self.issuer_identity.clone(),
            policy_revision: self.policy_revision.clone(),
            signature_digest_sha256: None,
            actor_principal: self.actor_principal.clone(),
        })
    }
}

impl ModulePlatformAdmissionCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.expected_revision < 1
            || crate::normalize_module_registry_id(&self.registry_id).as_deref()
                != Some(self.registry_id.as_str())
        {
            return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
        }
        self.reference
            .validate()
            .map_err(|_| ModuleGovernanceError::InvalidPlatformAdmissionCommand)?;
        self.descriptor
            .validate()
            .map_err(|_| ModuleGovernanceError::InvalidPlatformAdmissionCommand)?;
        if self.request_id.trim().is_empty()
            || self.evidence.manifest_digest != self.reference.digest
            || self.descriptor.artifact_digest != self.evidence.payload_digest
            || self.descriptor.payload_kind == crate::ArtifactPayloadKind::StaticPromoted
            || !prefixed_sha256_digest(&self.evidence.payload_digest)
            || self.evidence.media_type.trim().is_empty()
            || self.evidence.media_type.len() > MAX_PLATFORM_ADMISSION_MEDIA_TYPE_BYTES
            || self.evidence.signer_identity.trim().is_empty()
            || self.evidence.signer_identity.len() > MAX_PUBLICATION_EVIDENCE_IDENTITY_BYTES
            || !self.evidence.admitted()
        {
            return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
        }
        validate_publication_evidence_fields(
            &self.request_id,
            receipt_digest_sha256(&self.reference.digest)
                .map_err(|_| ModuleGovernanceError::InvalidPlatformAdmissionCommand)?,
            &platform_admission_evidence_reference(&self.reference, &self.evidence),
            "rustok-platform-admission",
            &platform_admission_policy_revision(&self.evidence),
            &self.actor_principal,
        )
    }

    pub(crate) fn publication_evidence(
        &self,
        artifact_origin: ModulePublicationArtifactOrigin,
    ) -> Result<ModulePublicationEvidenceCommand, ModuleGovernanceError> {
        let subject_digest_sha256 = match artifact_origin {
            ModulePublicationArtifactOrigin::PlatformBuilt => {
                receipt_digest_sha256(&self.reference.digest)
            }
            ModulePublicationArtifactOrigin::ExternalPrebuilt
            | ModulePublicationArtifactOrigin::AlloyAuthored => {
                receipt_digest_sha256(&self.evidence.payload_digest)
            }
        }
        .map_err(|_| ModuleGovernanceError::InvalidPlatformAdmissionCommand)?;
        Ok(ModulePublicationEvidenceCommand {
            request_id: self.request_id.clone(),
            expected_revision: self.expected_revision,
            authority: ModulePublicationEvidenceAuthority::PlatformAdmission,
            subject_digest_sha256: subject_digest_sha256.to_string(),
            evidence_reference: platform_admission_evidence_reference(
                &self.reference,
                &self.evidence,
            ),
            issuer_identity: "rustok-platform-admission".to_string(),
            policy_revision: platform_admission_policy_revision(&self.evidence),
            signature_digest_sha256: None,
            actor_principal: self.actor_principal.clone(),
        })
    }
}

impl ModulePublishPlatformBuildStageCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        let actor_id = self.context.actor_id.to_string();
        if self.request_id.trim().is_empty()
            || self.expected_revision < 1
            || self.build_request_id.is_nil()
            || !self.actor_principal.is_object()
            || self.context.validate().is_err()
            || !matches!(self.context.tenant_id, Some(tenant_id) if !tenant_id.is_nil())
            || !matches!(
                governance_principal_user_id(&self.actor_principal),
                Some(principal_actor_id) if principal_actor_id == actor_id.as_str()
            )
        {
            return Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand);
        }
        Ok(())
    }
}

