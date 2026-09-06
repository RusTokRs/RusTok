//! Independent durable worker for origin-aware registry artifact validation.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use object_store::ObjectStoreExt;
use rustok_build_publication::{RegistryCredentialBroker, RegistryCredentialError};
use sha2::{Digest, Sha256};

use rustok_modules::{
    ModulePlatformPublicationEvidenceCommand, ModulePlatformPublicationEvidenceProducer,
    ModulePublicationArtifactOrigin, ModulePublicationArtifactRegistryProvider,
    ModuleValidationJobResultCommand, ModuleValidationJobResultOutcome,
    ModuleValidationJobRetryCommand, OciArtifactPublicationTarget, OciArtifactReference,
    OciDistributionArtifactRegistry, SeaOrmModuleGovernanceService,
    validate_module_publish_artifact,
};
use rustok_storage::StorageRuntime;

const ARTIFACT_LOAD_RETRY_DELAYS_SECONDS: &[u64] = &[1, 3, 5];
const OCI_CREDENTIAL_MINIMUM_TTL: Duration = Duration::from_secs(6 * 60);

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
    publication_policy: RegistryValidationPublicationPolicy,
}

impl RegistryValidationWorker {
    pub fn new(
        service: SeaOrmModuleGovernanceService,
        storage: StorageRuntime,
        actor_id: impl Into<String>,
        publication_evidence: Arc<ModulePlatformPublicationEvidenceProducer>,
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
            &work_item.artifact_content_type,
            &artifact,
        );
        let mut warnings = work_item.existing_warnings;
        warnings.extend(validation.warnings);
        dedupe(&mut warnings);
        let artifact_contract_passed = validation.errors.is_empty();
        let platform_evidence = if artifact_contract_passed
            && work_item.artifact_origin == ModulePublicationArtifactOrigin::PlatformBuilt
        {
            let command = self
                .publication_policy
                .command(work_item.request_id.clone(), self.actor_principal.clone())?;
            match self.publication_evidence.produce(command).await {
                Ok(_) => Ok(()),
                Err(error) => {
                    tracing::warn!(
                        request_id = %work_item.request_id,
                        error = %error,
                        "Platform publication evidence verification failed"
                    );
                    Err("Platform publication evidence did not satisfy the isolated supply-chain verification policy.".to_string())
                }
            }
        } else {
            Ok(())
        };
        let platform_evidence_passed = platform_evidence.is_ok();
        let (outcome, errors, automated_checks) = if artifact_contract_passed
            && platform_evidence_passed
        {
            warnings.push("Automated artifact validation passed; follow-up validation stages are still required before publication.".to_string());
            dedupe(&mut warnings);
            (
                ModuleValidationJobResultOutcome::Passed,
                Vec::new(),
                if work_item.artifact_origin == ModulePublicationArtifactOrigin::PlatformBuilt {
                    serde_json::json!([
                        {"check":"artifact_contract","status":"passed"},
                        {"check":"platform_publication_evidence","status":"passed"}
                    ])
                } else {
                    serde_json::json!([{"check":"artifact_contract","status":"passed"}])
                },
            )
        } else {
            let mut errors = validation.errors;
            if let Err(error) = platform_evidence {
                errors.push(error);
            }
            dedupe(&mut errors);
            let automated_checks = if work_item.artifact_origin
                == ModulePublicationArtifactOrigin::PlatformBuilt
            {
                serde_json::json!([
                    {"check":"artifact_contract","status":if artifact_contract_passed {"passed"} else {"failed"}},
                    {"check":"platform_publication_evidence","status":if artifact_contract_passed {"failed"} else {"not_run"}}
                ])
            } else {
                serde_json::json!([{"check":"artifact_contract","status":"failed"}])
            };
            (
                ModuleValidationJobResultOutcome::Failed,
                errors,
                automated_checks,
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
                                automated_checks: serde_json::json!([{"check":"artifact_load","status":"failed"}]),
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
