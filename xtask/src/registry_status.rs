use super::*;

pub(crate) fn run_publish_follow_up_stages_if_needed(
    registry_url: &str,
    preview: &ModulePublishDryRunPreview,
    mut status: RegistryPublishStatusHttpResponse,
    auth_token: &str,
    confirm_manual_review: bool,
) -> Result<RegistryPublishStatusHttpResponse> {
    let executable_stages = ["compile_smoke", "targeted_tests"];
    for stage in executable_stages {
        if publish_status_stage_requires_action(&status, stage) {
            let plan = build_module_validation_stage_run_preview(
                preview,
                &status.request_id,
                stage,
                None,
            )?;
            run_validation_stage_plan_via_registry(registry_url, &plan, auth_token)?;
            status = fetch_registry_publish_status_with_actor(
                registry_url,
                &status.request_id,
                Some(auth_token),
            )?;
            ensure_publish_status_not_rejected(&status)?;
        }
    }

    if publish_status_stage_requires_action(&status, "security_policy_review")
        && confirm_manual_review
    {
        let plan = build_module_validation_stage_run_preview(
            preview,
            &status.request_id,
            "security_policy_review",
            None,
        )?;
        run_validation_stage_plan_via_registry(registry_url, &plan, auth_token)?;
        status = fetch_registry_publish_status_with_actor(
            registry_url,
            &status.request_id,
            Some(auth_token),
        )?;
        ensure_publish_status_not_rejected(&status)?;
    }

    Ok(status)
}

pub(crate) fn fetch_registry_publish_status_with_actor(
    registry_url: &str,
    request_id: &str,
    auth_token: Option<&str>,
) -> Result<RegistryPublishStatusHttpResponse> {
    let endpoint = format!(
        "{}/v2/catalog/publish/{request_id}",
        registry_url.trim_end_matches('/')
    );
    get_registry_json_parsed(&endpoint, auth_token)
}

pub(crate) fn publish_status_stage_requires_action(
    status: &RegistryPublishStatusHttpResponse,
    stage_key: &str,
) -> bool {
    status
        .validation_stages
        .iter()
        .find(|stage| stage.key.eq_ignore_ascii_case(stage_key))
        .is_some_and(|stage| !stage.status.eq_ignore_ascii_case("passed"))
}

pub(crate) fn publish_status_action_available(
    status: &RegistryPublishStatusHttpResponse,
    action_key: &str,
) -> bool {
    status
        .governance_actions
        .iter()
        .any(|action| action.key.eq_ignore_ascii_case(action_key))
}

pub(crate) fn poll_registry_publish_status_until(
    endpoint: &str,
    auth_token: Option<&str>,
    desired_statuses: &[&str],
) -> Result<RegistryPublishStatusHttpResponse> {
    let mut last_status = None;

    for attempt in 0..10 {
        let status: RegistryPublishStatusHttpResponse =
            get_registry_json_parsed(endpoint, auth_token)?;
        let terminal = desired_statuses
            .iter()
            .any(|candidate| status.status == *candidate);
        last_status = Some(status);
        if terminal {
            break;
        }
        if attempt < 9 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    last_status.with_context(|| format!("Registry status endpoint {endpoint} returned no payload"))
}

pub(crate) fn ensure_publish_step_accepted(
    step: &str,
    accepted: bool,
    status: Option<&str>,
    errors: &[String],
) -> Result<()> {
    if accepted && status != Some("rejected") {
        return Ok(());
    }

    let status_label = status.unwrap_or("unknown");
    anyhow::bail!(
        "Registry publish {step} finished with status '{status_label}': {}",
        join_registry_errors(errors)
    );
}

pub(crate) fn ensure_publish_status_not_rejected(
    status: &RegistryPublishStatusHttpResponse,
) -> Result<()> {
    if status.accepted && status.status != "rejected" {
        return Ok(());
    }

    anyhow::bail!(
        "Registry publish request '{}' was rejected: {}",
        status.request_id,
        join_registry_errors(&status.errors)
    );
}

pub(crate) fn join_registry_errors(errors: &[String]) -> String {
    if errors.is_empty() {
        "no error details returned".to_string()
    } else {
        errors.join("; ")
    }
}
