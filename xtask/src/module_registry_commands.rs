use super::*;

pub(crate) fn module_request_changes_command(args: &[String]) -> Result<()> {
    module_publish_governance_command(
        args,
        "request-changes",
        "request_changes",
        REGISTRY_REQUEST_CHANGES_REASON_CODES,
    )
}

pub(crate) fn module_hold_command(args: &[String]) -> Result<()> {
    module_publish_governance_command(args, "hold", "hold", REGISTRY_HOLD_REASON_CODES)
}

pub(crate) fn module_resume_command(args: &[String]) -> Result<()> {
    module_publish_governance_command(args, "resume", "resume", REGISTRY_RESUME_REASON_CODES)
}

pub(crate) fn module_publish_governance_command(
    args: &[String],
    command_name: &str,
    action_key: &str,
    allowed_reason_codes: &[&str],
) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("module {command_name} requires a request id");
    }

    let request_id = args[0].trim();
    if request_id.is_empty() {
        anyhow::bail!("module {command_name} requires a non-empty request id");
    }

    let dry_run = args.iter().skip(1).any(|arg| arg == "--dry-run");
    let auth_token = auth_token_argument(&args[1..]);
    let reason = reason_argument(&args[1..]);
    let reason_code =
        governance_reason_code_argument(&args[1..], command_name, allowed_reason_codes)?;
        let preview = ModulePublishGovernanceDryRunPreview {
            action: action_key.to_string(),
            request_id: request_id.to_string(),
            actor: None,
            reason: reason.clone(),
            reason_code: reason_code.clone(),
        };
    let registry_url = registry_url_argument(&args[1..]);

    if dry_run {
        if let Some(registry_url) = registry_url {
            let payload = publish_request_governance_via_registry(
                &registry_url,
                request_id,
                command_name,
                action_key,
                auth_token.as_deref(),
                reason,
                reason_code,
                true,
            )?;
            println!("{payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
        return Ok(());
    }

    let registry_url = registry_url.with_context(|| {
        format!("Live module {command_name} requires --registry-url or RUSTOK_MODULE_REGISTRY_URL")
    })?;
    let auth_token = auth_token.with_context(|| {
        format!(
            "Live module {command_name} requires --auth-token <token> or {}",
            MODULE_AUTH_TOKEN_ENV
        )
    })?;
    let reason =
        reason.with_context(|| format!("Live module {command_name} requires --reason <text>"))?;
    let reason_code = reason_code.with_context(|| {
        format!(
            "Live module {command_name} requires --reason-code <{}>",
            allowed_reason_codes.join("|")
        )
    })?;

    let payload = publish_request_governance_via_registry(
        &registry_url,
        request_id,
        command_name,
        action_key,
        Some(&auth_token),
        Some(reason),
        Some(reason_code),
        false,
    )?;
    println!("{payload}");
    Ok(())
}

pub(crate) fn module_stage_command(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        print_module_usage();
        anyhow::bail!("module stage requires a request id, stage key, and status");
    }

    let request_id = args[0].trim();
    let stage = args[1].trim();
    let status = args[2].trim();
    if request_id.is_empty() {
        anyhow::bail!("module stage requires a non-empty request id");
    }
    if stage.is_empty() {
        anyhow::bail!("module stage requires a non-empty stage key");
    }

    let stage_status = match status {
        "queued" | "running" | "passed" | "failed" | "manual_review" => status,
        _ => anyhow::bail!(
            "module stage status '{}' is invalid; expected queued|running|passed|failed|manual_review",
            status
        ),
    };
    let requeue = args.iter().skip(3).any(|arg| arg == "--requeue");
    if requeue && stage_status != "queued" {
        anyhow::bail!("module stage --requeue requires status 'queued'");
    }

    let dry_run = args.iter().skip(3).any(|arg| arg == "--dry-run");
    let detail = detail_argument(&args[3..]);
    let reason_code = validation_stage_reason_code_argument(&args[3..])?;
    let auth_token = auth_token_argument(&args[3..]);
    let registry_url = registry_url_argument(&args[3..]);

    let preview = ModuleValidationStageDryRunPreview {
        action: if requeue {
            "stage_requeue".to_string()
        } else {
            "stage_update".to_string()
        },
        request_id: request_id.to_string(),
        stage: stage.to_string(),
        status: stage_status.to_string(),
        detail,
        reason_code,
        requeue,
    };

    if dry_run {
        if let Some(registry_url) = registry_url {
            let payload = validation_stage_via_registry_dry_run(&registry_url, &preview)?;
            println!("{payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
        return Ok(());
    }

    let registry_url = registry_url.with_context(|| {
        "Live module stage requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
    })?;
    let auth_token = auth_token.with_context(|| {
        format!(
            "Live module stage requires --auth-token <token> or {}",
            MODULE_AUTH_TOKEN_ENV
        )
    })?;
    if validation_stage_status_requires_reason_code(&preview.status)
        && preview.reason_code.is_none()
    {
        anyhow::bail!(
            "Live module stage status '{}' requires --reason-code <{}>",
            preview.status,
            REGISTRY_VALIDATION_STAGE_REASON_CODES.join("|")
        );
    }
    let payload = validation_stage_via_registry_live(&registry_url, &preview, &auth_token)?;
    println!("{payload}");
    Ok(())
}
