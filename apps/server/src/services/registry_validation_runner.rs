use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{bail, Context};
use tokio::process::Command;

use crate::common::settings::RegistryValidationRunnerSettings;
use crate::models::registry_publish_request;
use crate::models::registry_validation_stage::{self, RegistryValidationStageStatus};
use crate::services::marketplace_catalog::RegistryPublishUiPackagesRequest;
use crate::services::registry_governance::{request_ui_packages, RegistryGovernanceService};

const DEFAULT_CARGO_BIN: &str = "cargo";
const BUILD_CARGO_BIN_ENV: &str = "RUSTOK_BUILD_CARGO_BIN";

#[derive(Debug, Clone)]
pub struct RegistryValidationRunnerService {
    governance: RegistryGovernanceService,
    config: RegistryValidationRunnerSettings,
}

#[derive(Debug, Clone)]
pub struct RegistryValidationStageExecutionReport {
    pub request_id: String,
    pub slug: String,
    pub stage_key: String,
    pub status: String,
}

#[derive(Debug, Clone)]
struct RegistryValidationStageExecutionPlan {
    success_detail: String,
    failure_detail_prefix: String,
    success_reason_code: &'static str,
    failure_reason_code: &'static str,
    commands: Vec<RegistryValidationStageCommand>,
}

#[derive(Debug, Clone)]
struct RegistryValidationStageCommand {
    label: String,
    program: String,
    args: Vec<String>,
}

impl RegistryValidationRunnerService {
    pub fn new(db: sea_orm::DatabaseConnection, config: RegistryValidationRunnerSettings) -> Self {
        Self {
            governance: RegistryGovernanceService::new(db),
            config,
        }
    }

    pub async fn execute_next_queued_stage(
        &self,
    ) -> anyhow::Result<Option<RegistryValidationStageExecutionReport>> {
        let supported_stages = supported_runner_stages(self.config.auto_confirm_manual_review);
        let Some(claim) = self
            .governance
            .claim_next_queued_validation_stage(&self.config.actor, &supported_stages)
            .await?
        else {
            return Ok(None);
        };

        let plan = build_execution_plan(&claim.request, &claim.stage)?;
        let result = self.run_plan(&plan).await;

        let completed = match result {
            Ok(()) => {
                self.governance
                    .complete_claimed_validation_stage(
                        claim.clone(),
                        &self.config.actor,
                        RegistryValidationStageStatus::Passed,
                        &plan.success_detail,
                        Some(plan.success_reason_code),
                    )
                    .await?
            }
            Err(error) => {
                let detail = format!("{}: {error}", plan.failure_detail_prefix);
                self.governance
                    .complete_claimed_validation_stage(
                        claim.clone(),
                        &self.config.actor,
                        RegistryValidationStageStatus::Failed,
                        &detail,
                        Some(plan.failure_reason_code),
                    )
                    .await?
            }
        };

        Ok(
            completed.map(|stage| RegistryValidationStageExecutionReport {
                request_id: claim.request.id.clone(),
                slug: claim.request.slug.clone(),
                stage_key: stage.stage_key,
                status: match stage.status {
                    RegistryValidationStageStatus::Queued => "queued",
                    RegistryValidationStageStatus::Running => "running",
                    RegistryValidationStageStatus::Passed => "passed",
                    RegistryValidationStageStatus::Failed => "failed",
                    RegistryValidationStageStatus::Blocked => "blocked",
                }
                .to_string(),
            }),
        )
    }

    async fn run_plan(&self, plan: &RegistryValidationStageExecutionPlan) -> anyhow::Result<()> {
        for command in &plan.commands {
            run_command(command).await.with_context(|| {
                format!(
                    "registry validation runner command '{}' failed",
                    command.label
                )
            })?;
        }
        Ok(())
    }
}

fn supported_runner_stages(auto_confirm_manual_review: bool) -> Vec<&'static str> {
    let mut stages = vec!["compile_smoke", "targeted_tests"];
    if auto_confirm_manual_review {
        stages.push("security_policy_review");
    }
    stages
}

fn build_execution_plan(
    request: &registry_publish_request::Model,
    stage: &registry_validation_stage::Model,
) -> anyhow::Result<RegistryValidationStageExecutionPlan> {
    let ui_packages = request_ui_packages(request);
    let commands = match stage.stage_key.as_str() {
        "compile_smoke" => build_compile_smoke_commands(request, &ui_packages),
        "targeted_tests" => build_targeted_test_commands(request, &ui_packages),
        "security_policy_review" => build_security_policy_review_commands(request),
        other => bail!("Registry validation runner does not support stage '{other}'"),
    };

    Ok(RegistryValidationStageExecutionPlan {
        success_detail: executable_validation_stage_success_detail(
            stage.stage_key.as_str(),
            &request.slug,
        ),
        failure_detail_prefix: executable_validation_stage_failure_prefix(stage.stage_key.as_str()),
        success_reason_code: validation_stage_success_reason_code(stage.stage_key.as_str()),
        failure_reason_code: validation_stage_failure_reason_code(stage.stage_key.as_str()),
        commands,
    })
}

fn build_compile_smoke_commands(
    request: &registry_publish_request::Model,
    ui_packages: &RegistryPublishUiPackagesRequest,
) -> Vec<RegistryValidationStageCommand> {
    let mut commands = vec![package_check_command("module crate", &request.crate_name)];

    if let Some(admin) = ui_packages.admin.as_ref() {
        commands.push(package_check_command("admin ui crate", &admin.crate_name));
    }

    if let Some(storefront) = ui_packages.storefront.as_ref() {
        commands.push(package_check_command(
            "storefront ui crate",
            &storefront.crate_name,
        ));
    }

    commands
}

fn build_targeted_test_commands(
    request: &registry_publish_request::Model,
    ui_packages: &RegistryPublishUiPackagesRequest,
) -> Vec<RegistryValidationStageCommand> {
    let mut commands = vec![xtask_command(
        "module validate",
        &["module", "validate", request.slug.as_str()],
    )];
    commands.extend(build_compile_smoke_commands(request, ui_packages));
    commands
}

fn build_security_policy_review_commands(
    request: &registry_publish_request::Model,
) -> Vec<RegistryValidationStageCommand> {
    vec![
        xtask_command(
            "module validate",
            &["module", "validate", request.slug.as_str()],
        ),
        xtask_command(
            "module test dry-run",
            &["module", "test", request.slug.as_str(), "--dry-run"],
        ),
    ]
}

fn package_check_command(label: &str, package: &str) -> RegistryValidationStageCommand {
    RegistryValidationStageCommand {
        label: label.to_string(),
        program: cargo_bin(),
        args: vec!["check".to_string(), "-p".to_string(), package.to_string()],
    }
}

fn xtask_command(label: &str, args: &[&str]) -> RegistryValidationStageCommand {
    let mut argv = vec![
        "run".to_string(),
        "-p".to_string(),
        "xtask".to_string(),
        "--".to_string(),
    ];
    argv.extend(args.iter().map(|value| (*value).to_string()));
    RegistryValidationStageCommand {
        label: label.to_string(),
        program: cargo_bin(),
        args: argv,
    }
}

fn cargo_bin() -> String {
    std::env::var(BUILD_CARGO_BIN_ENV).unwrap_or_else(|_| DEFAULT_CARGO_BIN.to_string())
}

fn executable_validation_stage_success_detail(stage: &str, slug: &str) -> String {
    match stage {
        "compile_smoke" => format!("Local compile smoke checks passed for module '{slug}'."),
        "targeted_tests" => {
            format!("Local targeted tests completed successfully for module '{slug}'.")
        }
        "security_policy_review" => format!(
            "Local security/policy preflight completed and manual review was confirmed for module '{slug}'."
        ),
        _ => format!("Local validation stage '{stage}' completed successfully for '{slug}'."),
    }
}

fn executable_validation_stage_failure_prefix(stage: &str) -> String {
    match stage {
        "compile_smoke" => "Local compile smoke failed".to_string(),
        "targeted_tests" => "Local targeted tests failed".to_string(),
        "security_policy_review" => "Local security/policy review preflight failed".to_string(),
        _ => format!("Local validation stage '{stage}' failed"),
    }
}

fn validation_stage_success_reason_code(stage: &str) -> &'static str {
    match stage {
        "security_policy_review" => "manual_review_complete",
        _ => "local_runner_passed",
    }
}

fn validation_stage_failure_reason_code(stage: &str) -> &'static str {
    match stage {
        "compile_smoke" => "build_failure",
        "targeted_tests" => "test_failure",
        "security_policy_review" => "policy_preflight_failed",
        _ => "other",
    }
}

async fn run_command(command: &RegistryValidationStageCommand) -> anyhow::Result<()> {
    let status = Command::new(&command.program)
        .args(&command.args)
        .current_dir(workspace_root())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .with_context(|| {
            format!(
                "failed to launch registry validation runner command '{} {}'",
                command.program,
                command.args.join(" ")
            )
        })?;

    if !status.success() {
        let exit_code = status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string());
        bail!(
            "command failed with exit status {exit_code}: {} {}",
            command.program,
            command.args.join(" ")
        );
    }

    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from apps/server")
}
