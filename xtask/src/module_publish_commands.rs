use super::*;

pub(crate) fn module_publish_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("module publish requires a module slug");
    }

    let slug = args[0].as_str();
    let dry_run = args.iter().skip(1).any(|arg| arg == "--dry-run");
    let registry_url = registry_url_argument(&args[1..]);
    let auth_token = auth_token_argument(&args[1..]);
    let auto_approve = auto_approve_argument(&args[1..]);
    let approve_reason = approve_reason_argument(&args[1..]);
    let approve_reason_code = approve_reason_code_argument(&args[1..])?;
    let confirm_manual_review = manual_review_confirmation_argument(&args[1..]);
    let live_registry_url = if dry_run {
        None
    } else {
        Some(registry_url.clone().with_context(|| {
            "Live module publish requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?)
    };
    let live_auth_token = if dry_run {
        None
    } else {
        Some(auth_token.with_context(|| {
            format!(
                "Live module publish requires --auth-token <token> or {}",
                MODULE_AUTH_TOKEN_ENV
            )
        })?)
    };

    let preview = load_module_publish_preview_for_slug(slug)?;
    if dry_run {
        if let Some(registry_url) = registry_url {
            let payload = publish_via_registry_dry_run(&registry_url, &preview)?;
            println!("{payload}");
        } else {
            let payload = serde_json::to_string_pretty(&preview)?;
            println!("{payload}");
        }
    } else {
        let live_registry_url = live_registry_url
            .as_deref()
            .with_context(|| "Live module publish requires resolved registry URL")?;
        let live_auth_token = live_auth_token
            .as_deref()
            .with_context(|| "Live module publish requires resolved auth token")?;
        let payload = publish_via_registry_live(
            live_registry_url,
            &preview,
            live_auth_token,
            auto_approve,
            approve_reason,
            approve_reason_code,
            confirm_manual_review,
        )?;
        println!("{payload}");
    }

    Ok(())
}
