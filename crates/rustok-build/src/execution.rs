use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, bail};
use tokio::process::Command;
use uuid::Uuid;

use crate::{BuildExecutionPlan, FrontendBuildPlan, FrontendBuildTool};

const DEFAULT_CARGO_BIN: &str = "cargo";
const BUILD_CARGO_BIN_ENV: &str = "RUSTOK_BUILD_CARGO_BIN";
const DEFAULT_TRUNK_BIN: &str = "trunk";
const BUILD_TRUNK_BIN_ENV: &str = "RUSTOK_BUILD_TRUNK_BIN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildCommandSpec {
    program: String,
    args: Vec<String>,
    workdir: PathBuf,
    manifest_path: PathBuf,
}

impl BuildCommandSpec {
    pub fn from_server_plan(
        plan: &BuildExecutionPlan,
        workspace_root: &Path,
        manifest_path: PathBuf,
    ) -> Self {
        let program =
            std::env::var(BUILD_CARGO_BIN_ENV).unwrap_or_else(|_| DEFAULT_CARGO_BIN.to_string());
        let mut args = vec![
            "build".to_string(),
            "-p".to_string(),
            plan.cargo_package.clone(),
        ];
        if plan.cargo_profile == "release" {
            args.push("--release".to_string());
        } else {
            args.push("--profile".to_string());
            args.push(plan.cargo_profile.clone());
        }
        if let Some(target) = &plan.cargo_target {
            args.push("--target".to_string());
            args.push(target.clone());
        }
        if !plan.cargo_features.is_empty() {
            args.push("--features".to_string());
            args.push(plan.cargo_features.join(","));
        }

        Self {
            program,
            args,
            workdir: workspace_root.to_path_buf(),
            manifest_path,
        }
    }

    pub fn from_frontend_plan(
        plan: &FrontendBuildPlan,
        workspace_root: &Path,
        manifest_path: PathBuf,
    ) -> anyhow::Result<Self> {
        let workdir = workspace_root.join(&plan.workspace_path);

        match plan.tool {
            FrontendBuildTool::Cargo => {
                let program = std::env::var(BUILD_CARGO_BIN_ENV)
                    .unwrap_or_else(|_| DEFAULT_CARGO_BIN.to_string());
                let mut args = vec!["build".to_string(), "-p".to_string(), plan.package.clone()];
                if plan.profile == "release" {
                    args.push("--release".to_string());
                } else {
                    args.push("--profile".to_string());
                    args.push(plan.profile.clone());
                }
                if let Some(target) = &plan.target {
                    args.push("--target".to_string());
                    args.push(target.to_string());
                }

                Ok(Self {
                    program,
                    args,
                    workdir,
                    manifest_path,
                })
            }
            FrontendBuildTool::Trunk => {
                let program = std::env::var(BUILD_TRUNK_BIN_ENV)
                    .unwrap_or_else(|_| DEFAULT_TRUNK_BIN.to_string());
                let mut args = vec!["build".to_string()];
                if plan.profile == "release" {
                    args.push("--release".to_string());
                }

                Ok(Self {
                    program,
                    args,
                    workdir,
                    manifest_path,
                })
            }
        }
    }

    pub fn render(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn workdir(&self) -> &Path {
        &self.workdir
    }
}

pub fn build_manifest_snapshot_path(build_id: Uuid) -> PathBuf {
    std::env::temp_dir()
        .join("rustok-build-manifests")
        .join(build_id.to_string())
        .join("modules.toml")
}

pub async fn run_build_command(spec: &BuildCommandSpec) -> anyhow::Result<()> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.workdir)
        .env("RUSTOK_MODULES_MANIFEST", &spec.manifest_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = command
        .status()
        .await
        .with_context(|| format!("failed to spawn {}", spec.render()))?;

    if !status.success() {
        let exit_code = status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string());
        bail!(
            "build command failed with exit status {exit_code}: {}",
            spec.render()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{BuildCommandSpec, build_manifest_snapshot_path};
    use crate::{
        BuildExecutionPlan, BuildRuntimeMode, FrontendArtifactKind, FrontendBuildPlan,
        FrontendBuildTool,
    };

    #[test]
    fn derives_command_spec_from_plan() {
        let plan = BuildExecutionPlan {
            runtime_mode: BuildRuntimeMode::Full,
            cargo_package: "rustok-server".to_string(),
            cargo_profile: "release".to_string(),
            cargo_target: Some("x86_64-unknown-linux-gnu".to_string()),
            cargo_features: vec!["embed-admin".to_string()],
            cargo_command: String::new(),
            admin_build: None,
            storefront_build: None,
        };

        let spec = BuildCommandSpec::from_server_plan(
            &plan,
            Path::new("workspace"),
            build_manifest_snapshot_path(uuid::Uuid::nil()),
        );
        assert_eq!(
            spec.args()[0..4],
            ["build", "-p", "rustok-server", "--release"]
        );
        assert!(
            spec.args()
                .contains(&"x86_64-unknown-linux-gnu".to_string())
        );
        assert!(spec.args().contains(&"embed-admin".to_string()));
    }

    #[test]
    fn derives_trunk_command_spec_from_frontend_plan() {
        let plan = FrontendBuildPlan {
            surface: "admin".to_string(),
            tool: FrontendBuildTool::Trunk,
            package: "rustok-admin".to_string(),
            workspace_path: "apps/admin".to_string(),
            profile: "release".to_string(),
            target: None,
            artifact_path: "apps/admin/dist".to_string(),
            artifact_kind: FrontendArtifactKind::Directory,
            command: "trunk build --release".to_string(),
        };

        let spec = BuildCommandSpec::from_frontend_plan(
            &plan,
            Path::new("workspace"),
            build_manifest_snapshot_path(uuid::Uuid::nil()),
        )
        .unwrap();
        assert_eq!(spec.program(), "trunk");
        assert_eq!(spec.args(), ["build", "--release"]);
        assert!(spec.workdir().ends_with("apps/admin"));
    }
}
