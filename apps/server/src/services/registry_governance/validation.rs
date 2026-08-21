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
                expected_revision: request.request.revision,
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
        reason_code: Option<&str>,
        requeue: bool,
    ) -> anyhow::Result<RegistryValidationStageReportResult> {
        let request = self
            .authorized_publish_request_status_snapshot(
                request_id,
                authority,
                RegistryPublishRequestPermission::Review,
                "update validation stage",
            )
            .await?;

        self.publication_service()
            .report_validation_stage(ModuleValidationStageReportCommand {
                request_id: request.request.id.clone(),
                expected_revision: request.request.revision,
                stage_key: stage_key.to_string(),
                status: status.to_string(),
                actor_principal: authority.principal.to_json_value(),
                reason_code: reason_code.map(ToString::to_string),
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
            .find(|stage| stage.key.eq_ignore_ascii_case(stage_key.trim()))
            .cloned()
            .ok_or_else(|| anyhow!("validated registry stage disappeared"))?;
        Ok(RegistryValidationStageReportResult { status, stage })
    }
}
