use super::*;
use rustok_modules::{
    ModuleRemoteValidationClaimCommand, ModuleRemoteValidationHeartbeatCommand,
    ModuleRemoteValidationTerminalCommand, ModuleRemoteValidationTerminalOutcome,
    ModuleValidationJobEnqueueCommand, ModuleValidationStageReportCommand,
};

impl RegistryGovernanceService {
    pub async fn validate_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
    ) -> anyhow::Result<RegistryValidationQueueResult> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_manage_publish_request(authority, &request, "validate")
            .await?;

        let was_requeued = match request.status {
            RegistryPublishRequestStatus::Rejected => {
                let latest_event_type = self.latest_request_event_type(&request.id).await?;
                if rejected_publish_request_can_retry(
                    latest_event_type.as_deref(),
                    request.rejection_reason.as_deref(),
                ) {
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
                request_id: request.id.clone(),
                actor_principal: authority.principal.to_json_value(),
                allow_rejected_retry: was_requeued,
            })
            .await
            .map_err(anyhow::Error::new)?;
        let request = self
            .get_publish_request(&result.request_id)
            .await?
            .ok_or_else(|| anyhow!("owner-enqueued registry publish request disappeared"))?;
        Ok(RegistryValidationQueueResult {
            request,
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
    ) -> anyhow::Result<RegistryValidationStageMutationResult> {
        let stage_key = normalize_validation_stage_key(stage_key)?;
        let requested_status = parse_validation_stage_status(status)?;
        let normalized_reason_code = reason_code
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(
            authority,
            &request,
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
                request_id: request.id.clone(),
                stage_key: stage_key.to_string(),
                status: validation_stage_status_label(requested_status).to_string(),
                actor_principal: authority.principal.to_json_value(),
                detail,
                reason_code: normalized_reason_code,
                requeue,
            })
            .await
            .map_err(anyhow::Error::new)?;
        let request = self
            .get_publish_request(&request.id)
            .await?
            .ok_or_else(|| anyhow!("validated registry publish request disappeared"))?;
        let stage = self
            .latest_validation_stage(&request.id, stage_key)
            .await?
            .ok_or_else(|| anyhow!("validated registry stage disappeared"))?;
        Ok(RegistryValidationStageMutationResult { request, stage })
    }

    pub async fn claim_remote_validation_stage(
        &self,
        runner_id: &str,
        supported_stages: &[String],
        lease_ttl_ms: u64,
    ) -> anyhow::Result<Option<RegistryRemoteValidationClaim>> {
        self.publication_service()
            .claim_remote_validation_stage(ModuleRemoteValidationClaimCommand {
                runner_id: runner_id.to_string(),
                supported_stages: supported_stages.to_vec(),
                lease_ttl_ms,
            })
            .await
            .map_err(anyhow::Error::new)
            .map(|claim| {
                claim.map(|claim| RegistryRemoteValidationClaim {
                    artifact_download_url: registry_artifact_download_path(&claim.request_id),
                    claim_id: claim.claim_id,
                    request_id: claim.request_id,
                    slug: claim.slug,
                    version: claim.version,
                    stage_key: claim.stage_key,
                    execution_mode: claim.execution_mode,
                    runnable: true,
                    requires_manual_confirmation: claim.requires_manual_confirmation,
                    allowed_terminal_reason_codes: claim.allowed_terminal_reason_codes,
                    suggested_pass_reason_code: Some(claim.suggested_pass_reason_code),
                    suggested_failure_reason_code: Some(claim.suggested_failure_reason_code),
                    suggested_blocked_reason_code: Some(claim.suggested_blocked_reason_code),
                    artifact_checksum_sha256: claim.artifact_checksum_sha256,
                    crate_name: claim.crate_name,
                })
            })
    }

    pub async fn heartbeat_remote_validation_stage(
        &self,
        claim_id: &str,
        runner_id: &str,
        lease_ttl_ms: u64,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        self.publication_service()
            .heartbeat_remote_validation_stage(ModuleRemoteValidationHeartbeatCommand {
                claim_id: claim_id.to_string(),
                runner_id: runner_id.to_string(),
                lease_ttl_ms,
            })
            .await?;
        self.remote_validation_stage_by_claim_id(claim_id)
            .await?
            .ok_or_else(|| anyhow!("owner-heartbeated validation stage disappeared"))
    }

    pub async fn complete_remote_validation_stage(
        &self,
        claim_id: &str,
        runner_id: &str,
        detail: Option<&str>,
        reason_code: Option<&str>,
    ) -> anyhow::Result<RegistryValidationStageMutationResult> {
        let stage_id = self
            .publication_service()
            .complete_remote_validation_stage(ModuleRemoteValidationTerminalCommand {
                claim_id: claim_id.to_string(),
                runner_id: runner_id.to_string(),
                outcome: ModuleRemoteValidationTerminalOutcome::Passed,
                detail: detail.map(ToString::to_string),
                reason_code: reason_code.map(|value| value.trim().to_ascii_lowercase()),
            })
            .await?;
        let stage = RegistryValidationStageEntity::find_by_id(stage_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("owner-completed validation stage disappeared"))?;
        let request = self
            .get_publish_request(&stage.request_id)
            .await?
            .ok_or_else(|| anyhow!("owner-completed validation request disappeared"))?;
        Ok(RegistryValidationStageMutationResult { request, stage })
    }

    pub async fn fail_remote_validation_stage(
        &self,
        claim_id: &str,
        runner_id: &str,
        detail: Option<&str>,
        reason_code: Option<&str>,
    ) -> anyhow::Result<RegistryValidationStageMutationResult> {
        let stage_id = self
            .publication_service()
            .complete_remote_validation_stage(ModuleRemoteValidationTerminalCommand {
                claim_id: claim_id.to_string(),
                runner_id: runner_id.to_string(),
                outcome: ModuleRemoteValidationTerminalOutcome::Failed,
                detail: detail.map(ToString::to_string),
                reason_code: reason_code.map(|value| value.trim().to_ascii_lowercase()),
            })
            .await?;
        let stage = RegistryValidationStageEntity::find_by_id(stage_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("owner-failed validation stage disappeared"))?;
        let request = self
            .get_publish_request(&stage.request_id)
            .await?
            .ok_or_else(|| anyhow!("owner-failed validation request disappeared"))?;
        Ok(RegistryValidationStageMutationResult { request, stage })
    }

    pub(crate) async fn validation_stage_rows(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Vec<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request_id))
            .order_by_desc(registry_validation_stage::Column::AttemptNumber)
            .order_by_desc(registry_validation_stage::Column::CreatedAt)
            .all(&self.db)
            .await?)
    }

    pub(crate) async fn latest_validation_stages_for_request(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Vec<registry_validation_stage::Model>> {
        let mut latest = HashMap::<String, registry_validation_stage::Model>::new();
        for stage in self.validation_stage_rows(request_id).await? {
            latest.entry(stage.stage_key.clone()).or_insert(stage);
        }
        let mut stages = latest.into_values().collect::<Vec<_>>();
        stages.sort_by(|left, right| left.stage_key.cmp(&right.stage_key));
        Ok(stages)
    }

    async fn latest_validation_stage(
        &self,
        request_id: &str,
        stage_key: &str,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request_id))
            .filter(registry_validation_stage::Column::StageKey.eq(stage_key))
            .order_by_desc(registry_validation_stage::Column::AttemptNumber)
            .order_by_desc(registry_validation_stage::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn remote_validation_stage_by_claim_id(
        &self,
        claim_id: &str,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
            .one(&self.db)
            .await?)
    }

    async fn latest_request_event_type(&self, request_id: &str) -> anyhow::Result<Option<String>> {
        Ok(RegistryGovernanceEventEntity::find()
            .filter(registry_governance_event::Column::RequestId.eq(request_id))
            .order_by_desc(registry_governance_event::Column::CreatedAt)
            .one(&self.db)
            .await?
            .map(|event| event.event_type))
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

pub(crate) fn validation_stage_details_value(
    stage: &registry_validation_stage::Model,
) -> serde_json::Value {
    serde_json::json!({
        "stage_id": stage.id.clone(),
        "stage_key": stage.stage_key.clone(),
        "status": validation_stage_status_label(stage.status.clone()),
        "detail": stage.detail.clone(),
        "attempt_number": stage.attempt_number,
        "queue_reason": stage.queue_reason.clone(),
        "started_at": stage.started_at.as_ref().map(|value| value.to_rfc3339()),
        "finished_at": stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
        "updated_at": stage.updated_at.to_rfc3339(),
    })
}

pub(crate) fn derive_validation_stage_snapshots(
    latest_request: Option<&registry_publish_request::Model>,
    recent_events: &[registry_governance_event::Model],
    stage_rows: &[registry_validation_stage::Model],
) -> Vec<RegistryValidationStageSnapshot> {
    let mut snapshots = Vec::new();
    let mut seen = HashSet::new();
    let mut latest_by_stage = HashMap::new();

    for stage in stage_rows {
        if seen.insert(stage.stage_key.as_str()) {
            latest_by_stage.insert(stage.stage_key.as_str(), stage);
        }
    }

    let required_gates = latest_request
        .map(|request| validation_follow_up_gates_for_origin(&request.artifact_origin))
        .unwrap_or(&[]);
    for stage_key in required_gates {
        if let Some(stage) = latest_by_stage.get(stage_key) {
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: validation_stage_status_label(stage.status.clone()).to_string(),
                detail: stage.detail.clone(),
                attempt_number: stage.attempt_number,
                updated_at: stage.updated_at.to_rfc3339(),
                started_at: stage.started_at.as_ref().map(|value| value.to_rfc3339()),
                finished_at: stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
            });
            continue;
        }

        let latest_event = recent_events.iter().find(|event| {
            matches!(
                event.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event
                .details
                .get("stage_key")
                .and_then(serde_json::Value::as_str)
                == Some(*stage_key)
        });

        if let Some(event) = latest_event {
            let status = event
                .details
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| match event.event_type.as_str() {
                    "follow_up_gate_passed" => "passed",
                    "follow_up_gate_failed" => "failed",
                    _ => "queued",
                });
            let normalized_status = if status.eq_ignore_ascii_case("pending") {
                "queued"
            } else {
                status
            };
            let detail = event
                .details
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| follow_up_gate_detail(stage_key));
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: normalized_status.to_string(),
                detail: detail.to_string(),
                attempt_number: 0,
                updated_at: event.created_at.to_rfc3339(),
                started_at: None,
                finished_at: None,
            });
            continue;
        }

        if latest_request.is_some_and(|request| {
            matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            )
        }) {
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: "queued".to_string(),
                detail: follow_up_gate_detail(stage_key).to_string(),
                attempt_number: 0,
                updated_at: latest_request
                    .and_then(|request| {
                        request
                            .validated_at
                            .as_ref()
                            .or(request.approved_at.as_ref())
                    })
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_default(),
                started_at: None,
                finished_at: None,
            });
        }
    }

    snapshots
}

pub(crate) fn derive_follow_up_gate_snapshots(
    latest_request: Option<&registry_publish_request::Model>,
    recent_events: &[registry_governance_event::Model],
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Vec<RegistryFollowUpGateSnapshot> {
    if !validation_stages.is_empty() {
        return validation_stages
            .iter()
            .map(|stage| RegistryFollowUpGateSnapshot {
                key: stage.key.clone(),
                status: match stage.status.as_str() {
                    "queued" => "pending".to_string(),
                    other => other.to_string(),
                },
                detail: stage.detail.clone(),
                updated_at: stage.updated_at.clone(),
            })
            .collect();
    }

    let mut snapshots = Vec::new();

    let required_gates = latest_request
        .map(|request| validation_follow_up_gates_for_origin(&request.artifact_origin))
        .unwrap_or(&[]);
    for gate in required_gates {
        let latest_event = recent_events.iter().find(|event| {
            matches!(
                event.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event
                .details
                .get("stage_key")
                .and_then(serde_json::Value::as_str)
                == Some(*gate)
        });

        if let Some(event) = latest_event {
            let status = event
                .details
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| match event.event_type.as_str() {
                    "follow_up_gate_passed" => "passed",
                    "follow_up_gate_failed" => "failed",
                    _ => "pending",
                });
            let detail = event
                .details
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| follow_up_gate_detail(gate));

            snapshots.push(RegistryFollowUpGateSnapshot {
                key: (*gate).to_string(),
                status: status.to_string(),
                detail: detail.to_string(),
                updated_at: event.created_at.to_rfc3339(),
            });
            continue;
        }

        if latest_request.is_some_and(|request| {
            matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            )
        }) {
            snapshots.push(RegistryFollowUpGateSnapshot {
                key: (*gate).to_string(),
                status: "pending".to_string(),
                detail: follow_up_gate_detail(gate).to_string(),
                updated_at: latest_request
                    .and_then(|request| {
                        request
                            .validated_at
                            .as_ref()
                            .or(request.approved_at.as_ref())
                    })
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_default(),
            });
        }
    }

    snapshots
}

pub(crate) fn rejected_publish_request_can_retry(
    latest_event_type: Option<&str>,
    rejection_reason: Option<&str>,
) -> bool {
    if matches!(latest_event_type, Some("validation_failed")) {
        return true;
    }

    rejection_reason
        .is_some_and(|reason| !reason.trim().starts_with("Governance rejection reason:"))
}

#[allow(dead_code)]
pub(crate) fn deserialize_message_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect()
}

pub(crate) fn compare_semver_desc(left: &str, right: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(left), semver::Version::parse(right)) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => std::cmp::Ordering::Equal,
    }
}
