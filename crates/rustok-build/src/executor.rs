//! Host-independent queued build execution.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, anyhow, bail};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    BuildCommandSpec, BuildEventPublisher, BuildExecutionReport, BuildService,
    build::{BuildStage, BuildStatus},
    build_manifest_snapshot_path, parse_execution_plan, run_build_command,
};

pub struct BuildExecutionService {
    build_service: BuildService,
    workspace_root: PathBuf,
}

impl BuildExecutionService {
    pub fn new(
        db: DatabaseConnection,
        event_publisher: Arc<dyn BuildEventPublisher>,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            build_service: BuildService::with_runtime(db, event_publisher),
            workspace_root,
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

        self.execute_build(build.id, dry_run).await.map(Some)
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

        let plan = parse_execution_plan(build.id, build.modules_delta.as_ref())?;
        let manifest_path = build_manifest_snapshot_path(build.id);
        let server_spec =
            BuildCommandSpec::from_server_plan(&plan, &self.workspace_root, manifest_path.clone());
        let admin_spec = plan
            .admin_build
            .as_ref()
            .map(|plan| {
                BuildCommandSpec::from_frontend_plan(
                    plan,
                    &self.workspace_root,
                    manifest_path.clone(),
                )
            })
            .transpose()?;
        let storefront_spec = plan
            .storefront_build
            .as_ref()
            .map(|plan| {
                BuildCommandSpec::from_frontend_plan(
                    plan,
                    &self.workspace_root,
                    manifest_path.clone(),
                )
            })
            .transpose()?;

        if dry_run {
            return Ok(report_for(
                build.id,
                "dry-run",
                &server_spec,
                admin_spec.as_ref(),
                storefront_spec.as_ref(),
            ));
        }

        self.build_service
            .update_build_status(
                build.id,
                BuildStatus::Running,
                Some(BuildStage::Checkout),
                Some(5),
            )
            .await?;
        materialize_manifest_snapshot(build.id, &build.manifest_snapshot, &manifest_path).await?;

        let mut specs = vec![("server", server_spec.clone())];
        if let Some(spec) = admin_spec.clone() {
            specs.push(("admin", spec));
        }
        if let Some(spec) = storefront_spec.clone() {
            specs.push(("storefront", spec));
        }
        let total_steps = specs.len().max(1);
        let result = async {
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

        match result {
            Ok(()) => {
                self.build_service
                    .update_build_status(
                        build.id,
                        BuildStatus::Success,
                        Some(BuildStage::Complete),
                        Some(100),
                    )
                    .await?;
                Ok(report_for(
                    build.id,
                    "success",
                    &server_spec,
                    admin_spec.as_ref(),
                    storefront_spec.as_ref(),
                ))
            }
            Err(error) => {
                self.build_service
                    .fail_build(build.id, error.to_string())
                    .await?;
                Err(error)
            }
        }
    }
}

fn report_for(
    build_id: Uuid,
    status: &str,
    server: &BuildCommandSpec,
    admin: Option<&BuildCommandSpec>,
    storefront: Option<&BuildCommandSpec>,
) -> BuildExecutionReport {
    BuildExecutionReport {
        build_id,
        status: status.to_string(),
        cargo_command: server.render(),
        admin_command: admin.map(BuildCommandSpec::render),
        storefront_command: storefront.map(BuildCommandSpec::render),
    }
}

async fn materialize_manifest_snapshot(
    build_id: Uuid,
    snapshot: &serde_json::Value,
    manifest_path: &Path,
) -> anyhow::Result<()> {
    let mut snapshot = snapshot.clone();
    remove_null_values(&mut snapshot);
    let manifest_toml = toml::to_string_pretty(&snapshot)
        .with_context(|| format!("failed to encode manifest snapshot for build {build_id}"))?;
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
        })
}

fn remove_null_values(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => values.iter_mut().for_each(remove_null_values),
        serde_json::Value::Object(values) => {
            values.retain(|_, value| !value.is_null());
            values.values_mut().for_each(remove_null_values);
        }
        _ => {}
    }
}
