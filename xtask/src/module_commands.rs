use super::*;
use crate::manifest_validation::module_validate_command;

pub(crate) fn module_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("Missing module subcommand");
    }

    match args[0].as_str() {
        "validate" => module_validate_command(&args[1..]),
        "test" => module_test_command(&args[1..]),
        "stage-run" => module_stage_run_command(&args[1..]),
        "runner" => module_runner_command(&args[1..]),
        "publish" => module_publish_command(&args[1..]),
        "request-changes" => module_request_changes_command(&args[1..]),
        "hold" => module_hold_command(&args[1..]),
        "resume" => module_resume_command(&args[1..]),
        "stage" => module_stage_command(&args[1..]),
        "owner-transfer" => module_owner_transfer_command(&args[1..]),
        "yank" => module_yank_command(&args[1..]),
        other => {
            print_module_usage();
            anyhow::bail!("Unknown module subcommand: {other}");
        }
    }
}

pub(crate) fn print_module_usage() {
    println!("Usage:");
    println!("  cargo xtask module validate [slug]");
    println!("  cargo xtask module test <slug> [--dry-run]");
    println!(
        "  cargo xtask module stage-run <slug> <request-id> <stage> [--dry-run] [--registry-url <url>] [--auth-token <token>] [--detail <text>] [--confirm-manual-review]"
    );
    println!(
        "  cargo xtask module runner <runner-id> [--dry-run] [--once] [--registry-url <url>] [--runner-token <token>] [--poll-interval-ms <ms>] [--heartbeat-interval-ms <ms>] [--confirm-manual-review]"
    );
    println!(
        "  cargo xtask module publish <slug> [--dry-run] [--registry-url <url>] [--auth-token <token>] [--auto-approve] [--approve-reason <text>] [--approve-reason-code <code>] [--confirm-manual-review]"
    );
    println!(
        "  cargo xtask module request-changes <request-id> [--dry-run] [--registry-url <url>] --auth-token <token> --reason <text> --reason-code <code>"
    );
    println!(
        "  cargo xtask module hold <request-id> [--dry-run] [--registry-url <url>] --auth-token <token> --reason <text> --reason-code <code>"
    );
    println!(
        "  cargo xtask module resume <request-id> [--dry-run] [--registry-url <url>] --auth-token <token> --reason <text> --reason-code <code>"
    );
    println!(
        "  cargo xtask module stage <request-id> <stage> <status> [--dry-run] [--detail <text>] [--reason-code <code>] [--registry-url <url>] [--auth-token <token>] [--requeue]"
    );
    println!(
        "  cargo xtask module owner-transfer <slug> <new-owner-user-id> [--dry-run] [--reason <text>] [--reason-code <code>] [--registry-url <url>] [--auth-token <token>]"
    );
    println!(
        "  cargo xtask module yank <slug> <version> [--dry-run] [--reason <text>] [--reason-code <code>] [--registry-url <url>] [--auth-token <token>]"
    );
}

pub(crate) fn module_test_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("module test requires a module slug");
    }

    let slug = args[0].as_str();
    let dry_run = args.iter().skip(1).any(|arg| arg == "--dry-run");
    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let spec = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown module slug '{slug}'"))?;
    let preview = build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
    let plan = build_module_test_plan(&preview);

    if dry_run {
        let payload = serde_json::to_string_pretty(&plan)?;
        println!("{payload}");
        return Ok(());
    }

    println!(
        "Running local module smoke checks for {slug} v{}...",
        preview.version
    );
    for command in &plan.commands {
        println!("  > {}", command.argv.join(" "));
        run_command(command)?;
    }

    Ok(())
}

pub(crate) fn module_stage_run_command(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        print_module_usage();
        anyhow::bail!("module stage-run requires a module slug, request id, and stage key");
    }

    let slug = args[0].trim();
    let request_id = args[1].trim();
    let stage = args[2].trim().to_ascii_lowercase();
    if slug.is_empty() {
        anyhow::bail!("module stage-run requires a non-empty module slug");
    }
    if request_id.is_empty() {
        anyhow::bail!("module stage-run requires a non-empty request id");
    }

    let dry_run = args.iter().skip(3).any(|arg| arg == "--dry-run");
    let registry_url = registry_url_argument(&args[3..]);
    let auth_token = auth_token_argument(&args[3..]);
    let live_registry_url = if dry_run {
        None
    } else {
        Some(registry_url.clone().with_context(|| {
            "Live module stage-run requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?)
    };
    let live_auth_token = if dry_run {
        None
    } else {
        Some(auth_token.with_context(|| {
            format!(
                "Live module stage-run requires --auth-token <token> or {}",
                MODULE_AUTH_TOKEN_ENV
            )
        })?)
    };
    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let spec = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown module slug '{slug}'"))?;
    let preview = build_module_publish_preview(&manifest_path, slug, spec, &workspace_manifest)?;
    let plan = build_module_validation_stage_run_preview(
        &preview,
        request_id,
        &stage,
        detail_argument(&args[3..]),
    )?;
    let confirm_manual_review = manual_review_confirmation_argument(&args[3..]);

    if dry_run {
        println!("{}", serde_json::to_string_pretty(&plan)?);
        return Ok(());
    }

    if plan.requires_manual_confirmation && !confirm_manual_review {
        anyhow::bail!(
            "module stage-run '{}' requires --confirm-manual-review before persisting a passed security/policy review",
            plan.stage
        );
    }
    let live_registry_url = live_registry_url
        .as_deref()
        .with_context(|| "Live module stage-run requires resolved registry URL")?;
    let live_auth_token = live_auth_token
        .as_deref()
        .with_context(|| "Live module stage-run requires resolved auth token")?;
    run_validation_stage_plan_via_registry(live_registry_url, &plan, live_auth_token)?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}
