use super::*;

pub(crate) fn run_command(command: &ModuleCommandPreview) -> Result<()> {
    let program = command
        .argv
        .first()
        .with_context(|| format!("Command '{}' has empty argv", command.label))?;
    let status = Command::new(program)
        .args(&command.argv[1..])
        .current_dir(workspace_root())
        .status()
        .with_context(|| format!("Failed to launch '{}'", command.argv.join(" ")))?;
    if !status.success() {
        anyhow::bail!(
            "Command '{}' failed with status {}",
            command.argv.join(" "),
            status
        );
    }
    Ok(())
}

pub(crate) fn run_validation_stage_plan_via_registry(
    registry_url: &str,
    plan: &ModuleValidationStageRunPreview,
    auth_token: &str,
) -> Result<()> {
    let running_preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: plan.request_id.clone(),
        stage: plan.stage.clone(),
        status: "running".to_string(),
        detail: Some(plan.running_detail.clone()),
        reason_code: None,
        requeue: false,
    };
    validation_stage_via_registry_live(registry_url, &running_preview, auth_token)?;

    for command in &plan.commands {
        println!("  > {}", command.argv.join(" "));
        if let Err(error) = run_command(command) {
            let failed_preview = ModuleValidationStageDryRunPreview {
                action: "validation_stage".to_string(),
                request_id: plan.request_id.clone(),
                stage: plan.stage.clone(),
                status: "failed".to_string(),
                detail: Some(format!("{}: {error}", plan.failure_detail_prefix)),
                reason_code: Some(validation_stage_failure_reason_code(&plan.stage).to_string()),
                requeue: false,
            };
            let report_error =
                validation_stage_via_registry_live(registry_url, &failed_preview, auth_token)
                    .err()
                    .map(|report_error| {
                        format!("; failed to persist failed stage status: {report_error}")
                    })
                    .unwrap_or_default();
            anyhow::bail!("{error}{report_error}");
        }
    }

    let passed_preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: plan.request_id.clone(),
        stage: plan.stage.clone(),
        status: "passed".to_string(),
        detail: Some(plan.success_detail.clone()),
        reason_code: Some(validation_stage_success_reason_code(&plan.stage).to_string()),
        requeue: false,
    };
    validation_stage_via_registry_live(registry_url, &passed_preview, auth_token)?;
    Ok(())
}
