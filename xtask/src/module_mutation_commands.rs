use super::*;

pub(crate) fn module_yank_command(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        print_module_usage();
        anyhow::bail!("module yank requires a module slug and version");
    }

    let slug = args[0].as_str();
    let version = args[1].trim();
    let dry_run = args.iter().skip(2).any(|arg| arg == "--dry-run");
    Version::parse(version)
        .with_context(|| format!("module yank version '{version}' is not valid semver"))?;
    let registry_url = registry_url_argument(&args[2..]);
    let auth_token = auth_token_argument(&args[2..]);
    let reason = reason_argument(&args[2..]);
    let reason_code = reason_code_argument(&args[2..])?;
    let live_registry_url = if dry_run {
        None
    } else {
        Some(registry_url.clone().with_context(|| {
            "Live module yank requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?)
    };
    let live_auth_token = if dry_run {
        None
    } else {
        Some(auth_token.with_context(|| {
            format!(
                "Live module yank requires --auth-token <token> or {}",
                MODULE_AUTH_TOKEN_ENV
            )
        })?)
    };
    let live_reason = if dry_run {
        None
    } else {
        Some(
            reason
                .clone()
                .with_context(|| "Live module yank requires --reason <text>")?,
        )
    };
    let live_reason_code = if dry_run {
        None
    } else {
        Some(reason_code.clone().with_context(|| {
            format!(
                "Live module yank requires --reason-code <{}>",
                REGISTRY_YANK_REASON_CODES.join("|")
            )
        })?)
    };
    let payload = build_module_yank_preview_for_slug(slug, version)?;
    if dry_run {
        if let Some(registry_url) = registry_url {
            let remote_payload =
                yank_via_registry_dry_run(&registry_url, &payload, reason, reason_code)?;
            println!("{remote_payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
    } else {
        let live_registry_url = live_registry_url
            .as_deref()
            .with_context(|| "Live module yank requires resolved registry URL")?;
        let live_auth_token = live_auth_token
            .as_deref()
            .with_context(|| "Live module yank requires resolved auth token")?;
        let live_reason =
            live_reason.with_context(|| "Live module yank requires resolved reason")?;
        let live_reason_code =
            live_reason_code.with_context(|| "Live module yank requires resolved reason code")?;
        let remote_payload = yank_via_registry_live(
            live_registry_url,
            &payload,
            live_auth_token,
            live_reason,
            live_reason_code,
        )?;
        println!("{remote_payload}");
    }

    Ok(())
}

pub(crate) fn module_owner_transfer_command(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        print_module_usage();
        anyhow::bail!("module owner-transfer requires a module slug and new owner user id");
    }

    let slug = args[0].trim();
    let new_owner_user_id = args[1].trim();
    if slug.is_empty() {
        anyhow::bail!("module owner-transfer requires a non-empty module slug");
    }
    if new_owner_user_id.is_empty() {
        anyhow::bail!("module owner-transfer requires a non-empty new owner user id");
    }

    let dry_run = args.iter().skip(2).any(|arg| arg == "--dry-run");
    let registry_url = registry_url_argument(&args[2..]);
    let auth_token = auth_token_argument(&args[2..]);
    let reason = reason_argument(&args[2..]);
    let reason_code = owner_transfer_reason_code_argument(&args[2..])?;
    let live_registry_url = if dry_run {
        None
    } else {
        Some(registry_url.clone().with_context(|| {
            "Live module owner-transfer requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
        })?)
    };
    let live_auth_token = if dry_run {
        None
    } else {
        Some(auth_token.with_context(|| {
            format!(
                "Live module owner-transfer requires --auth-token <token> or {}",
                MODULE_AUTH_TOKEN_ENV
            )
        })?)
    };
    let live_reason = if dry_run {
        None
    } else {
        Some(
            reason
                .clone()
                .with_context(|| "Live module owner-transfer requires --reason <text>")?,
        )
    };
    let live_reason_code = if dry_run {
        None
    } else {
        Some(reason_code.clone().with_context(|| {
            format!(
                "Live module owner-transfer requires --reason-code <{}>",
                REGISTRY_OWNER_TRANSFER_REASON_CODES.join("|")
            )
        })?)
    };
    let payload = build_module_owner_transfer_preview_for_slug(
        slug,
        new_owner_user_id,
        reason.clone(),
        reason_code.clone(),
    )?;
    if dry_run {
        if let Some(registry_url) = registry_url {
            let remote_payload = owner_transfer_via_registry_dry_run(
                &registry_url,
                &payload,
                reason.clone(),
                reason_code.clone(),
            )?;
            println!("{remote_payload}");
        } else {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        return Ok(());
    }

    let live_registry_url = live_registry_url
        .as_deref()
        .with_context(|| "Live module owner-transfer requires resolved registry URL")?;
    let live_auth_token = live_auth_token
        .as_deref()
        .with_context(|| "Live module owner-transfer requires resolved auth token")?;
    let live_reason =
        live_reason.with_context(|| "Live module owner-transfer requires resolved reason")?;
    let live_reason_code = live_reason_code
        .with_context(|| "Live module owner-transfer requires resolved reason code")?;
    let remote_payload = owner_transfer_via_registry_live(
        live_registry_url,
        &payload,
        live_auth_token,
        live_reason,
        live_reason_code,
    )?;
    println!("{remote_payload}");
    Ok(())
}
