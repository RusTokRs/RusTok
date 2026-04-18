use super::*;

pub(crate) fn yank_via_registry_dry_run(
    registry_url: &str,
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> Result<String> {
    let endpoint = format!("{}/v2/catalog/yank", registry_url.trim_end_matches('/'));
    let request = build_yank_registry_request(preview, reason, reason_code);

    post_registry_json(&endpoint, &request)
}

pub(crate) fn yank_via_registry_live(
    registry_url: &str,
    preview: &ModuleYankDryRunPreview,
    auth_token: &str,
    reason: String,
    reason_code: String,
) -> Result<String> {
    let endpoint = format!("{}/v2/catalog/yank", registry_url.trim_end_matches('/'));
    let request = build_live_yank_registry_request(preview, Some(reason), Some(reason_code));
    let response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&endpoint, &request, Some(auth_token))?;
    if !response.accepted {
        anyhow::bail!(
            "Registry yank request was not accepted: {}",
            join_registry_errors(&response.errors)
        );
    }

    pretty_json(&response)
}

pub(crate) fn owner_transfer_via_registry_dry_run(
    registry_url: &str,
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/owner-transfer",
        registry_url.trim_end_matches('/')
    );
    let request = build_owner_transfer_registry_request(preview, reason, reason_code);

    post_registry_json(&endpoint, &request)
}

pub(crate) fn owner_transfer_via_registry_live(
    registry_url: &str,
    preview: &ModuleOwnerTransferDryRunPreview,
    auth_token: &str,
    reason: String,
    reason_code: String,
) -> Result<String> {
    let endpoint = format!(
        "{}/v2/catalog/owner-transfer",
        registry_url.trim_end_matches('/')
    );
    let request =
        build_live_owner_transfer_registry_request(preview, Some(reason), Some(reason_code));
    let response: RegistryMutationHttpResponse =
        post_registry_json_parsed(&endpoint, &request, Some(auth_token))?;
    if !response.accepted {
        anyhow::bail!(
            "Registry owner transfer request was not accepted: {}",
            join_registry_errors(&response.errors)
        );
    }

    pretty_json(&response)
}
