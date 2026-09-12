use super::*;

pub(crate) fn module_runner_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        print_module_usage();
        anyhow::bail!("module runner requires a non-empty runner id");
    }

    let runner_id = args[0].trim();
    if runner_id.is_empty() {
        anyhow::bail!("module runner requires a non-empty runner id");
    }

    let dry_run = args.iter().skip(1).any(|arg| arg == "--dry-run");
    let once = args.iter().skip(1).any(|arg| arg == "--once");
    let confirm_manual_review = manual_review_confirmation_argument(&args[1..]);
    let poll_interval_ms =
        positive_u64_argument(&args[1..], "--poll-interval-ms", "module runner")?
            .unwrap_or(DEFAULT_REMOTE_RUNNER_POLL_INTERVAL_MS);
    let heartbeat_interval_ms =
        positive_u64_argument(&args[1..], "--heartbeat-interval-ms", "module runner")?
            .unwrap_or(DEFAULT_REMOTE_RUNNER_HEARTBEAT_INTERVAL_MS);
    let supported_stages = supported_remote_runner_stages(confirm_manual_review);
    let preview = ModuleRunnerPreview {
        action: "remote_validation_runner".to_string(),
        runner_id: runner_id.to_string(),
        supported_stages: supported_stages
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        poll_interval_ms,
        heartbeat_interval_ms,
        once,
        confirm_manual_review,
    };

    if dry_run {
        println!("{}", serde_json::to_string_pretty(&preview)?);
        return Ok(());
    }

    let registry_url = registry_url_argument(&args[1..]).with_context(|| {
        "Live module runner requires --registry-url or RUSTOK_MODULE_REGISTRY_URL"
    })?;
    let runner_token = runner_token_argument(&args[1..]).with_context(|| {
        format!("Live module runner requires --runner-token or {REMOTE_RUNNER_TOKEN_ENV}")
    })?;

    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;

    if once {
        let processed = run_remote_validation_runner_once(
            &registry_url,
            runner_id,
            &runner_token,
            &manifest_path,
            &manifest,
            &workspace_manifest,
            confirm_manual_review,
            heartbeat_interval_ms,
        )?;
        if !processed {
            println!("No queued remote validation stages were available.");
        }
        return Ok(());
    }

    println!(
        "Starting remote validation runner '{runner_id}' against {}...",
        registry_url
    );
    loop {
        let processed = run_remote_validation_runner_once(
            &registry_url,
            runner_id,
            &runner_token,
            &manifest_path,
            &manifest,
            &workspace_manifest,
            confirm_manual_review,
            heartbeat_interval_ms,
        )?;
        if !processed {
            thread::sleep(Duration::from_millis(poll_interval_ms));
        }
    }
}
