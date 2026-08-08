use super::*;
use rustok_modules::{ModuleValidationJobEnqueueCommand, ModuleValidationStageReportCommand};

impl RegistryGovernanceService {
    pub async fn validate_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
    ) -> anyhow::Result<RegistryValidationQueueResult> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Manage,
                "validate",
            )
            .await?;

        let was_requeued = match request.request.status.as_str() {
            "rejected" => {
                if request.rejected_retry_allowed {
                    true
                } else {
                    return Err(conflict_error(format!(
                        "Registry publish request '{}' was manually rejected by governance review and cannot be revalidated; create a new publish request instead",
                        request_id
                    )));
                }
            }
            _ => false,
        };

        let result = self
            .publication_service()
            .enqueue_validation_job(ModuleValidationJobEnqueueCommand {
                request_id: request.request.id.clone(),
                actor_principal: authority.principal.to_json_value(),
                allow_rejected_retry: was_requeued,
            })
            .await
            .map_err(anyhow::Error::new)?;
        let status = self
            .publish_request_status_snapshot_for_authority(&result.request_id, Some(authority))
            .await?
            .ok_or_else(|| anyhow!("owner-enqueued registry publish request disappeared"))?;
        Ok(RegistryValidationQueueResult {
            status,
            queued: result.queued,
            validation_job_id: result.validation_job_id,
        })
    }

    pub async fn report_validation_stage(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        stage_key: &str,
        status: &str,
        detail: Option<&str>,
        reason_code: Option<&str>,
        requeue: bool,
    ) -> anyhow::Result<RegistryValidationStageReportResult> {
        let stage_key = normalize_validation_stage_key(stage_key)?;
        let requested_status = parse_validation_stage_status(status)?;
        let normalized_reason_code = reason_code
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Review,
                "update validation stage",
            )
            .await?;
        let detail = detail
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| default_validation_stage_detail(stage_key, &requested_status));

        self.publication_service()
            .report_validation_stage(ModuleValidationStageReportCommand {
                request_id: request.request.id.clone(),
                stage_key: stage_key.to_string(),
                status: validation_stage_status_label(requested_status).to_string(),
                actor_principal: authority.principal.to_json_value(),
                detail,
                reason_code: normalized_reason_code,
                requeue,
            })
            .await
            .map_err(anyhow::Error::new)?;
        let status = self
            .publish_request_status_snapshot_for_authority(&request.request.id, Some(authority))
            .await?
            .ok_or_else(|| anyhow!("validated registry publish request disappeared"))?;
        let stage = status
            .validation_stages
            .iter()
            .find(|stage| stage.key == stage_key)
            .cloned()
            .ok_or_else(|| anyhow!("validated registry stage disappeared"))?;
        Ok(RegistryValidationStageReportResult { status, stage })
    }
}

pub fn validation_stage_status_label(status: RegistryValidationStageStatus) -> &'static str {
    match status {
        RegistryValidationStageStatus::Queued => "queued",
        RegistryValidationStageStatus::Running => "running",
        RegistryValidationStageStatus::Passed => "passed",
        RegistryValidationStageStatus::Failed => "failed",
        RegistryValidationStageStatus::Blocked => "blocked",
    }
}

fn parse_validation_stage_status(value: &str) -> anyhow::Result<RegistryValidationStageStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "queued" => Ok(RegistryValidationStageStatus::Queued),
        "running" => Ok(RegistryValidationStageStatus::Running),
        "passed" => Ok(RegistryValidationStageStatus::Passed),
        "failed" => Ok(RegistryValidationStageStatus::Failed),
        "blocked" => Ok(RegistryValidationStageStatus::Blocked),
        other => Err(malformed_error(format!(
            "Unsupported validation stage status '{}'; expected queued, running, passed, failed, or blocked",
            other
        ))),
    }
}

fn normalize_validation_stage_key(value: &str) -> anyhow::Result<&str> {
    let value = value.trim();
    if REGISTRY_VALIDATION_FOLLOW_UP_GATES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(value))
    {
        let canonical = REGISTRY_VALIDATION_FOLLOW_UP_GATES
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(value))
            .copied()
            .expect("validated gate must exist");
        return Ok(canonical);
    }

    Err(malformed_error(format!(
        "Unsupported validation stage '{}'; expected one of {}",
        value,
        REGISTRY_VALIDATION_FOLLOW_UP_GATES.join(", ")
    )))
}

fn default_validation_stage_detail(
    stage_key: &str,
    status: &RegistryValidationStageStatus,
) -> String {
    match status {
        RegistryValidationStageStatus::Queued => follow_up_gate_detail(stage_key).to_string(),
        RegistryValidationStageStatus::Running => {
            format!("Validation stage '{stage_key}' is now running.")
        }
        RegistryValidationStageStatus::Passed => {
            format!("Validation stage '{stage_key}' passed.")
        }
        RegistryValidationStageStatus::Failed => {
            format!("Validation stage '{stage_key}' failed.")
        }
        RegistryValidationStageStatus::Blocked => {
            format!("Validation stage '{stage_key}' is blocked on external follow-up.")
        }
    }
}
