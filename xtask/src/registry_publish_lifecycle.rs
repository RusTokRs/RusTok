use super::*;

pub(crate) fn publish_via_registry_dry_run(
    registry_url: &str,
    preview: &ModulePublishDryRunPreview,
) -> Result<String> {
    let endpoint = format!("{}/v2/catalog/publish", registry_url.trim_end_matches('/'));
    let request = build_publish_registry_request(preview);

    post_registry_json(&endpoint, &request)
}

pub(crate) fn publish_via_registry_live(
    registry_url: &str,
    preview: &ModulePublishDryRunPreview,
    auth_token: &str,
    auto_approve: bool,
    approve_reason: Option<String>,
    approve_reason_code: Option<String>,
    confirm_manual_review: bool,
) -> Result<String> {
    let create_endpoint = format!("{}/v2/catalog/publish", registry_url.trim_end_matches('/'));
    let create_request = build_live_publish_registry_request(preview);
    let create_response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&create_endpoint, &create_request, Some(auth_token))?;
    if !create_response.accepted {
        anyhow::bail!(
            "Registry publish request was not accepted: {}",
            join_registry_errors(&create_response.errors)
        );
    }

    let request_id = create_response
        .request_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .with_context(|| {
            format!(
                "Registry publish did not return request_id; status={:?}, next_step={:?}",
                create_response.status, create_response.next_step
            )
        })?;

    let artifact_bytes = build_publish_artifact_bytes(preview)?;
    let upload_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/artifact",
        registry_url.trim_end_matches('/')
    );
    let upload_response: RegistryMutationHttpResponse = put_registry_bytes_parsed(
        &upload_endpoint,
        &artifact_bytes,
        "application/json",
        Some(auth_token),
    )?;
    ensure_publish_step_accepted(
        "artifact upload",
        upload_response.accepted,
        upload_response.status.as_deref(),
        &upload_response.errors,
    )?;

    let validate_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/validate",
        registry_url.trim_end_matches('/')
    );
    let validate_request = RegistryPublishValidationHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run: false,
    };
    let validate_response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&validate_endpoint, &validate_request, Some(auth_token))?;
    ensure_publish_step_accepted(
        "validation",
        validate_response.accepted,
        validate_response.status.as_deref(),
        &validate_response.errors,
    )?;

    let readiness_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}",
        registry_url.trim_end_matches('/')
    );
    let readiness = poll_registry_publish_status_until(
        &readiness_endpoint,
        Some(auth_token),
        &["approved", "published", "rejected"],
    )?;
    ensure_publish_status_not_rejected(&readiness)?;
    if readiness.status == "published" {
        return pretty_json(&readiness);
    }
    if readiness.status != "approved" {
        anyhow::bail!(
            "Registry publish request '{}' did not reach approved status after validation; current status is '{}'",
            readiness.request_id,
            readiness.status
        );
    }
    if !publish_status_action_available(&readiness, "approve") {
        anyhow::bail!(
            "Registry publish request '{}' reached status '{}', but the status contract does not advertise approve in governanceActions. Next step: {}",
            readiness.request_id,
            readiness.status,
            readiness
                .next_step
                .clone()
                .unwrap_or_else(|| "no next_step returned".to_string())
        );
    }
    if !auto_approve {
        return pretty_json(&readiness);
    }
    let readiness = run_publish_follow_up_stages_if_needed(
        registry_url,
        preview,
        readiness,
        auth_token,
        confirm_manual_review,
    )?;
    if readiness.approval_override_required
        && (approve_reason.as_deref().is_none_or(str::is_empty)
            || approve_reason_code.as_deref().is_none_or(str::is_empty))
    {
        let pending_stages = readiness
            .validation_stages
            .iter()
            .filter(|stage| !stage.status.eq_ignore_ascii_case("passed"))
            .map(|stage| format!("{} ({})", stage.key, stage.status))
            .collect::<Vec<_>>();
        let security_policy_hint = if publish_status_stage_requires_action(
            &readiness,
            "security_policy_review",
        ) {
            "; rerun with --confirm-manual-review to complete the local security/policy review path, or provide explicit approve override fields"
        } else {
            ""
        };
        anyhow::bail!(
            "Registry publish request '{}' still has non-passed follow-up stages [{}]; rerun with --approve-reason and --approve-reason-code <{}> or mark the remaining stages as passed first{}",
            readiness.request_id,
            pending_stages.join(", "),
            if readiness.approval_override_reason_codes.is_empty() {
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join("|")
            } else {
                readiness.approval_override_reason_codes.join("|")
            },
            security_policy_hint
        );
    }

    let approve_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/approve",
        registry_url.trim_end_matches('/')
    );
    let approve_request = RegistryPublishDecisionHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run: false,
        reason: approve_reason,
        reason_code: approve_reason_code,
    };
    let approve_response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&approve_endpoint, &approve_request, Some(auth_token))?;
    ensure_publish_step_accepted(
        "approval",
        approve_response.accepted,
        approve_response.status.as_deref(),
        &approve_response.errors,
    )?;

    let status_endpoint = format!(
        "{}/v2/catalog/publish/{request_id}",
        registry_url.trim_end_matches('/')
    );
    let status = poll_registry_publish_status_until(
        &status_endpoint,
        Some(auth_token),
        &["published", "rejected"],
    )?;
    ensure_publish_status_not_rejected(&status)?;
    pretty_json(&status)
}
