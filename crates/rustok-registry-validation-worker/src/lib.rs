//! Independent durable worker for origin-aware registry artifact validation.

use sha2::{Digest, Sha256};

use rustok_modules::{
    validate_module_publish_artifact,
    ModuleValidationJobResultCommand, ModuleValidationJobResultOutcome,
    ModuleValidationJobRetryCommand, SeaOrmModuleGovernanceService,
};
use rustok_storage::StorageService;

const ARTIFACT_LOAD_RETRY_DELAYS_SECONDS: &[u64] = &[1, 3, 5];

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
    storage: StorageService,
    actor_principal: serde_json::Value,
}

impl RegistryValidationWorker {
    pub fn new(
        service: SeaOrmModuleGovernanceService,
        storage: StorageService,
        actor_id: impl Into<String>,
    ) -> Result<Self, String> {
        let actor_id = actor_id.into();
        if actor_id.trim().is_empty() {
            return Err("registry validation worker actor ID must be configured".to_string());
        }
        Ok(Self {
            service,
            storage,
            actor_principal: serde_json::json!({"kind":"service","id":actor_id}),
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
        let (outcome, errors, automated_checks) = if validation.errors.is_empty() {
            warnings.push("Automated artifact validation passed; follow-up validation stages are still required before publication.".to_string());
            dedupe(&mut warnings);
            (
                ModuleValidationJobResultOutcome::Passed,
                Vec::new(),
                serde_json::json!([{"check":"artifact_contract","status":"passed"}]),
            )
        } else {
            let mut errors = validation.errors;
            dedupe(&mut errors);
            (
                ModuleValidationJobResultOutcome::Failed,
                errors,
                serde_json::json!([{"check":"artifact_contract","status":"failed"}]),
            )
        };
        self.service
            .apply_validation_job_result(ModuleValidationJobResultCommand {
                validation_job_id: validation_job_id.clone(),
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
            .read(&work_item.artifact_storage_key)
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
