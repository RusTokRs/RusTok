//! Independent durable worker for origin-aware registry artifact validation.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use object_store::ObjectStoreExt;
use rustok_build_publication::{
    CosignArtifactSigner, RegistryCredentialBroker, RegistryCredentialError,
    SignedOciArtifactPublicationError, publish_signed_oci_artifact,
};
use sha2::{Digest, Sha256};

use rustok_modules::{
    ArtifactAdmissionLimits, MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES,
    ModuleAlloyPublicationEvidenceCommand, ModuleAlloyPublicationEvidenceProducer,
    ModuleGovernanceAutomatedCheck, ModulePlatformPublicationEvidenceCommand,
    ModulePlatformPublicationEvidenceProducer, ModulePublicationArtifactOrigin,
    ModulePublicationArtifactRegistryProvider, ModuleValidationJobResultCommand,
    ModuleValidationJobResultOutcome, ModuleValidationJobRetryCommand,
    OciArtifactPublicationBundle, OciArtifactPublicationTarget, OciArtifactReference,
    OciDistributionArtifactRegistry, OciRhaiWorkspacePublicationProvenance,
    SeaOrmModuleGovernanceService, validate_module_publish_artifact,
};
use rustok_storage::StorageRuntime;

const ARTIFACT_LOAD_RETRY_DELAYS_SECONDS: &[u64] = &[1, 3, 5];
const OCI_CREDENTIAL_MINIMUM_TTL: Duration = Duration::from_secs(6 * 60);
const ALLOY_OCI_PUBLICATION_TIMEOUT: Duration = Duration::from_secs(14 * 60);
const OCI_CREDENTIAL_LEASE_SAFETY_MARGIN: Duration = Duration::from_secs(30);

fn automated_check(key: &str, status: &str, detail: &str) -> ModuleGovernanceAutomatedCheck {
    ModuleGovernanceAutomatedCheck {
        key: key.to_string(),
        status: status.to_string(),
        detail: Some(detail.to_string()),
    }
}

fn successful_validation_checks(
    origin: ModulePublicationArtifactOrigin,
) -> Vec<ModuleGovernanceAutomatedCheck> {
    let mut checks = vec![automated_check(
        "artifact_contract",
        "passed",
        "Artifact contract validation passed.",
    )];
    match origin {
        ModulePublicationArtifactOrigin::PlatformBuilt => checks.push(automated_check(
            "platform_publication_evidence",
            "passed",
            "Platform publication evidence verification passed.",
        )),
        ModulePublicationArtifactOrigin::AlloyAuthored => {
            checks.push(automated_check(
                "alloy_oci_publication",
                "passed",
                "Canonical Rhai workspace OCI publication and signature passed.",
            ));
            checks.push(automated_check(
                "platform_admission",
                "passed",
                "Independent platform admission of the signed Alloy OCI package passed.",
            ));
        }
        ModulePublicationArtifactOrigin::ExternalPrebuilt => {}
    }
    checks
}

fn failed_validation_checks(
    origin: ModulePublicationArtifactOrigin,
    artifact_contract_passed: bool,
) -> Vec<ModuleGovernanceAutomatedCheck> {
    let artifact_status = if artifact_contract_passed {
        "passed"
    } else {
        "failed"
    };
    let artifact_detail = if artifact_contract_passed {
        "Artifact contract validation passed."
    } else {
        "Artifact contract validation failed."
    };
    let mut checks = vec![automated_check(
        "artifact_contract",
        artifact_status,
        artifact_detail,
    )];
    match origin {
        ModulePublicationArtifactOrigin::PlatformBuilt => checks.push(automated_check(
            "platform_publication_evidence",
            if artifact_contract_passed {
                "failed"
            } else {
                "not_run"
            },
            if artifact_contract_passed {
                "Platform publication evidence verification failed."
            } else {
                "Not run because artifact contract validation failed."
            },
        )),
        ModulePublicationArtifactOrigin::AlloyAuthored => {
            checks.push(automated_check(
                "alloy_oci_publication",
                if artifact_contract_passed {
                    "failed"
                } else {
                    "not_run"
                },
                if artifact_contract_passed {
                    "Canonical Rhai workspace OCI publication or signature failed."
                } else {
                    "Not run because artifact contract validation failed."
                },
            ));
            checks.push(automated_check(
                "platform_admission",
                "not_run",
                "Not recorded because the signed Alloy OCI package was not fully admitted.",
            ));
        }
        ModulePublicationArtifactOrigin::ExternalPrebuilt => {}
    }
    checks
}

/// Deployment-owned policy revisions and identities used only when a claimed
/// platform-built bundle reaches supply-chain verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryValidationPublicationPolicy {
    pub registry_id: String,
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub build_service_issuer_identity: String,
    pub build_service_policy_revision: String,
}

impl RegistryValidationPublicationPolicy {
    fn command(
        &self,
        request_id: String,
        actor_principal: serde_json::Value,
    ) -> Result<ModulePlatformPublicationEvidenceCommand, String> {
        let command = ModulePlatformPublicationEvidenceCommand {
            request_id,
            registry_id: self.registry_id.clone(),
            trust_policy_revision: self.trust_policy_revision,
            capability_policy_revision: self.capability_policy_revision,
            build_service_issuer_identity: self.build_service_issuer_identity.clone(),
            build_service_policy_revision: self.build_service_policy_revision.clone(),
            actor_principal,
        };
        command.validate().map_err(|error| error.to_string())?;
        Ok(command)
    }

    fn alloy_command(
        &self,
        request_id: String,
        artifact: OciArtifactReference,
        actor_principal: serde_json::Value,
    ) -> Result<ModuleAlloyPublicationEvidenceCommand, String> {
        let command = ModuleAlloyPublicationEvidenceCommand {
            request_id,
            registry_id: self.registry_id.clone(),
            artifact,
            trust_policy_revision: self.trust_policy_revision,
            capability_policy_revision: self.capability_policy_revision,
            actor_principal,
        };
        command.validate().map_err(|error| error.to_string())?;
        Ok(command)
    }
}

/// Deployment-owned adapters required to turn a validated Alloy workspace into
/// a signed, digest-pinned OCI package and record its independent admission.
#[derive(Clone)]
pub struct RegistryValidationAlloyPublication {
    evidence: Arc<ModuleAlloyPublicationEvidenceProducer>,
    target: OciArtifactPublicationTarget,
    credentials: Arc<dyn RegistryCredentialBroker>,
    signer: Arc<CosignArtifactSigner>,
}

impl RegistryValidationAlloyPublication {
    pub fn new(
        evidence: Arc<ModuleAlloyPublicationEvidenceProducer>,
        target: OciArtifactPublicationTarget,
        credentials: Arc<dyn RegistryCredentialBroker>,
        signer: Arc<CosignArtifactSigner>,
    ) -> Result<Self, String> {
        target.validate().map_err(|error| error.to_string())?;
        if !credentials.is_ready() || !signer.is_ready() {
            return Err("Alloy OCI publication adapters are not ready".to_string());
        }
        Ok(Self {
            evidence,
            target,
            credentials,
            signer,
        })
    }
}

/// Short-lived credential adapter for exact staged OCI registry/repository
/// identities. The lease never enters the modules owner or validation result.
pub struct CredentialedOciRegistryProvider {
    credentials: Arc<dyn RegistryCredentialBroker>,
}

impl CredentialedOciRegistryProvider {
    pub fn new(credentials: Arc<dyn RegistryCredentialBroker>) -> Result<Self, String> {
        if !credentials.is_ready() {
            return Err("registry credential broker is not ready".to_string());
        }
        Ok(Self { credentials })
    }
}

#[async_trait]
impl ModulePublicationArtifactRegistryProvider for CredentialedOciRegistryProvider {
    async fn registry_for(
        &self,
        reference: &OciArtifactReference,
    ) -> Result<Arc<dyn rustok_modules::ArtifactRegistry>, String> {
        reference.validate().map_err(|error| error.to_string())?;
        let target = OciArtifactPublicationTarget {
            registry: reference.registry.clone(),
            repository: reference.repository.clone(),
        };
        let lease = self
            .credentials
            .acquire(&target, OCI_CREDENTIAL_MINIMUM_TTL)
            .await
            .map_err(|error| match error {
                RegistryCredentialError::Rejected => {
                    "registry credential request was rejected".to_string()
                }
                RegistryCredentialError::TimedOut => {
                    "registry credential request timed out".to_string()
                }
                RegistryCredentialError::Unavailable(_) => {
                    "registry credential broker is unavailable".to_string()
                }
            })?;
        lease
            .ensure_valid()
            .map_err(|_| "registry credential broker returned an expired credential".to_string())?;
        let registry = OciDistributionArtifactRegistry::strict(lease.registry_auth())
            .map_err(|error| error.to_string())?;
        Ok(Arc::new(registry))
    }
}

/// Outcome of the worker-owned artifact-read retry policy. A terminal failure
/// has already been durably recorded by the owner and is therefore a completed
/// queue delivery, not an iteration error for the host process.
enum ArtifactLoadOutcome {
    Loaded(Vec<u8>),
    Terminalized,
}

/// Executes claimed validation jobs without an HTTP server dependency.
#[derive(Clone)]
pub struct RegistryValidationWorker {
    service: SeaOrmModuleGovernanceService,
    storage: StorageRuntime,
    actor_principal: serde_json::Value,
    publication_evidence: Arc<ModulePlatformPublicationEvidenceProducer>,
    alloy_publication: RegistryValidationAlloyPublication,
    publication_policy: RegistryValidationPublicationPolicy,
}

impl RegistryValidationWorker {
    pub fn new(
        service: SeaOrmModuleGovernanceService,
        storage: StorageRuntime,
        actor_id: impl Into<String>,
        publication_evidence: Arc<ModulePlatformPublicationEvidenceProducer>,
        alloy_publication: RegistryValidationAlloyPublication,
        publication_policy: RegistryValidationPublicationPolicy,
    ) -> Result<Self, String> {
        let actor_id = actor_id.into();
        if actor_id.trim().is_empty() {
            return Err("registry validation worker actor ID must be configured".to_string());
        }
        let actor_principal = serde_json::json!({"kind":"service","id":actor_id});
        publication_policy.command("configuration-probe".to_string(), actor_principal.clone())?;
        Ok(Self {
            service,
            storage,
            actor_principal,
            publication_evidence,
            alloy_publication,
            publication_policy,
        })
    }

    /// Claims and processes one durable queue item. The caller may poll again
    /// after `Ok(None)`; broker delivery is deliberately not required.
    pub async fn process_next(&self) -> Result<Option<String>, String> {
        let Some(claim) = self
            .service
            .claim_next_validation_job(self.actor_principal.clone())
            .await
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        if !claim.should_run {
            return Ok(None);
        }
        let work_item = claim
            .work_item
            .ok_or_else(|| "claimed validation job is missing immutable work item".to_string())?;
        let validation_job_id = work_item.validation_job_id.clone();
        let artifact = match self.load_artifact_with_retry(&work_item).await? {
            ArtifactLoadOutcome::Loaded(artifact) => artifact,
            ArtifactLoadOutcome::Terminalized => return Ok(Some(validation_job_id)),
        };
        let validation = validate_module_publish_artifact(
            work_item.artifact_origin,
            &work_item.contract,
            work_item.alloy_descriptor.as_ref(),
            &work_item.artifact_content_type,
            &artifact,
        );
        let mut warnings = work_item.existing_warnings.clone();
        warnings.extend(validation.warnings);
        dedupe(&mut warnings);
        let artifact_contract_passed = validation.errors.is_empty();
        let supply_chain_evidence = if artifact_contract_passed {
            match work_item.artifact_origin {
                ModulePublicationArtifactOrigin::PlatformBuilt => {
                    let command = self.publication_policy.command(
                        work_item.request_id.clone(),
                        self.actor_principal.clone(),
                    )?;
                    self.publication_evidence
                        .produce(command)
                        .await
                        .map(|_| ())
                        .map_err(|error| {
                            tracing::warn!(
                                request_id = %work_item.request_id,
                                error = %error,
                                "Platform publication evidence verification failed"
                            );
                            "Platform publication evidence did not satisfy the isolated supply-chain verification policy.".to_string()
                        })
                }
                ModulePublicationArtifactOrigin::AlloyAuthored => self
                    .publish_alloy_workspace(&work_item, artifact)
                    .await
                    .map_err(|error| {
                        tracing::warn!(
                            request_id = %work_item.request_id,
                            error = %error,
                            "Alloy OCI publication or platform admission failed"
                        );
                        "Alloy OCI publication did not satisfy the isolated supply-chain verification policy.".to_string()
                    }),
                ModulePublicationArtifactOrigin::ExternalPrebuilt => Ok(()),
            }
        } else {
            Ok(())
        };
        let supply_chain_evidence_passed = supply_chain_evidence.is_ok();
        let (outcome, errors, automated_checks) = if artifact_contract_passed
            && supply_chain_evidence_passed
        {
            warnings.push("Automated artifact validation passed; follow-up validation stages are still required before publication.".to_string());
            dedupe(&mut warnings);
            (
                ModuleValidationJobResultOutcome::Passed,
                Vec::new(),
                successful_validation_checks(work_item.artifact_origin),
            )
        } else {
            let mut errors = validation.errors;
            if let Err(error) = supply_chain_evidence {
                errors.push(error);
            }
            dedupe(&mut errors);
            (
                ModuleValidationJobResultOutcome::Failed,
                errors,
                failed_validation_checks(work_item.artifact_origin, artifact_contract_passed),
            )
        };
        self.service
            .apply_validation_job_result(ModuleValidationJobResultCommand {
                validation_job_id: validation_job_id.clone(),
                expected_request_revision: work_item.expected_request_revision,
                actor_principal: self.actor_principal.clone(),
                outcome,
                warnings,
                errors,
                automated_checks,
            })
            .await
            .map_err(|error| error.to_string())?;
        Ok(Some(validation_job_id))
    }

    async fn publish_alloy_workspace(
        &self,
        work_item: &rustok_modules::ModuleValidationJobWorkItem,
        artifact: Vec<u8>,
    ) -> Result<(), String> {
        let receipted_descriptor = work_item.alloy_descriptor.as_ref().ok_or_else(|| {
            "claimed Alloy validation work item has no receipt descriptor".to_string()
        })?;
        let source = self
            .service
            .load_alloy_publication_source(&work_item.request_id)
            .await
            .map_err(|error| format!("owner source reload failed: {error}"))?;
        if source.request_id != work_item.request_id
            || source.request_revision != work_item.expected_request_revision
            || source.slug != work_item.slug
            || source.version != work_item.version
            || source.descriptor != *receipted_descriptor
            || source.source_digest != format!("sha256:{}", work_item.artifact_checksum_sha256)
        {
            return Err(
                "owner source no longer matches the claimed Alloy validation receipt".to_string(),
            );
        }
        let publication_limits = ArtifactAdmissionLimits {
            max_descriptor_bytes: ArtifactAdmissionLimits::default().max_descriptor_bytes,
            max_payload_bytes: MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES as u64,
        };
        let bundle = OciArtifactPublicationBundle::from_verified_rhai_workspace(
            source.descriptor.clone(),
            artifact,
            &source.license,
            OciRhaiWorkspacePublicationProvenance {
                request_id: source.request_id.clone(),
                alloy_tenant_id: source.alloy_tenant_id,
                alloy_script_id: source.alloy_script_id,
                source_revision: source.source_revision,
                source_digest: source.source_digest.clone(),
                review_digest: source.review_digest.clone(),
                descriptor_digest: source.descriptor_digest.clone(),
            },
            publication_limits,
        )
        .map_err(|error| format!("canonical Alloy OCI bundle construction failed: {error}"))?;
        let receipt = publish_signed_oci_artifact(
            self.alloy_publication.credentials.as_ref(),
            self.alloy_publication.signer.as_ref(),
            &self.alloy_publication.target,
            bundle,
            publication_limits,
            ALLOY_OCI_PUBLICATION_TIMEOUT,
            OCI_CREDENTIAL_LEASE_SAFETY_MARGIN,
        )
        .await
        .map_err(|error| match error {
            SignedOciArtifactPublicationError::Rejected => {
                "canonical Alloy OCI publication was rejected".to_string()
            }
            SignedOciArtifactPublicationError::TimedOut => {
                "canonical Alloy OCI publication timed out".to_string()
            }
            SignedOciArtifactPublicationError::Unavailable(error) => {
                format!("canonical Alloy OCI publication infrastructure is unavailable: {error}")
            }
        })?;
        if receipt.artifact.registry != self.alloy_publication.target.registry
            || receipt.artifact.repository != self.alloy_publication.target.repository
        {
            return Err(
                "OCI publisher returned a reference outside the configured Alloy target"
                    .to_string(),
            );
        }
        let command = self.publication_policy.alloy_command(
            work_item.request_id.clone(),
            receipt.artifact,
            self.actor_principal.clone(),
        )?;
        self.alloy_publication
            .evidence
            .produce(command)
            .await
            .map_err(|error| format!("independent Alloy platform admission failed: {error}"))?;
        Ok(())
    }

    async fn load_artifact_with_retry(
        &self,
        work_item: &rustok_modules::ModuleValidationJobWorkItem,
    ) -> Result<ArtifactLoadOutcome, String> {
        for attempt in 1..=ARTIFACT_LOAD_RETRY_DELAYS_SECONDS.len() + 1 {
            match self.load_artifact(work_item).await {
                Ok(bytes) => return Ok(ArtifactLoadOutcome::Loaded(bytes)),
                Err(error) => {
                    let retry_after_seconds =
                        ARTIFACT_LOAD_RETRY_DELAYS_SECONDS.get(attempt - 1).copied();
                    self.service
                        .record_validation_job_retry(ModuleValidationJobRetryCommand {
                            validation_job_id: work_item.validation_job_id.clone(),
                            actor_principal: self.actor_principal.clone(),
                            attempt: attempt as u32,
                            retry_after_seconds,
                            error: "registry validation artifact load failed".to_string(),
                        })
                        .await
                        .map_err(|owner_error| owner_error.to_string())?;
                    if let Some(delay) = retry_after_seconds {
                        tracing::warn!(
                            validation_job_id = %work_item.validation_job_id,
                            attempt,
                            error = %error,
                            "Registry validation artifact load failed; retrying"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    } else {
                        self.service
                            .apply_validation_job_result(ModuleValidationJobResultCommand {
                                validation_job_id: work_item.validation_job_id.clone(),
                                expected_request_revision: work_item.expected_request_revision,
                                actor_principal: self.actor_principal.clone(),
                                outcome: ModuleValidationJobResultOutcome::Failed,
                                warnings: work_item.existing_warnings.clone(),
                                errors: vec!["Validation job exhausted artifact-load retries before artifact checks.".to_string()],
                                automated_checks: vec![automated_check(
                                    "artifact_load",
                                    "failed",
                                    "Artifact could not be loaded after the retry budget was exhausted.",
                                )],
                            })
                            .await
                            .map_err(|owner_error| owner_error.to_string())?;
                        return Ok(ArtifactLoadOutcome::Terminalized);
                    }
                }
            }
        }
        unreachable!("retry delay schedule always has a terminal attempt")
    }

    async fn load_artifact(
        &self,
        work_item: &rustok_modules::ModuleValidationJobWorkItem,
    ) -> Result<Vec<u8>, String> {
        let bytes = self
            .storage
            .objects
            .get(&object_store::path::Path::from(
                work_item.artifact_storage_key.as_str(),
            ))
            .await
            .map_err(|error| error.to_string())?
            .bytes()
            .await
            .map_err(|error| error.to_string())?;
        if u64::try_from(bytes.len()).ok() != Some(work_item.artifact_size) {
            return Err(
                "registry validation artifact size does not match the claimed work item"
                    .to_string(),
            );
        }
        if hex::encode(Sha256::digest(&bytes)) != work_item.artifact_checksum_sha256 {
            return Err(
                "registry validation artifact checksum does not match the claimed work item"
                    .to_string(),
            );
        }
        Ok(bytes.to_vec())
    }
}

fn dedupe(values: &mut Vec<String>) {
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
}
