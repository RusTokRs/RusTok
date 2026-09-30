//! Database row mappings, automated checks, and governance action derivation.

use sea_orm::QueryResult;

use super::*;

pub(crate) struct GovernanceRequestRow {
    snapshot: ModuleGovernanceRequestSnapshot,
    validated_at: Option<String>,
    approved_at: Option<String>,
}

pub(crate) struct GovernanceEventRow {
    snapshot: ModuleGovernanceEventSnapshot,
    details: serde_json::Value,
}

pub(crate) fn map_governance_owner_snapshot(
    row: &QueryResult,
) -> Result<ModuleGovernanceOwnerSnapshot, ModuleGovernanceError> {
    Ok(ModuleGovernanceOwnerSnapshot {
        owner_principal: required_json_text(row, "owner_principal")?,
        bound_by_principal: required_json_text(row, "bound_by_principal")?,
        bound_at: required_timestamp(row, "bound_at")?,
        updated_at: required_timestamp(row, "updated_at")?,
    })
}

pub(crate) fn map_governance_request_row(
    row: &QueryResult,
) -> Result<GovernanceRequestRow, ModuleGovernanceError> {
    Ok(GovernanceRequestRow {
        snapshot: ModuleGovernanceRequestSnapshot {
            id: required_column(row, "id")?,
            revision: required_column(row, "revision")?,
            slug: required_column(row, "slug")?,
            version: required_column(row, "version")?,
            status: required_column(row, "status")?,
            artifact_origin: required_column(row, "artifact_origin")?,
            requested_by_principal: required_json_text(row, "requested_by_principal")?,
            publisher_principal: optional_json_text(row, "publisher_principal")?,
            approved_by_principal: optional_json_text(row, "approved_by_principal")?,
            rejected_by_principal: optional_json_text(row, "rejected_by_principal")?,
            rejection_reason: optional_column(row, "rejection_reason")?,
            changes_requested_by_principal: optional_json_text(
                row,
                "changes_requested_by_principal",
            )?,
            changes_requested_reason: optional_column(row, "changes_requested_reason")?,
            changes_requested_reason_code: optional_column(row, "changes_requested_reason_code")?,
            changes_requested_at: optional_timestamp(row, "changes_requested_at")?,
            held_by_principal: optional_json_text(row, "held_by_principal")?,
            held_reason: optional_column(row, "held_reason")?,
            held_reason_code: optional_column(row, "held_reason_code")?,
            held_at: optional_timestamp(row, "held_at")?,
            held_from_status: optional_column(row, "held_from_status")?,
            warnings: json_string_list(optional_json_text(row, "validation_warnings")?)?,
            errors: json_string_list(optional_json_text(row, "validation_errors")?)?,
            created_at: required_timestamp(row, "created_at")?,
            updated_at: required_timestamp(row, "updated_at")?,
            published_at: optional_timestamp(row, "published_at")?,
        },
        validated_at: optional_timestamp(row, "validated_at")?,
        approved_at: optional_timestamp(row, "approved_at")?,
    })
}

pub(crate) fn map_governance_release_snapshot(
    row: &QueryResult,
) -> Result<ModuleGovernanceReleaseSnapshot, ModuleGovernanceError> {
    Ok(ModuleGovernanceReleaseSnapshot {
        version: required_column(row, "version")?,
        status: required_column(row, "status")?,
        publisher_principal: required_json_text(row, "publisher_principal")?,
        checksum_sha256: optional_column(row, "checksum_sha256")?,
        published_at: required_timestamp(row, "published_at")?,
        yanked_reason: optional_column(row, "yanked_reason")?,
        yanked_by_principal: optional_json_text(row, "yanked_by_principal")?,
        yanked_at: optional_timestamp(row, "yanked_at")?,
    })
}

pub(crate) fn map_governance_event_row(
    row: &QueryResult,
) -> Result<GovernanceEventRow, ModuleGovernanceError> {
    let details = optional_json_text(row, "details")?.unwrap_or(serde_json::Value::Null);
    Ok(GovernanceEventRow {
        snapshot: ModuleGovernanceEventSnapshot {
            id: required_column(row, "id")?,
            event_type: required_column(row, "event_type")?,
            actor_principal: required_json_text(row, "actor_principal")?,
            publisher_principal: optional_json_text(row, "publisher_principal")?,
            payload: governance_event_payload(&details),
            created_at: required_timestamp(row, "created_at")?,
        },
        details,
    })
}

pub(crate) fn derive_governance_validation_stages(
    latest_request: Option<&GovernanceRequestRow>,
    recent_events: &[GovernanceEventRow],
    stage_rows: &[QueryResult],
) -> Result<Vec<ModuleGovernanceValidationStageSnapshot>, ModuleGovernanceError> {
    let Some(request) = latest_request else {
        return Ok(Vec::new());
    };
    let origin = ModulePublicationArtifactOrigin::parse(&request.snapshot.artifact_origin)
        .ok_or_else(|| {
            ModuleGovernanceError::InvalidLifecycleArtifactOrigin(
                request.snapshot.artifact_origin.clone(),
            )
        })?;
    let required_stages = publication_follow_up_stages(origin);
    let mut latest_by_key = std::collections::HashMap::new();
    for row in stage_rows {
        let key: String = required_column(row, "stage_key")?;
        latest_by_key.entry(key).or_insert(row);
    }

    let mut snapshots = Vec::new();
    for required_stage in required_stages {
        if let Some(row) = latest_by_key.get(required_stage.key) {
            snapshots.push(map_governance_validation_stage_row(row)?);
            continue;
        }

        if let Some(event) = recent_events.iter().find(|event| {
            matches!(
                event.snapshot.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event
                .details
                .get("stage_key")
                .and_then(serde_json::Value::as_str)
                == Some(required_stage.key)
        }) {
            let status = event
                .details
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(match event.snapshot.event_type.as_str() {
                    "follow_up_gate_passed" => "passed",
                    "follow_up_gate_failed" => "failed",
                    _ => "queued",
                });
            snapshots.push(governance_validation_stage_snapshot(
                required_stage.key,
                if status.eq_ignore_ascii_case("pending") {
                    "queued"
                } else {
                    status
                },
                event
                    .details
                    .get("detail")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_else(|| governance_gate_detail(required_stage.key)),
                0,
                event.snapshot.created_at.clone(),
                None,
                None,
            ));
            continue;
        }

        if matches!(request.snapshot.status.as_str(), "approved" | "published") {
            snapshots.push(governance_validation_stage_snapshot(
                required_stage.key,
                "queued",
                governance_gate_detail(required_stage.key),
                0,
                request
                    .validated_at
                    .as_ref()
                    .or(request.approved_at.as_ref())
                    .cloned()
                    .unwrap_or_default(),
                None,
                None,
            ));
        }
    }
    Ok(snapshots)
}

pub(crate) fn map_governance_validation_stage_row(
    row: &QueryResult,
) -> Result<ModuleGovernanceValidationStageSnapshot, ModuleGovernanceError> {
    let key: String = required_column(row, "stage_key")?;
    Ok(governance_validation_stage_snapshot(
        &key,
        &required_column::<String>(row, "status")?,
        &required_column::<String>(row, "detail")?,
        required_column(row, "attempt_number")?,
        required_timestamp(row, "updated_at")?,
        optional_timestamp(row, "started_at")?,
        optional_timestamp(row, "finished_at")?,
    ))
}

pub(crate) fn governance_validation_stage_snapshot(
    key: &str,
    status: &str,
    detail: &str,
    attempt_number: i32,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
) -> ModuleGovernanceValidationStageSnapshot {
    ModuleGovernanceValidationStageSnapshot {
        key: key.to_string(),
        status: status.to_string(),
        detail: detail.to_string(),
        attempt_number,
        updated_at,
        started_at,
        finished_at,
        execution_mode: governance_stage_execution_mode(key).to_string(),
        runnable: matches!(
            key,
            "compile_smoke" | "targeted_tests" | "security_policy_review"
        ),
        requires_manual_confirmation: key == "security_policy_review",
        allowed_terminal_reason_codes: REGISTRY_VALIDATION_STAGE_REASON_CODES
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        suggested_pass_reason_code: governance_stage_pass_reason_code(key).map(str::to_string),
        suggested_failure_reason_code: governance_stage_failure_reason_code(key)
            .map(str::to_string),
        suggested_blocked_reason_code: governance_stage_blocked_reason_code(key)
            .map(str::to_string),
    }
}

pub(crate) fn derive_governance_follow_up_gates(
    latest_request: Option<&GovernanceRequestRow>,
    recent_events: &[GovernanceEventRow],
    validation_stages: &[ModuleGovernanceValidationStageSnapshot],
) -> Result<Vec<ModuleGovernanceGateSnapshot>, ModuleGovernanceError> {
    if !validation_stages.is_empty() {
        return Ok(validation_stages
            .iter()
            .map(|stage| ModuleGovernanceGateSnapshot {
                key: stage.key.clone(),
                status: if stage.status == "queued" {
                    "pending".to_string()
                } else {
                    stage.status.clone()
                },
                detail: stage.detail.clone(),
                updated_at: stage.updated_at.clone(),
            })
            .collect());
    }

    let Some(request) = latest_request else {
        return Ok(Vec::new());
    };
    let origin = ModulePublicationArtifactOrigin::parse(&request.snapshot.artifact_origin)
        .ok_or_else(|| {
            ModuleGovernanceError::InvalidLifecycleArtifactOrigin(
                request.snapshot.artifact_origin.clone(),
            )
        })?;
    let mut gates = Vec::new();
    for required_stage in publication_follow_up_stages(origin) {
        if let Some(event) = recent_events.iter().find(|event| {
            matches!(
                event.snapshot.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event
                .details
                .get("stage_key")
                .and_then(serde_json::Value::as_str)
                == Some(required_stage.key)
        }) {
            let status = event
                .details
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(match event.snapshot.event_type.as_str() {
                    "follow_up_gate_passed" => "passed",
                    "follow_up_gate_failed" => "failed",
                    _ => "pending",
                });
            gates.push(ModuleGovernanceGateSnapshot {
                key: required_stage.key.to_string(),
                status: status.to_string(),
                detail: event
                    .details
                    .get("detail")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_else(|| governance_gate_detail(required_stage.key))
                    .to_string(),
                updated_at: event.snapshot.created_at.clone(),
            });
        } else if matches!(request.snapshot.status.as_str(), "approved" | "published") {
            gates.push(ModuleGovernanceGateSnapshot {
                key: required_stage.key.to_string(),
                status: "pending".to_string(),
                detail: governance_gate_detail(required_stage.key).to_string(),
                updated_at: request
                    .validated_at
                    .as_ref()
                    .or(request.approved_at.as_ref())
                    .cloned()
                    .unwrap_or_default(),
            });
        }
    }
    Ok(gates)
}

pub(crate) fn derive_governance_request_actions(
    request: &ModuleGovernanceRequestSnapshot,
    authorization: &ModuleGovernanceRequestAuthorizationSnapshot,
    approval_override_required: bool,
) -> Vec<ModuleGovernanceAction> {
    let can_manage = authorization.can_manage;
    let can_review = authorization.can_review;
    let mut actions = Vec::new();

    if can_manage && matches!(request.status.as_str(), "draft" | "changes_requested") {
        actions.push(governance_action(
            "upload_artifact",
            false,
            false,
            &[],
            false,
        ));
    }
    if can_manage && matches!(request.status.as_str(), "artifact_uploaded" | "submitted") {
        actions.push(governance_action("validate", false, false, &[], false));
    }
    if can_review && request.status == "approved" {
        actions.push(governance_action(
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
        actions.push(governance_action(
            "request_changes",
            true,
            true,
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
            false,
        ));
    }
    if can_review
        && matches!(
            request.status.as_str(),
            "submitted" | "approved" | "changes_requested"
        )
    {
        actions.push(governance_action(
            "hold",
            true,
            true,
            REGISTRY_HOLD_REASON_CODES,
            false,
        ));
    }
    if can_review && request.status == "on_hold" {
        actions.push(governance_action(
            "resume",
            true,
            true,
            REGISTRY_RESUME_REASON_CODES,
            false,
        ));
    }
    if can_review
        && !matches!(
            request.status.as_str(),
            "rejected" | "published" | "on_hold"
        )
    {
        actions.push(governance_action(
            "reject",
            true,
            true,
            REGISTRY_REJECT_REASON_CODES,
            true,
        ));
    }
    actions
}

pub(crate) fn governance_actor_can_manage_request(
    request: &ModuleGovernanceRequestSnapshot,
    owner_binding: Option<&ModuleGovernanceOwnerSnapshot>,
    actor: Option<&ModuleGovernanceActorContext>,
) -> bool {
    actor.is_some_and(|actor| {
        governance_actor_can_manage_request_principals(
            &request.requested_by_principal,
            request.publisher_principal.as_ref(),
            owner_binding.map(|owner| &owner.owner_principal),
            &actor.principal,
            actor.can_manage_modules,
        )
    })
}

pub(crate) fn governance_actor_can_manage_request_principals(
    requested_by_principal: &serde_json::Value,
    publisher_principal: Option<&serde_json::Value>,
    owner_principal: Option<&serde_json::Value>,
    actor_principal: &serde_json::Value,
    actor_can_manage_modules: bool,
) -> bool {
    actor_can_manage_modules
        || principal_matches_governance_actor(owner_principal, actor_principal)
        || (owner_principal.is_none()
            && (principal_matches_governance_actor(Some(requested_by_principal), actor_principal)
                || principal_matches_governance_actor(publisher_principal, actor_principal)))
}

pub(crate) fn governance_actor_can_create_publish_request(
    owner_principal: Option<&serde_json::Value>,
    actor_principal: &serde_json::Value,
    actor_can_manage_modules: bool,
) -> bool {
    actor_can_manage_modules
        || principal_matches_governance_actor(owner_principal, actor_principal)
        || (owner_principal.is_none() && governance_principal_user_id(actor_principal).is_some())
}

pub(crate) fn governance_actor_can_manage_release(
    publisher_principal: &serde_json::Value,
    owner_principal: Option<&serde_json::Value>,
    actor_principal: &serde_json::Value,
    actor_can_manage_modules: bool,
) -> bool {
    actor_can_manage_modules
        || principal_matches_governance_actor(Some(publisher_principal), actor_principal)
        || principal_matches_governance_actor(owner_principal, actor_principal)
}

pub(crate) fn governance_actor_can_transfer_owner(
    owner_principal: &serde_json::Value,
    actor_principal: &serde_json::Value,
    actor_can_manage_modules: bool,
) -> bool {
    actor_can_manage_modules
        || principal_matches_governance_actor(Some(owner_principal), actor_principal)
}

pub(crate) fn governance_actor_can_review_request(
    owner_binding: Option<&ModuleGovernanceOwnerSnapshot>,
    actor: Option<&ModuleGovernanceActorContext>,
) -> bool {
    actor.is_some_and(|actor| {
        actor.can_manage_modules
            || principal_matches_governance_actor(
                owner_binding.map(|owner| &owner.owner_principal),
                &actor.principal,
            )
    })
}

pub(crate) fn governance_pending_stage_labels(
    validation_stages: &[ModuleGovernanceValidationStageSnapshot],
) -> Vec<String> {
    validation_stages
        .iter()
        .filter(|stage| !stage.status.eq_ignore_ascii_case("passed"))
        .map(|stage| format!("{} ({})", stage.key, stage.status.to_ascii_lowercase()))
        .collect()
}

pub(crate) fn governance_rejected_request_can_retry(
    latest_event_type: Option<&str>,
    rejection_reason: Option<&str>,
) -> bool {
    matches!(latest_event_type, Some("validation_failed"))
        || rejection_reason
            .is_some_and(|reason| !reason.trim().starts_with("Governance rejection reason:"))
}

pub(crate) fn publish_request_next_action(
    request: &ModuleGovernanceRequestSnapshot,
    approval_override_required: bool,
) -> Result<Option<ModuleGovernancePublishRequestNextAction>, ModuleGovernanceError> {
    if request.status == "approved" {
        if !approval_override_required {
            return Ok(Some(
                ModuleGovernancePublishRequestNextAction::FinalizePublication,
            ));
        }
        let artifact_origin = ModulePublicationArtifactOrigin::parse(&request.artifact_origin)
            .ok_or_else(|| {
                ModuleGovernanceError::InvalidLifecycleArtifactOrigin(
                    request.artifact_origin.clone(),
                )
            })?;
        let stage_action = match artifact_origin {
            ModulePublicationArtifactOrigin::ExternalPrebuilt => {
                ModuleGovernancePublishRequestNextAction::StageExternalPrebuilt
            }
            ModulePublicationArtifactOrigin::PlatformBuilt => {
                ModuleGovernancePublishRequestNextAction::StagePlatformBuild
            }
            ModulePublicationArtifactOrigin::AlloyAuthored => {
                ModuleGovernancePublishRequestNextAction::StageAlloyRelease
            }
        };
        return Ok(Some(stage_action));
    }

    let next_action = match request.status.as_str() {
        "draft" => ModuleGovernancePublishRequestNextAction::UploadArtifact,
        "changes_requested" => ModuleGovernancePublishRequestNextAction::UploadArtifactRevision,
        "artifact_uploaded" | "submitted" => {
            ModuleGovernancePublishRequestNextAction::TriggerValidation
        }
        "validating" => ModuleGovernancePublishRequestNextAction::PollStatus,
        "on_hold" => ModuleGovernancePublishRequestNextAction::Resume,
        "rejected" => ModuleGovernancePublishRequestNextAction::RetryRejected,
        "published" => return Ok(None),
        status => {
            return Err(ModuleGovernanceError::InvalidPublishRequestStatus(
                status.to_string(),
            ));
        }
    };

    Ok(Some(next_action))
}

pub(crate) fn principal_matches_governance_actor(
    persisted_principal: Option<&serde_json::Value>,
    actor_principal: &serde_json::Value,
) -> bool {
    let Some(persisted_principal) = persisted_principal else {
        return false;
    };
    let persisted_user_id = governance_principal_user_id(persisted_principal);
    let actor_user_id = governance_principal_user_id(actor_principal);
    if let (Some(persisted_user_id), Some(actor_user_id)) = (persisted_user_id, actor_user_id) {
        return persisted_user_id == actor_user_id;
    }

    governance_principal_subject(persisted_principal)
        .zip(governance_principal_subject(actor_principal))
        .is_some_and(|(persisted_subject, actor_subject)| persisted_subject == actor_subject)
        || governance_principal_label(persisted_principal)
            .zip(governance_principal_label(actor_principal))
            .is_some_and(|(persisted_label, actor_label)| persisted_label == actor_label)
}

pub(crate) fn governance_principal_user_id(principal: &serde_json::Value) -> Option<&str> {
    (principal.get("kind").and_then(serde_json::Value::as_str) == Some("user"))
        .then(|| {
            principal
                .get("user_id")
                .or_else(|| principal.get("id"))
                .and_then(serde_json::Value::as_str)
        })
        .flatten()
}

pub(crate) fn governance_principal_subject(principal: &serde_json::Value) -> Option<&str> {
    principal.as_str().or_else(|| {
        principal
            .get("subject")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
    })
}

pub(crate) fn governance_principal_label(principal: &serde_json::Value) -> Option<&str> {
    principal.as_str().or_else(|| {
        principal
            .get("legacy_label")
            .or_else(|| principal.get("display_label"))
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
    })
}

pub(crate) fn derive_governance_actions(
    latest_request: Option<&ModuleGovernanceRequestSnapshot>,
    latest_release: Option<&ModuleGovernanceReleaseSnapshot>,
    owner_binding: Option<&ModuleGovernanceOwnerSnapshot>,
    validation_stages: &[ModuleGovernanceValidationStageSnapshot],
) -> Vec<ModuleGovernanceAction> {
    let mut actions = Vec::new();
    if let Some(request) = latest_request {
        let approval_override_required = request.status == "approved"
            && validation_stages
                .iter()
                .any(|stage| !stage.status.eq_ignore_ascii_case("passed"));
        match request.status.as_str() {
            "artifact_uploaded" | "submitted" => {
                actions.push(governance_action("validate", false, false, &[], false));
            }
            "approved" => {
                actions.push(governance_action(
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
                actions.push(governance_action(
                    "request_changes",
                    true,
                    true,
                    REGISTRY_REQUEST_CHANGES_REASON_CODES,
                    false,
                ));
            }
            _ => {}
        }
        if matches!(
            request.status.as_str(),
            "submitted" | "approved" | "changes_requested"
        ) {
            actions.push(governance_action(
                "hold",
                true,
                true,
                REGISTRY_HOLD_REASON_CODES,
                false,
            ));
        }
        if request.status == "on_hold" {
            actions.push(governance_action(
                "resume",
                true,
                true,
                REGISTRY_RESUME_REASON_CODES,
                false,
            ));
        }
        if !matches!(
            request.status.as_str(),
            "rejected" | "published" | "on_hold"
        ) {
            actions.push(governance_action(
                "reject",
                true,
                true,
                REGISTRY_REJECT_REASON_CODES,
                true,
            ));
        }
        if request
            .publisher_principal
            .as_ref()
            .is_some_and(|publisher| {
                owner_binding.is_none_or(|owner| owner.owner_principal != *publisher)
            })
            || owner_binding.is_some()
        {
            actions.push(governance_action(
                "owner_transfer",
                true,
                true,
                REGISTRY_OWNER_TRANSFER_REASON_CODES,
                true,
            ));
        }
    } else if owner_binding.is_some() {
        actions.push(governance_action(
            "owner_transfer",
            true,
            true,
            REGISTRY_OWNER_TRANSFER_REASON_CODES,
            true,
        ));
    }
    if latest_release.is_some_and(|release| release.status == "active") {
        actions.push(governance_action(
            "yank",
            true,
            true,
            REGISTRY_YANK_REASON_CODES,
            true,
        ));
    }
    let mut seen = std::collections::HashSet::new();
    actions
        .into_iter()
        .filter(|action| seen.insert(action.key.clone()))
        .collect()
}

pub(crate) fn governance_action(
    key: &str,
    reason_required: bool,
    reason_code_required: bool,
    reason_codes: &[&str],
    destructive: bool,
) -> ModuleGovernanceAction {
    ModuleGovernanceAction {
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

pub(crate) fn governance_event_payload(details: &serde_json::Value) -> ModuleGovernanceEventPayload {
    let string = |key| {
        details
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string)
    };
    ModuleGovernanceEventPayload {
        reason: string("reason"),
        reason_code: string("reason_code"),
        detail: string("detail"),
        version: string("version"),
        stage_key: string("stage_key"),
        attempt_number: details
            .get("attempt_number")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        owner_transition: details
            .get("owner_transition")
            .and_then(serde_json::Value::as_object)
            .map(|transition| ModuleGovernanceOwnerTransition {
                previous_owner_principal: transition.get("previous_owner").cloned(),
                new_owner_principal: transition.get("new_owner").cloned(),
                bound_by_principal: transition.get("bound_by").cloned(),
            }),
        warnings: details
            .get("warnings")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(ToString::to_string)
            .collect(),
        errors: details
            .get("errors")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(ToString::to_string)
            .collect(),
        mode: string("mode"),
        automated_checks: details
            .get("automated_checks")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let key = item.get("key")?.as_str()?.trim();
                let status = item.get("status")?.as_str()?.trim();
                if key.is_empty() || status.is_empty() {
                    return None;
                }
                let detail = item
                    .get("detail")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|detail| !detail.is_empty())
                    .map(ToString::to_string);
                Some(ModuleGovernanceAutomatedCheck {
                    key: key.to_string(),
                    status: status.to_string(),
                    detail,
                })
            })
            .collect(),
    }
}

pub(crate) fn normalize_governance_automated_checks(
    checks: &[ModuleGovernanceAutomatedCheck],
) -> Vec<ModuleGovernanceAutomatedCheck> {
    checks
        .iter()
        .map(|check| ModuleGovernanceAutomatedCheck {
            key: check.key.trim().to_string(),
            status: check.status.trim().to_string(),
            detail: check
                .detail
                .as_deref()
                .map(str::trim)
                .filter(|detail| !detail.is_empty())
                .map(ToString::to_string),
        })
        .collect()
}

pub(crate) fn governance_gate_detail(key: &str) -> &'static str {
    match key {
        "compile_smoke" => "Compile smoke awaits exact platform build-worker validation evidence.",
        "targeted_tests" => "Targeted tests await exact platform build-worker validation evidence.",
        "security_policy_review" => {
            "Security and policy review await exact origin-specific owner evidence."
        }
        _ => "External follow-up gate is still pending.",
    }
}

pub(crate) fn governance_stage_execution_mode(key: &str) -> &'static str {
    match key {
        "security_policy_review" => "manual_review",
        "compile_smoke" | "targeted_tests" => "remote",
        _ => "external",
    }
}

pub(crate) fn governance_stage_pass_reason_code(key: &str) -> Option<&'static str> {
    match key {
        "compile_smoke" | "targeted_tests" => Some("local_runner_passed"),
        "security_policy_review" => Some("manual_review_complete"),
        _ => None,
    }
}

pub(crate) fn governance_stage_failure_reason_code(key: &str) -> Option<&'static str> {
    match key {
        "compile_smoke" => Some("build_failure"),
        "targeted_tests" => Some("test_failure"),
        "security_policy_review" => Some("policy_preflight_failed"),
        _ => None,
    }
}

pub(crate) fn governance_stage_blocked_reason_code(key: &str) -> Option<&'static str> {
    match key {
        "security_policy_review" => Some("security_findings"),
        "compile_smoke" | "targeted_tests" => Some("other"),
        _ => None,
    }
}

