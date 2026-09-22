use super::*;

pub(crate) fn claim_remote_validation_stage_via_registry(
    registry_url: &str,
    runner_id: &str,
    runner_token: &str,
    supported_stages: &[&str],
) -> Result<Option<RegistryRunnerClaimHttpPayload>> {
    let endpoint = format!(
        "{}/v2/catalog/runner/claim",
        registry_url.trim_end_matches('/')
    );
    let request = RegistryRunnerClaimHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        runner_id: runner_id.to_string(),
        supported_stages: supported_stages
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    };
    let response: RegistryRunnerClaimHttpResponse =
        post_registry_json_with_runner_token_parsed(&endpoint, &request, runner_token)?;
    if !response.accepted {
        anyhow::bail!("Registry remote runner claim was not accepted");
    }

    Ok(response.claim)
}

pub(crate) fn heartbeat_remote_validation_stage_via_registry(
    registry_url: &str,
    claim_id: &str,
    runner_id: &str,
    runner_token: &str,
) -> Result<()> {
    let endpoint = format!(
        "{}/v2/catalog/runner/{claim_id}/heartbeat",
        registry_url.trim_end_matches('/')
    );
    let request = RegistryRunnerHeartbeatHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        runner_id: runner_id.to_string(),
    };
    let response: RegistryRunnerMutationHttpResponse =
        post_registry_json_with_runner_token_parsed(&endpoint, &request, runner_token)?;
    if !response.accepted {
        anyhow::bail!(
            "Registry remote runner heartbeat for claim '{}' was not accepted",
            claim_id
        );
    }

    Ok(())
}

pub(crate) fn complete_remote_validation_stage_via_registry(
    registry_url: &str,
    claim_id: &str,
    runner_id: &str,
    runner_token: &str,
    detail: String,
    reason_code: String,
) -> Result<()> {
    let endpoint = format!(
        "{}/v2/catalog/runner/{claim_id}/complete",
        registry_url.trim_end_matches('/')
    );
    let request = RegistryRunnerCompletionHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        runner_id: runner_id.to_string(),
        detail: Some(detail),
        reason_code: Some(reason_code),
    };
    let response: RegistryRunnerMutationHttpResponse =
        post_registry_json_with_runner_token_parsed(&endpoint, &request, runner_token)?;
    if !response.accepted {
        anyhow::bail!(
            "Registry remote runner completion for claim '{}' was not accepted",
            claim_id
        );
    }

    Ok(())
}

pub(crate) fn fail_remote_validation_stage_via_registry(
    registry_url: &str,
    claim_id: &str,
    runner_id: &str,
    runner_token: &str,
    detail: String,
    reason_code: String,
) -> Result<()> {
    let endpoint = format!(
        "{}/v2/catalog/runner/{claim_id}/fail",
        registry_url.trim_end_matches('/')
    );
    let request = RegistryRunnerCompletionHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        runner_id: runner_id.to_string(),
        detail: Some(detail),
        reason_code: Some(reason_code),
    };
    let response: RegistryRunnerMutationHttpResponse =
        post_registry_json_with_runner_token_parsed(&endpoint, &request, runner_token)?;
    if !response.accepted {
        anyhow::bail!(
            "Registry remote runner failure report for claim '{}' was not accepted",
            claim_id
        );
    }

    Ok(())
}
