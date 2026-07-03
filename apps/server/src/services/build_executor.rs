use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context};
use serde::Serialize;
use tokio::process::Command;
use uuid::Uuid;

use crate::models::build::{BuildStage, BuildStatus, Model as Build};
use crate::models::release::Model as Release;
use crate::modules::{BuildExecutionPlan, FrontendBuildPlan, FrontendBuildTool, ModulesManifest};
use crate::services::build_event_hub::{build_event_hub_from_context, BuildEventHubPublisher};
use crate::services::build_service::{BuildEventPublisher, BuildService};
use crate::services::server_runtime_context::ServerRuntimeContext;

const DEFAULT_CARGO_BIN: &str = "cargo";
const BUILD_CARGO_BIN_ENV: &str = "RUSTOK_BUILD_CARGO_BIN";
const DEFAULT_TRUNK_BIN: &str = "trunk";
const BUILD_TRUNK_BIN_ENV: &str = "RUSTOK_BUILD_TRUNK_BIN";

#[derive(Debug, Clone, Serialize)]
pub struct BuildExecutionReport {
    pub build_id: Uuid,
    pub status: String,
    pub cargo_command: String,
    pub admin_command: Option<String>,
    pub storefront_command: Option<String>,
    pub release_id: Option<String>,
    pub release_status: Option<String>,
}

pub struct BuildExecutionService {
    build_service: BuildService,
}

impl BuildExecutionService {
    pub fn new(ctx: &ServerRuntimeContext) -> Self {
        Self::with_event_publisher(
            ctx,
            Arc::new(BuildEventHubPublisher::new(build_event_hub_from_context(
                ctx,
            ))),
        )
    }

    pub fn with_event_publisher(
        ctx: &ServerRuntimeContext,
        event_publisher: Arc<dyn BuildEventPublisher>,
    ) -> Self {
        Self {
            build_service: BuildService::with_event_publisher(ctx.db_clone(), event_publisher),
        }
    }

    pub async fn execute_next_queued_build(
        &self,
        dry_run: bool,
    ) -> anyhow::Result<Option<BuildExecutionReport>> {
        if let Some(running) = self.build_service.running_build().await? {
            bail!("build {} is already running", running.id);
        }

        let Some(build) = self.build_service.next_queued_build().await? else {
            return Ok(None);
        };

        let report = self.execute_build(build.id, dry_run).await?;
        Ok(Some(report))
    }

    pub async fn execute_build(
        &self,
        build_id: Uuid,
        dry_run: bool,
    ) -> anyhow::Result<BuildExecutionReport> {
        let build = self
            .build_service
            .get_build(build_id)
            .await?
            .ok_or_else(|| anyhow!("Build not found"))?;

        if build.is_final() {
            bail!(
                "build {} is already final and cannot be executed again",
                build.id
            );
        }

        if build.status == BuildStatus::Running {
            bail!("build {} is already running", build.id);
        }

        if let Some(running) = self.build_service.running_build().await? {
            if running.id != build.id {
                bail!("build {} is already running", running.id);
            }
        }

        let plan = build_execution_plan(&build)?;
        let manifest_path = build_manifest_snapshot_path(build.id);
        let server_spec = BuildCommandSpec::from_server_plan(&plan, manifest_path.clone());
        let admin_spec = plan
            .admin_build
            .as_ref()
            .map(|plan| BuildCommandSpec::from_frontend_plan(plan, manifest_path.clone()))
            .transpose()?;
        let storefront_spec = plan
            .storefront_build
            .as_ref()
            .map(|plan| BuildCommandSpec::from_frontend_plan(plan, manifest_path.clone()))
            .transpose()?;

        if dry_run {
            return Ok(BuildExecutionReport {
                build_id: build.id,
                status: "dry-run".to_string(),
                cargo_command: server_spec.render(),
                admin_command: admin_spec.as_ref().map(BuildCommandSpec::render),
                storefront_command: storefront_spec.as_ref().map(BuildCommandSpec::render),
                release_id: None,
                release_status: None,
            });
        }

        self.build_service
            .update_build_status(
                build.id,
                BuildStatus::Running,
                Some(BuildStage::Checkout),
                Some(5),
            )
            .await?;

        materialize_build_manifest_snapshot(&build, &manifest_path).await?;

        let mut specs = vec![("server", server_spec.clone())];
        if let Some(spec) = admin_spec.clone() {
            specs.push(("admin", spec));
        }
        if let Some(spec) = storefront_spec.clone() {
            specs.push(("storefront", spec));
        }

        let total_steps = specs.len().max(1);
        let status = async {
            for (index, (label, spec)) in specs.iter().enumerate() {
                let progress = 15 + (((index + 1) * 75) / total_steps) as i32;
                self.build_service
                    .update_build_status(
                        build.id,
                        BuildStatus::Running,
                        Some(BuildStage::Build),
                        Some(progress),
                    )
                    .await?;

                run_build_command(spec).await.with_context(|| {
                    format!(
                        "failed to execute {label} build command for build {}",
                        build.id
                    )
                })?;
            }

            Ok::<(), anyhow::Error>(())
        }
        .await;

        match status {
            Ok(()) => {
                self.build_service
                    .update_build_status(
                        build.id,
                        BuildStatus::Success,
                        Some(BuildStage::Complete),
                        Some(100),
                    )
                    .await?;

                Ok(BuildExecutionReport {
                    build_id: build.id,
                    status: "success".to_string(),
                    cargo_command: server_spec.render(),
                    admin_command: admin_spec.as_ref().map(BuildCommandSpec::render),
                    storefront_command: storefront_spec.as_ref().map(BuildCommandSpec::render),
                    release_id: None,
                    release_status: None,
                })
            }
            Err(error) => {
                self.build_service
                    .fail_build(build.id, error.to_string())
                    .await?;

                Err(error)
            }
        }
    }

    pub async fn ensure_release_for_build(
        &self,
        build_id: Uuid,
        environment: &str,
        activate: bool,
    ) -> anyhow::Result<Release> {
        let build = self
            .build_service
            .get_build(build_id)
            .await?
            .ok_or_else(|| anyhow!("Build not found"))?;

        if build.status != BuildStatus::Success {
            bail!(
                "build {} must be successful before creating a release",
                build.id
            );
        }

        let release = if let Some(release_id) = build.release_id.clone() {
            self.build_service
                .get_release(&release_id)
                .await?
                .ok_or_else(|| anyhow!("release {} referenced by build is missing", release_id))?
        } else {
            let modules = build_module_slugs(&build)?;
            self.build_service
                .create_release(build.id, environment.to_string(), modules)
                .await?
        };

        if activate && release.status != crate::models::release::ReleaseStatus::Active {
            return self.build_service.activate_release(&release.id).await;
        }

        Ok(release)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildCommandSpec {
    program: String,
    args: Vec<String>,
    workdir: PathBuf,
    manifest_path: PathBuf,
}

impl BuildCommandSpec {
    fn from_server_plan(plan: &BuildExecutionPlan, manifest_path: PathBuf) -> Self {
        let program =
            std::env::var(BUILD_CARGO_BIN_ENV).unwrap_or_else(|_| DEFAULT_CARGO_BIN.to_string());
        let workdir = workspace_root();

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
            workdir,
            manifest_path,
        }
    }

    fn from_frontend_plan(
        plan: &FrontendBuildPlan,
        manifest_path: PathBuf,
    ) -> anyhow::Result<Self> {
        let workdir = workspace_root().join(&plan.workspace_path);

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

    fn render(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from apps/server")
}

fn build_execution_plan(build: &Build) -> anyhow::Result<BuildExecutionPlan> {
    let value = build
        .modules_delta
        .as_ref()
        .ok_or_else(|| anyhow!("build {} does not contain execution metadata", build.id))?;

    let plan = value
        .get("execution_plan")
        .ok_or_else(|| anyhow!("build {} is missing execution_plan metadata", build.id))?;

    serde_json::from_value(plan.clone()).map_err(|error| {
        anyhow!(
            "build {} has invalid execution_plan metadata: {error}",
            build.id
        )
    })
}

fn build_manifest_snapshot_path(build_id: Uuid) -> PathBuf {
    std::env::temp_dir()
        .join("rustok-build-manifests")
        .join(build_id.to_string())
        .join("modules.toml")
}

async fn materialize_build_manifest_snapshot(
    build: &Build,
    manifest_path: &PathBuf,
) -> anyhow::Result<()> {
    let manifest: ModulesManifest = serde_json::from_value(build.manifest_snapshot.clone())
        .with_context(|| format!("build {} has invalid manifest_snapshot", build.id))?;
    let manifest_toml = toml::to_string_pretty(&manifest)
        .with_context(|| format!("failed to encode manifest snapshot for build {}", build.id))?;
    if let Some(parent) = manifest_path.parent() {
        tokio::fs::create_dir_all(parent).await.with_context(|| {
            format!(
                "failed to create build manifest directory {}",
                parent.display()
            )
        })?;
    }
    tokio::fs::write(manifest_path, manifest_toml)
        .await
        .with_context(|| {
            format!(
                "failed to write build manifest snapshot {}",
                manifest_path.display()
            )
        })?;
    Ok(())
}

fn build_module_slugs(build: &Build) -> anyhow::Result<Vec<String>> {
    let value = build
        .modules_delta
        .as_ref()
        .ok_or_else(|| anyhow!("build {} does not contain module metadata", build.id))?;

    let modules = value
        .get("modules")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!("build {} is missing modules metadata", build.id))?;

    let mut slugs = modules.keys().cloned().collect::<Vec<_>>();
    slugs.sort();
    Ok(slugs)
}

async fn run_build_command(spec: &BuildCommandSpec) -> anyhow::Result<()> {
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
    use super::{
        build_execution_plan, build_manifest_snapshot_path, workspace_root, BuildCommandSpec,
    };
    use crate::models::build::{DeploymentProfile, Model as Build};
    use crate::modules::{
        BuildExecutionPlan, FrontendArtifactKind, FrontendBuildPlan, FrontendBuildTool,
    };

    fn build_with_plan(plan: &BuildExecutionPlan) -> Build {
        let mut build = Build::new(
            "refs/heads/main".to_string(),
            "hash".to_string(),
            1,
            serde_json::json!({}),
            "tester".to_string(),
            DeploymentProfile::Monolith,
        );
        build.modules_delta = Some(serde_json::json!({
            "summary": "+pages",
            "execution_plan": plan,
        }));
        build
    }

    #[test]
    fn parses_execution_plan_from_build_metadata() {
        let plan = BuildExecutionPlan {
            cargo_package: "rustok-server".to_string(),
            cargo_profile: "release".to_string(),
            cargo_target: Some("x86_64-unknown-linux-gnu".to_string()),
            cargo_features: vec!["embed-admin".to_string(), "embed-storefront".to_string()],
            cargo_command: "cargo build -p rustok-server --release".to_string(),
            admin_build: Some(FrontendBuildPlan {
                surface: "admin".to_string(),
                tool: FrontendBuildTool::Trunk,
                package: "rustok-admin".to_string(),
                workspace_path: "apps/admin".to_string(),
                profile: "release".to_string(),
                target: None,
                artifact_path: "apps/admin/dist".to_string(),
                artifact_kind: FrontendArtifactKind::Directory,
                command: "trunk build --release".to_string(),
            }),
            storefront_build: None,
        };

        let parsed = build_execution_plan(&build_with_plan(&plan)).unwrap();
        assert_eq!(parsed, plan);
    }

    #[test]
    fn derives_command_spec_from_plan() {
        let plan = BuildExecutionPlan {
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
            build_manifest_snapshot_path(uuid::Uuid::nil()),
        );
        assert_eq!(
            spec.args[0..4],
            ["build", "-p", "rustok-server", "--release"]
        );
        assert!(spec.args.contains(&"x86_64-unknown-linux-gnu".to_string()));
        assert!(spec.args.contains(&"embed-admin".to_string()));
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
            build_manifest_snapshot_path(uuid::Uuid::nil()),
        )
        .unwrap();
        assert_eq!(spec.program, "trunk");
        assert_eq!(
            spec.args,
            vec!["build".to_string(), "--release".to_string()]
        );
        assert!(spec.workdir.ends_with("apps\\admin") || spec.workdir.ends_with("apps/admin"));
    }

    #[test]
    fn extracts_sorted_module_slugs_from_build_metadata() {
        let plan = BuildExecutionPlan {
            cargo_package: "rustok-server".to_string(),
            cargo_profile: "release".to_string(),
            cargo_target: None,
            cargo_features: vec![],
            cargo_command: String::new(),
            admin_build: None,
            storefront_build: None,
        };

        let mut build = build_with_plan(&plan);
        build.modules_delta = Some(serde_json::json!({
            "summary": "+pages,+blog",
            "modules": {
                "pages": { "crate_name": "rustok-pages" },
                "blog": { "crate_name": "rustok-blog" }
            },
            "execution_plan": plan,
        }));

        let modules = super::build_module_slugs(&build).unwrap();
        assert_eq!(modules, vec!["blog".to_string(), "pages".to_string()]);
    }

    #[test]
    fn workspace_root_points_to_repo_root() {
        assert!(workspace_root().join("modules.toml").exists());
    }
}
