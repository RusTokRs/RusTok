use std::{
    fs,
    path::Path,
    process::Stdio,
};

use tokio::{process::Command, time::timeout};

use super::{
    StaticDistributionJobConfig, StaticDistributionJobError, StaticDistributionJobRequest,
    create_directory_path, digest_bounded_regular, failed_outcome, io_error, validate_directory,
    write_terminal_receipt, MAX_CARGO_LOCK_BYTES, WORKSPACE_LOCK_FILE,
};

pub(super) struct CargoPipelineEvidence {
    pub(super) lock_command: Vec<String>,
    pub(super) test_command: Vec<String>,
    pub(super) build_command: Vec<String>,
    pub(super) resolved_lock_digest: String,
}

struct StageOutcomeDesc<'a> {
    failed_code: &'a str,
    timeout_code: &'a str,
    failed_detail: &'a str,
    timeout_detail: &'a str,
}

struct StageContext<'a> {
    config: &'a StaticDistributionJobConfig,
    workspace: &'a Path,
    job_receipt_path: &'a Path,
    request: &'a StaticDistributionJobRequest,
    job_request_digest: &'a str,
}

impl<'a> StageContext<'a> {
    async fn run_stage(
        &self,
        command_args: &[String],
        desc: StageOutcomeDesc<'a>,
    ) -> Result<bool, StaticDistributionJobError> {
        match run_fixed_command(&self.config.cargo_path, command_args, self.workspace, self.config).await? {
            FixedCommandOutcome::Succeeded => Ok(true),
            FixedCommandOutcome::Failed => {
                write_terminal_receipt(
                    self.job_receipt_path,
                    self.request,
                    self.job_request_digest,
                    failed_outcome(desc.failed_code, desc.failed_detail),
                )?;
                Ok(false)
            }
            FixedCommandOutcome::TimedOut => {
                write_terminal_receipt(
                    self.job_receipt_path,
                    self.request,
                    self.job_request_digest,
                    failed_outcome(desc.timeout_code, desc.timeout_detail),
                )?;
                Ok(false)
            }
        }
    }
}

async fn run_lock_resolution(
    ctx: &StageContext<'_>,
    workspace: &Path,
) -> Result<Option<(Vec<String>, String)>, StaticDistributionJobError> {
    let lock_command = cargo_lock_command();
    let lock_succeeded = ctx
        .run_stage(
            &lock_command,
            StageOutcomeDesc {
                failed_code: "static_lock_resolution_failed",
                timeout_code: "static_lock_resolution_timed_out",
                failed_detail: "static distribution dependency lock resolution failed",
                timeout_detail:
                    "static distribution dependency lock resolution exceeded the command deadline",
            },
        )
        .await?;
    if !lock_succeeded {
        return Ok(None);
    }
    let resolved_lock_digest = digest_bounded_regular(
        &workspace.join(WORKSPACE_LOCK_FILE),
        MAX_CARGO_LOCK_BYTES,
    )?;
    Ok(Some((lock_command, resolved_lock_digest)))
}

async fn run_tests_and_build(
    ctx: &StageContext<'_>,
) -> Result<Option<(Vec<String>, Vec<String>)>, StaticDistributionJobError> {
    let test_command = cargo_test_command(ctx.config);
    let tests_succeeded = ctx
        .run_stage(
            &test_command,
            StageOutcomeDesc {
                failed_code: "static_tests_failed",
                timeout_code: "static_tests_timed_out",
                failed_detail: "static distribution tests failed",
                timeout_detail: "static distribution tests exceeded the command deadline",
            },
        )
        .await?;
    if !tests_succeeded {
        return Ok(None);
    }

    let build_command = cargo_build_command(ctx.config);
    let build_succeeded = ctx
        .run_stage(
            &build_command,
            StageOutcomeDesc {
                failed_code: "static_build_failed",
                timeout_code: "static_build_timed_out",
                failed_detail: "static distribution release build failed",
                timeout_detail: "static distribution build exceeded the command deadline",
            },
        )
        .await?;
    if !build_succeeded {
        return Ok(None);
    }

    Ok(Some((test_command, build_command)))
}

pub(super) async fn run_cargo_pipeline(
    config: &StaticDistributionJobConfig,
    workspace: &Path,
    job_receipt_path: &Path,
    request: &StaticDistributionJobRequest,
    job_request_digest: &str,
) -> Result<Option<CargoPipelineEvidence>, StaticDistributionJobError> {
    let ctx = StageContext {
        config,
        workspace,
        job_receipt_path,
        request,
        job_request_digest,
    };
    let Some((lock_command, resolved_lock_digest)) =
        run_lock_resolution(&ctx, workspace).await?
    else {
        return Ok(None);
    };
    let Some((test_command, build_command)) = run_tests_and_build(&ctx).await? else {
        return Ok(None);
    };
    Ok(Some(CargoPipelineEvidence {
        lock_command,
        test_command,
        build_command,
        resolved_lock_digest,
    }))
}

enum FixedCommandOutcome {
    Succeeded,
    Failed,
    TimedOut,
}

fn cargo_test_command(config: &StaticDistributionJobConfig) -> Vec<String> {
    vec![
        "test".to_string(),
        "--locked".to_string(),
        "--offline".to_string(),
        "--workspace".to_string(),
        "--all-targets".to_string(),
        "--target".to_string(),
        config.build_target.clone(),
    ]
}

fn cargo_lock_command() -> Vec<String> {
    vec!["generate-lockfile".to_string(), "--offline".to_string()]
}

fn cargo_build_command(config: &StaticDistributionJobConfig) -> Vec<String> {
    vec![
        "build".to_string(),
        "--locked".to_string(),
        "--offline".to_string(),
        "--workspace".to_string(),
        "--release".to_string(),
        "--target".to_string(),
        config.build_target.clone(),
    ]
}

async fn run_fixed_command(
    program: &Path,
    arguments: &[String],
    workspace: &Path,
    config: &StaticDistributionJobConfig,
) -> Result<FixedCommandOutcome, StaticDistributionJobError> {
    config.validate_runtime()?;
    validate_cargo_home(&config.cargo_home)?;
    let target_dir = workspace.join(".rustok").join("target");
    let home_dir = workspace.join(".rustok").join("home");
    create_directory_path(&target_dir)?;
    create_directory_path(&home_dir)?;
    let mut command = Command::new(program);
    command
        .args(arguments)
        .current_dir(workspace)
        .env_clear()
        .env("CARGO_HOME", &config.cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("CARGO_TERM_COLOR", "never")
        .env("HOME", &home_dir)
        .env("RUSTC", &config.rustc_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let status = match timeout(config.command_timeout(), command.status()).await {
        Ok(status) => {
            status.map_err(|error| StaticDistributionJobError::Command(error.to_string()))?
        }
        Err(_) => return Ok(FixedCommandOutcome::TimedOut),
    };
    if status.success() {
        Ok(FixedCommandOutcome::Succeeded)
    } else {
        Ok(FixedCommandOutcome::Failed)
    }
}

pub(super) fn validate_cargo_home(path: &Path) -> Result<(), StaticDistributionJobError> {
    validate_directory(path, "Cargo home")?;
    for name in ["config", "config.toml", "credentials", "credentials.toml"] {
        match fs::symlink_metadata(path.join(name)) {
            Ok(_) => {
                return Err(StaticDistributionJobError::InvalidConfig(
                    "Cargo home must not contain config or credential files".to_string(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
    }
    Ok(())
}
