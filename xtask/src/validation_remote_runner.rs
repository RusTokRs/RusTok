use super::*;
use sha2::{Digest, Sha256};

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_remote_validation_runner_once(
    registry_url: &str,
    runner_id: &str,
    runner_token: &str,
    manifest_path: &Path,
    manifest: &Manifest,
    workspace_manifest: &TomlValue,
    confirm_manual_review: bool,
    heartbeat_interval_ms: u64,
) -> Result<bool> {
    let supported_stages = supported_remote_runner_stages(confirm_manual_review);
    let Some(claim) = claim_remote_validation_stage_via_registry(
        registry_url,
        runner_id,
        runner_token,
        &supported_stages,
    )?
    else {
        return Ok(false);
    };

    println!(
        "Claimed remote validation stage '{}' for {}@{} ({})",
        claim.stage_key, claim.slug, claim.version, claim.request_id
    );
    if !claim.runnable {
        anyhow::bail!(
            "Remote runner claim '{}' for stage '{}' is marked non-runnable",
            claim.claim_id,
            claim.stage_key
        );
    }

    let stop_heartbeats = Arc::new(AtomicBool::new(false));
    let heartbeat_stop = Arc::clone(&stop_heartbeats);
    let heartbeat_registry_url = registry_url.to_string();
    let heartbeat_runner_id = runner_id.to_string();
    let heartbeat_runner_token = runner_token.to_string();
    let heartbeat_claim_id = claim.claim_id.clone();
    let heartbeat_handle = thread::spawn(move || {
        while !heartbeat_stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(heartbeat_interval_ms));
            if heartbeat_stop.load(Ordering::Relaxed) {
                break;
            }
            if let Err(error) = heartbeat_remote_validation_stage_via_registry(
                &heartbeat_registry_url,
                &heartbeat_claim_id,
                &heartbeat_runner_id,
                &heartbeat_runner_token,
            ) {
                eprintln!(
                    "Remote validation runner heartbeat failed for claim '{}': {}",
                    heartbeat_claim_id, error
                );
                break;
            }
        }
    });

    let result =
        execute_remote_validation_claim(manifest_path, manifest, workspace_manifest, &claim)
            .and_then(|plan| {
                for command in &plan.commands {
                    println!("  > {}", command.argv.join(" "));
                    run_command(command)
                        .with_context(|| format!("remote runner stage '{}' failed", plan.stage))?;
                }

                let reason_code = claim
                    .suggested_pass_reason_code
                    .clone()
                    .filter(|value| {
                        claim.allowed_terminal_reason_codes.is_empty()
                            || claim
                                .allowed_terminal_reason_codes
                                .iter()
                                .any(|candidate| candidate.eq_ignore_ascii_case(value))
                    })
                    .unwrap_or_else(|| {
                        validation_stage_success_reason_code(&claim.stage_key).to_string()
                    });
                complete_remote_validation_stage_via_registry(
                    registry_url,
                    &claim.claim_id,
                    runner_id,
                    runner_token,
                    plan.success_detail,
                    reason_code,
                )
            });

    stop_heartbeats.store(true, Ordering::Relaxed);
    let _ = heartbeat_handle.join();

    if let Err(error) = result {
        let reason_code = claim
            .suggested_failure_reason_code
            .clone()
            .filter(|value| {
                claim.allowed_terminal_reason_codes.is_empty()
                    || claim
                        .allowed_terminal_reason_codes
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(value))
            })
            .unwrap_or_else(|| validation_stage_failure_reason_code(&claim.stage_key).to_string());
        let detail = format!(
            "{}: {}",
            executable_validation_stage_failure_prefix(&claim.stage_key),
            error
        );
        let report_error = fail_remote_validation_stage_via_registry(
            registry_url,
            &claim.claim_id,
            runner_id,
            runner_token,
            detail,
            reason_code,
        )
        .err()
        .map(|report_error| format!("; failed to report remote stage failure: {report_error}"))
        .unwrap_or_default();
        anyhow::bail!("{error}{report_error}");
    }

    Ok(true)
}

fn execute_remote_validation_claim(
    manifest_path: &Path,
    manifest: &Manifest,
    workspace_manifest: &TomlValue,
    claim: &RegistryRunnerClaimHttpPayload,
) -> Result<ModuleValidationStageRunPreview> {
    let spec = manifest.modules.get(&claim.slug).with_context(|| {
        format!(
            "Remote runner claimed unknown local module slug '{}'",
            claim.slug
        )
    })?;
    let preview =
        build_module_publish_preview(manifest_path, &claim.slug, spec, workspace_manifest)?;
    if preview.version != claim.version {
        anyhow::bail!(
            "Remote runner claimed '{}@{}', but local workspace resolves version '{}'",
            claim.slug,
            claim.version,
            preview.version
        );
    }
    if preview.crate_name != claim.crate_name {
        anyhow::bail!("Remote runner claim crate identity does not match the local publish bundle");
    }
    if claim.requires_manual_confirmation != (claim.stage_key == "security_policy_review") {
        anyhow::bail!("Remote runner claim has inconsistent manual confirmation policy");
    }
    let publish_artifact = build_publish_artifact_bytes(&preview)?;
    let local_checksum = format!("{:x}", Sha256::digest(&publish_artifact));
    if local_checksum != claim.artifact_checksum_sha256 {
        anyhow::bail!(
            "Remote runner local publish bundle does not match the owner-issued artifact digest"
        );
    }

    build_module_validation_stage_run_preview(
        &preview,
        &claim.request_id,
        &claim.stage_key,
        Some(remote_claim_running_detail(claim)),
    )
}

fn remote_claim_running_detail(claim: &RegistryRunnerClaimHttpPayload) -> String {
    format!(
        "Remote runner '{}' executing '{}' for module '{}' against the owner-issued artifact identity.",
        claim.execution_mode, claim.stage_key, claim.slug
    )
}
