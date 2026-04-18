use super::*;

pub(crate) fn validation_stage_via_registry_dry_run(
    registry_url: &str,
    preview: &ModuleValidationStageDryRunPreview,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/publish/{}/stages",
        registry_url.trim_end_matches('/'),
        preview.request_id
    );
    let request = build_validation_stage_registry_request(preview);

    post_registry_json(&endpoint, &request)
}

pub(crate) fn validation_stage_via_registry_live(
    registry_url: &str,
    preview: &ModuleValidationStageDryRunPreview,
    auth_token: &str,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/publish/{}/stages",
        registry_url.trim_end_matches('/'),
        preview.request_id
    );
    let request = build_live_validation_stage_registry_request(preview);
    let response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&endpoint, &request, Some(auth_token))?;
    if !response.accepted {
        anyhow::bail!(
            "Registry validation stage update was not accepted: {}",
            join_registry_errors(&response.errors)
        );
    }

    pretty_json(&response)
}

pub(crate) fn publish_request_governance_via_registry(
    registry_url: &str,
    request_id: &str,
    command_name: &str,
    action_key: &str,
    auth_token: Option<&str>,
    reason: Option<String>,
    reason_code: Option<String>,
    dry_run: bool,
) -> Result<String> {
    if !dry_run {
        let status = fetch_registry_publish_status_with_actor(registry_url, request_id, auth_token)?;
        if !publish_status_action_available(&status, action_key) {
            anyhow::bail!(
                "Registry publish request '{}' does not advertise '{}' in governanceActions. Current status: '{}'. Next step: {}",
                status.request_id,
                action_key,
                status.status,
                status
                    .next_step
                    .clone()
                    .unwrap_or_else(|| "no next_step returned".to_string())
            );
        }
    }

    let endpoint = format!(
        "{}/v2/catalog/publish/{request_id}/{}",
        registry_url.trim_end_matches('/'),
        command_name
    );
    let request = RegistryPublishDecisionHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        reason,
        reason_code,
    };
    let response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&endpoint, &request, auth_token)?;
    if !response.accepted {
        anyhow::bail!(
            "Registry publish {} request was not accepted: {}",
            action_key,
            join_registry_errors(&response.errors)
        );
    }

    pretty_json(&response)
}
