use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, anyhow, bail};
use chrono::Utc;
use reqwest::multipart;
use serde::Serialize;
use tokio::process::Command;

use crate::common::settings::{BuildDeploymentBackendKind, BuildDeploymentSettings};
use crate::services::release_activation_hook::ServerReleaseActivationHook;
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_build::build::Model as Build;
use rustok_build::release::Model as Release;
use rustok_build::{
    BuildExecutionPlan, DeploymentWorkspace, FrontendArtifactKind, FrontendBuildPlan,
};
use rustok_build::{
    BuildService, ReleaseArtifactBundle, ReleasePublishRequest, ReleasePublisherPort,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ReleasePublishState {
    Deploying,
    Active,
    Failed,
}

#[derive(Debug, Clone)]
struct ReleasePublishOutcome {
    artifacts: ReleaseArtifactBundle,
    state: ReleasePublishState,
}

#[derive(Debug, Clone)]
struct PreparedReleaseBundle {
    release_dir: PathBuf,
    server_artifact_url: String,
    admin_artifact_url: Option<String>,
    storefront_artifact_url: Option<String>,
}

pub struct ReleaseDeploymentService {
    build_service: BuildService,
    config: BuildDeploymentSettings,
    workspace: DeploymentWorkspace,
}

impl ReleaseDeploymentService {
    pub fn new(ctx: &ServerRuntimeContext, config: BuildDeploymentSettings) -> Self {
        Self::with_database_in_workspace(ctx.db_clone(), config, server_deployment_workspace())
    }

    /// Creates the host publication adapter for an installer-opened database.
    pub fn with_database(db: sea_orm::DatabaseConnection, config: BuildDeploymentSettings) -> Self {
        Self::with_database_in_workspace(db, config, server_deployment_workspace())
    }

    /// Creates a publication adapter with host-supplied artifact and runtime paths.
    pub fn with_database_in_workspace(
        db: sea_orm::DatabaseConnection,
        config: BuildDeploymentSettings,
        workspace: DeploymentWorkspace,
    ) -> Self {
        Self {
            build_service: BuildService::with_runtime(
                db.clone(),
                std::sync::Arc::new(rustok_build::NoopBuildEventPublisher),
                std::sync::Arc::new(ServerReleaseActivationHook::new(db)),
            ),
            config,
            workspace,
        }
    }

    pub async fn publish_release(
        &self,
        release_id: &str,
        activate: bool,
    ) -> anyhow::Result<Release> {
        let release = self
            .build_service
            .get_release(release_id)
            .await?
            .ok_or_else(|| anyhow!("Release not found"))?;
        let build = self
            .build_service
            .get_build(release.build_id)
            .await?
            .ok_or_else(|| anyhow!("Build {} for release is missing", release.build_id))?;
        let plan = build_execution_plan(&build)?;

        self.build_service
            .mark_release_deploying(release_id)
            .await?;

        let outcome = match publish_release_artifacts(
            &self.config,
            &self.workspace,
            &release,
            &build,
            &plan,
            activate,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                let _ = self.build_service.fail_release(release_id).await;
                return Err(error);
            }
        };

        let release = self
            .build_service
            .attach_release_artifacts(release_id, outcome.artifacts)
            .await?;

        match outcome.state {
            ReleasePublishState::Deploying => {
                if activate && self.config.backend != BuildDeploymentBackendKind::Http {
                    self.build_service.activate_release(&release.id).await
                } else {
                    Ok(release)
                }
            }
            ReleasePublishState::Active => self.build_service.activate_release(&release.id).await,
            ReleasePublishState::Failed => self.build_service.fail_release(&release.id).await,
        }
    }
}

#[async_trait::async_trait]
impl ReleasePublisherPort for ReleaseDeploymentService {
    async fn publish_release(&self, request: ReleasePublishRequest) -> anyhow::Result<Release> {
        ReleaseDeploymentService::publish_release(self, &request.release_id, request.activate).await
    }
}

async fn publish_release_artifacts(
    config: &BuildDeploymentSettings,
    workspace: &DeploymentWorkspace,
    release: &Release,
    build: &Build,
    plan: &BuildExecutionPlan,
    activate: bool,
) -> anyhow::Result<ReleasePublishOutcome> {
    match config.backend {
        BuildDeploymentBackendKind::RecordOnly => Ok(ReleasePublishOutcome {
            artifacts: ReleaseArtifactBundle::default(),
            state: ReleasePublishState::Deploying,
        }),
        BuildDeploymentBackendKind::Filesystem => {
            publish_release_to_filesystem(config, workspace, release, build, plan).await
        }
        BuildDeploymentBackendKind::Http => {
            publish_release_to_http(config, workspace, release, build, plan).await
        }
        BuildDeploymentBackendKind::Container => {
            publish_release_to_container(config, workspace, release, build, plan, activate).await
        }
    }
}

async fn publish_release_to_filesystem(
    config: &BuildDeploymentSettings,
    workspace: &DeploymentWorkspace,
    release: &Release,
    build: &Build,
    plan: &BuildExecutionPlan,
) -> anyhow::Result<ReleasePublishOutcome> {
    let bundle = prepare_release_bundle(config, workspace, release, build, plan).await?;

    Ok(ReleasePublishOutcome {
        artifacts: ReleaseArtifactBundle {
            container_image: None,
            server_artifact_url: Some(bundle.server_artifact_url),
            admin_artifact_url: bundle.admin_artifact_url,
            storefront_artifact_url: bundle.storefront_artifact_url,
        },
        state: ReleasePublishState::Deploying,
    })
}

async fn prepare_release_bundle(
    config: &BuildDeploymentSettings,
    workspace: &DeploymentWorkspace,
    release: &Release,
    build: &Build,
    plan: &BuildExecutionPlan,
) -> anyhow::Result<PreparedReleaseBundle> {
    let root_dir = resolve_artifact_root(workspace, &config.filesystem_root_dir);
    let release_dir = root_dir.join(&release.id);
    let artifacts_dir = release_dir.join("artifacts");

    tokio::fs::create_dir_all(&artifacts_dir)
        .await
        .with_context(|| {
            format!(
                "failed to create release artifact dir {}",
                artifacts_dir.display()
            )
        })?;

    let source_binary = compiled_binary_path(workspace, plan);
    if !tokio::fs::try_exists(&source_binary).await.unwrap_or(false) {
        bail!(
            "compiled server artifact not found for release {}: {}",
            release.id,
            source_binary.display()
        );
    }

    let binary_name = source_binary
        .file_name()
        .ok_or_else(|| anyhow!("compiled artifact path has no file name"))?
        .to_owned();
    let published_binary = artifacts_dir.join(&binary_name);
    tokio::fs::copy(&source_binary, &published_binary)
        .await
        .with_context(|| {
            format!(
                "failed to copy server artifact to {}",
                published_binary.display()
            )
        })?;

    let (admin_artifact_url, admin_artifact_path) = publish_frontend_artifact(
        &artifacts_dir,
        workspace,
        config.public_base_url.as_deref(),
        plan.admin_build.as_ref(),
    )
    .await?;
    let (storefront_artifact_url, storefront_artifact_path) = publish_frontend_artifact(
        &artifacts_dir,
        workspace,
        config.public_base_url.as_deref(),
        plan.storefront_build.as_ref(),
    )
    .await?;

    let bundle_manifest = ReleaseBundleManifest {
        release_id: release.id.clone(),
        build_id: build.id,
        environment: release.environment.clone(),
        manifest_hash: release.manifest_hash.clone(),
        generated_at: Utc::now().to_rfc3339(),
        runtime_mode: plan.runtime_mode,
        cargo_command: plan.cargo_command.clone(),
        cargo_target: plan.cargo_target.clone(),
        cargo_profile: plan.cargo_profile.clone(),
        cargo_features: plan.cargo_features.clone(),
        modules: serde_json::from_value(release.modules.clone()).unwrap_or_default(),
        server_artifact_path: published_binary.to_string_lossy().to_string(),
        admin_artifact_path,
        storefront_artifact_path,
    };
    let bundle_path = release_dir.join("release-bundle.json");
    let bundle_payload = serde_json::to_vec_pretty(&bundle_manifest)
        .map_err(|error| anyhow!("failed to serialize release bundle manifest: {error}"))?;
    tokio::fs::write(&bundle_path, bundle_payload)
        .await
        .with_context(|| {
            format!(
                "failed to write release bundle manifest {}",
                bundle_path.display()
            )
        })?;

    let server_artifact_url =
        externalized_path(&published_binary, config.public_base_url.as_deref());

    Ok(PreparedReleaseBundle {
        release_dir,
        server_artifact_url,
        admin_artifact_url,
        storefront_artifact_url,
    })
}

async fn publish_release_to_container(
    config: &BuildDeploymentSettings,
    workspace: &DeploymentWorkspace,
    release: &Release,
    build: &Build,
    plan: &BuildExecutionPlan,
    activate: bool,
) -> anyhow::Result<ReleasePublishOutcome> {
    let bundle = prepare_release_bundle(config, workspace, release, build, plan).await?;
    let runtime_binary_name = container_runtime_binary_name(plan)?;
    let image = container_image_reference(config, release)?;
    let migration_source = workspace.migration_dir().ok_or_else(|| {
        anyhow!("container deployment requires an explicit migration runtime asset directory")
    })?;
    let config_source = workspace.config_dir().ok_or_else(|| {
        anyhow!("container deployment requires an explicit config runtime asset directory")
    })?;
    let migration_target = bundle.release_dir.join("migration");
    let config_target = bundle.release_dir.join("config");
    let dockerfile_path = bundle.release_dir.join("Dockerfile.container");

    copy_directory_recursive(migration_source, &migration_target).await?;
    copy_directory_recursive(config_source, &config_target).await?;

    tokio::fs::write(
        &dockerfile_path,
        container_runtime_dockerfile(&runtime_binary_name, plan.runtime_mode),
    )
    .await
    .with_context(|| {
        format!(
            "failed to write container Dockerfile {}",
            dockerfile_path.display()
        )
    })?;

    run_command(
        &config.docker_bin,
        &[
            "build".to_string(),
            "-f".to_string(),
            dockerfile_path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .ok_or_else(|| anyhow!("container Dockerfile path has no file name"))?,
            "-t".to_string(),
            image.clone(),
            ".".to_string(),
        ],
        &bundle.release_dir,
    )
    .await
    .with_context(|| format!("failed to build container image {image}"))?;

    run_command(
        &config.docker_bin,
        &["push".to_string(), image.clone()],
        &bundle.release_dir,
    )
    .await
    .with_context(|| format!("failed to push container image {image}"))?;

    let state = if activate {
        if let Some(rollout_command) = config.rollout_command.as_deref() {
            let command = render_rollout_command(
                rollout_command,
                &image,
                &release.id,
                &release.environment,
                plan.runtime_mode,
                &bundle.release_dir,
            );
            run_rollout_command(
                &command,
                &bundle.release_dir,
                &image,
                release,
                plan.runtime_mode,
            )
            .await?;
            ReleasePublishState::Active
        } else {
            ReleasePublishState::Deploying
        }
    } else {
        ReleasePublishState::Deploying
    };

    Ok(ReleasePublishOutcome {
        artifacts: ReleaseArtifactBundle {
            container_image: Some(image),
            server_artifact_url: Some(bundle.server_artifact_url),
            admin_artifact_url: bundle.admin_artifact_url,
            storefront_artifact_url: bundle.storefront_artifact_url,
        },
        state,
    })
}

#[derive(Debug, Serialize)]
struct ReleaseBundleManifest {
    release_id: String,
    build_id: uuid::Uuid,
    environment: String,
    manifest_hash: String,
    generated_at: String,
    runtime_mode: rustok_build::BuildRuntimeMode,
    cargo_command: String,
    cargo_target: Option<String>,
    cargo_profile: String,
    cargo_features: Vec<String>,
    modules: Vec<String>,
    server_artifact_path: String,
    admin_artifact_path: Option<String>,
    storefront_artifact_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct RemoteReleasePublishRequest {
    release_id: String,
    build_id: uuid::Uuid,
    environment: String,
    manifest_hash: String,
    generated_at: String,
    runtime_mode: rustok_build::BuildRuntimeMode,
    cargo_command: String,
    cargo_target: Option<String>,
    cargo_profile: String,
    cargo_features: Vec<String>,
    modules: Vec<String>,
    binary_file_name: String,
}

#[derive(Debug, serde::Deserialize)]
struct RemoteReleasePublishResponse {
    #[serde(default)]
    deployment_status: Option<String>,
    #[serde(default)]
    container_image: Option<String>,
    #[serde(default)]
    server_artifact_url: Option<String>,
    #[serde(default)]
    admin_artifact_url: Option<String>,
    #[serde(default)]
    storefront_artifact_url: Option<String>,
}

fn resolve_artifact_root(workspace: &DeploymentWorkspace, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        workspace.root().join(path)
    }
}

fn server_deployment_workspace() -> DeploymentWorkspace {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .map(|root| {
            let app_server_root = root.join("apps").join("server");
            DeploymentWorkspace::new(root).with_runtime_assets(
                app_server_root.join("migration"),
                app_server_root.join("config"),
            )
        })
        .expect("workspace root should be resolvable from apps/server")
}

fn compiled_binary_path(workspace: &DeploymentWorkspace, plan: &BuildExecutionPlan) -> PathBuf {
    let mut path = workspace.root().join("target");
    if let Some(target) = &plan.cargo_target {
        path.push(target);
    }
    path.push(binary_output_dir_name(&plan.cargo_profile));
    path.push(binary_file_name(
        &plan.cargo_package,
        plan.cargo_target.as_deref(),
    ));
    path
}

async fn publish_frontend_artifact(
    artifacts_dir: &Path,
    workspace: &DeploymentWorkspace,
    public_base_url: Option<&str>,
    plan: Option<&FrontendBuildPlan>,
) -> anyhow::Result<(Option<String>, Option<String>)> {
    let Some(plan) = plan else {
        return Ok((None, None));
    };

    let source_path = workspace.root().join(&plan.artifact_path);
    if !tokio::fs::try_exists(&source_path).await.unwrap_or(false) {
        bail!(
            "compiled {} artifact not found: {}",
            plan.surface,
            source_path.display()
        );
    }

    let destination_root = artifacts_dir.join(&plan.surface);
    match plan.artifact_kind {
        FrontendArtifactKind::Directory => {
            copy_directory_recursive(&source_path, &destination_root).await?;
            let entry_path = destination_root.join("index.html");
            if !tokio::fs::try_exists(&entry_path).await.unwrap_or(false) {
                bail!(
                    "{} artifact directory does not contain index.html: {}",
                    plan.surface,
                    destination_root.display()
                );
            }

            Ok((
                Some(externalized_path(&entry_path, public_base_url)),
                Some(destination_root.to_string_lossy().to_string()),
            ))
        }
        FrontendArtifactKind::File => {
            tokio::fs::create_dir_all(&destination_root)
                .await
                .with_context(|| {
                    format!(
                        "failed to create destination directory {}",
                        destination_root.display()
                    )
                })?;

            let file_name = source_path
                .file_name()
                .ok_or_else(|| anyhow!("compiled artifact path has no file name"))?;
            let destination_path = destination_root.join(file_name);
            tokio::fs::copy(&source_path, &destination_path)
                .await
                .with_context(|| {
                    format!(
                        "failed to copy {} artifact to {}",
                        plan.surface,
                        destination_path.display()
                    )
                })?;

            Ok((
                Some(externalized_path(&destination_path, public_base_url)),
                Some(destination_path.to_string_lossy().to_string()),
            ))
        }
    }
}

fn binary_output_dir_name(profile: &str) -> &str {
    if profile == "release" {
        "release"
    } else {
        profile
    }
}

fn binary_file_name(package: &str, cargo_target: Option<&str>) -> String {
    let exe_suffix = executable_suffix(cargo_target);
    if exe_suffix.is_empty() {
        package.to_string()
    } else {
        format!("{package}.{exe_suffix}")
    }
}

fn executable_suffix(cargo_target: Option<&str>) -> &'static str {
    match cargo_target {
        Some(target) if target.contains("windows") => "exe",
        Some(_) => "",
        None => std::env::consts::EXE_EXTENSION,
    }
}

fn externalized_path(path: &Path, public_base_url: Option<&str>) -> String {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    match public_base_url {
        Some(base) => {
            let release_id = path
                .parent()
                .and_then(Path::parent)
                .and_then(|dir| dir.file_name())
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default();
            format!("{base}/{release_id}/artifacts/{file_name}")
        }
        None => path.to_string_lossy().to_string(),
    }
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

async fn publish_release_to_http(
    config: &BuildDeploymentSettings,
    workspace: &DeploymentWorkspace,
    release: &Release,
    _build: &Build,
    plan: &BuildExecutionPlan,
) -> anyhow::Result<ReleasePublishOutcome> {
    let endpoint_url = config
        .endpoint_url
        .as_deref()
        .ok_or_else(|| anyhow!("http deployment backend requires endpoint_url"))?;
    let binary_path = compiled_binary_path(workspace, plan);
    if !tokio::fs::try_exists(&binary_path).await.unwrap_or(false) {
        bail!(
            "compiled server artifact not found for release {}: {}",
            release.id,
            binary_path.display()
        );
    }

    let binary_name = binary_path
        .file_name()
        .ok_or_else(|| anyhow!("compiled artifact path has no file name"))?
        .to_string_lossy()
        .to_string();
    let binary_bytes = tokio::fs::read(&binary_path)
        .await
        .with_context(|| format!("failed to read compiled artifact {}", binary_path.display()))?;

    let payload = RemoteReleasePublishRequest {
        release_id: release.id.clone(),
        build_id: release.build_id,
        environment: release.environment.clone(),
        manifest_hash: release.manifest_hash.clone(),
        generated_at: Utc::now().to_rfc3339(),
        runtime_mode: plan.runtime_mode,
        cargo_command: plan.cargo_command.clone(),
        cargo_target: plan.cargo_target.clone(),
        cargo_profile: plan.cargo_profile.clone(),
        cargo_features: plan.cargo_features.clone(),
        modules: serde_json::from_value(release.modules.clone()).unwrap_or_default(),
        binary_file_name: binary_name.clone(),
    };

    let metadata_json = serde_json::to_string(&payload)
        .map_err(|error| anyhow!("failed to serialize remote release metadata: {error}"))?;
    let binary_part = multipart::Part::bytes(binary_bytes)
        .file_name(binary_name)
        .mime_str("application/octet-stream")
        .map_err(|error| anyhow!("failed to construct remote release artifact part: {error}"))?;
    let form = multipart::Form::new()
        .text("metadata", metadata_json)
        .part("server_artifact", binary_part);

    let client = reqwest::Client::new();
    let mut request = client.post(endpoint_url).multipart(form);
    if let Some(token) = config.bearer_token.as_deref() {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("failed to call remote deployment endpoint {endpoint_url}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string());
        bail!("remote deployment endpoint returned {status}: {body}");
    }

    let body = response.text().await.with_context(|| {
        format!("failed to read remote deployment response from {endpoint_url}")
    })?;
    if body.trim().is_empty() {
        return Ok(ReleasePublishOutcome {
            artifacts: ReleaseArtifactBundle::default(),
            state: ReleasePublishState::Deploying,
        });
    }

    let published: RemoteReleasePublishResponse =
        serde_json::from_str(&body).with_context(|| {
            format!("failed to parse remote deployment response from {endpoint_url}")
        })?;

    Ok(ReleasePublishOutcome {
        artifacts: ReleaseArtifactBundle {
            container_image: published.container_image,
            server_artifact_url: published.server_artifact_url,
            admin_artifact_url: published.admin_artifact_url,
            storefront_artifact_url: published.storefront_artifact_url,
        },
        state: parse_remote_publish_state(published.deployment_status.as_deref())?,
    })
}

fn container_runtime_binary_name(plan: &BuildExecutionPlan) -> anyhow::Result<String> {
    let binary_name = binary_file_name(&plan.cargo_package, plan.cargo_target.as_deref());
    if binary_name.ends_with(".exe") {
        bail!(
            "container deployment backend requires a linux server artifact, but build plan resolves to {binary_name}"
        );
    }
    Ok(binary_name)
}

fn container_image_reference(
    config: &BuildDeploymentSettings,
    release: &Release,
) -> anyhow::Result<String> {
    let repository = config
        .image_repository
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("container deployment backend requires image_repository"))?;
    let tag = format!(
        "{}-{}",
        sanitize_image_tag_component(&release.environment),
        sanitize_image_tag_component(&release.id)
    );
    Ok(format!("{}:{}", repository.trim_end_matches(':'), tag))
}

fn sanitize_image_tag_component(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.' | '-' => ch,
            _ => '-',
        })
        .collect::<String>();

    if sanitized.is_empty() {
        "release".to_string()
    } else {
        sanitized
    }
}

fn container_runtime_dockerfile(
    binary_name: &str,
    runtime_mode: rustok_build::BuildRuntimeMode,
) -> String {
    format!(
        "FROM debian:bookworm-slim AS runtime\n\
WORKDIR /app\n\
RUN apt-get update && apt-get install -y \\\n\
    ca-certificates \\\n\
    libssl3 \\\n\
    postgresql-client \\\n\
    && rm -rf /var/lib/apt/lists/*\n\
COPY artifacts/{binary_name} /app/{binary_name}\n\
COPY migration ./migration\n\
COPY config ./config\n\
ENV RUSTOK_RUNTIME_HOST_MODE={runtime_mode}\n\
EXPOSE 5150\n\
CMD [\"./{binary_name}\", \"start\"]\n",
        runtime_mode = runtime_mode.as_str(),
    )
}

fn render_rollout_command(
    template: &str,
    image: &str,
    release_id: &str,
    environment: &str,
    runtime_mode: rustok_build::BuildRuntimeMode,
    bundle_dir: &Path,
) -> String {
    template
        .replace("{image}", image)
        .replace("{release_id}", release_id)
        .replace("{environment}", environment)
        .replace("{runtime_mode}", runtime_mode.as_str())
        .replace("{bundle_dir}", &bundle_dir.to_string_lossy())
}

async fn run_rollout_command(
    command: &str,
    current_dir: &Path,
    image: &str,
    release: &Release,
    runtime_mode: rustok_build::BuildRuntimeMode,
) -> anyhow::Result<()> {
    let mut process = if cfg!(windows) {
        let mut process = Command::new("cmd");
        process.arg("/C").arg(command);
        process
    } else {
        let mut process = Command::new("sh");
        process.arg("-c").arg(command);
        process
    };

    let status = process
        .current_dir(current_dir)
        .env("RUSTOK_RELEASE_ID", &release.id)
        .env("RUSTOK_RELEASE_ENVIRONMENT", &release.environment)
        .env("RUSTOK_RUNTIME_HOST_MODE", runtime_mode.as_str())
        .env("RUSTOK_CONTAINER_IMAGE", image)
        .env(
            "RUSTOK_RELEASE_BUNDLE_DIR",
            current_dir.to_string_lossy().to_string(),
        )
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .with_context(|| format!("failed to spawn rollout command '{command}'"))?;

    if !status.success() {
        let exit_code = status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string());
        bail!("rollout command failed with exit status {exit_code}: {command}");
    }

    Ok(())
}

async fn run_command(program: &str, args: &[String], current_dir: &Path) -> anyhow::Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .with_context(|| format!("failed to spawn {} {}", program, args.join(" ")))?;

    if !status.success() {
        let exit_code = status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string());
        bail!(
            "command failed with exit status {exit_code}: {} {}",
            program,
            args.join(" ")
        );
    }

    Ok(())
}

async fn copy_directory_recursive(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if !tokio::fs::try_exists(source).await.unwrap_or(false) {
        bail!("source directory does not exist: {}", source.display());
    }

    tokio::fs::create_dir_all(destination)
        .await
        .with_context(|| {
            format!(
                "failed to create destination directory {}",
                destination.display()
            )
        })?;

    let mut pending = vec![(source.to_path_buf(), destination.to_path_buf())];
    while let Some((src_dir, dst_dir)) = pending.pop() {
        tokio::fs::create_dir_all(&dst_dir).await.with_context(|| {
            format!(
                "failed to create destination directory {}",
                dst_dir.display()
            )
        })?;

        let mut entries = tokio::fs::read_dir(&src_dir)
            .await
            .with_context(|| format!("failed to read source directory {}", src_dir.display()))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .with_context(|| format!("failed to iterate source directory {}", src_dir.display()))?
        {
            let entry_type = entry.file_type().await.with_context(|| {
                format!("failed to inspect source entry {}", entry.path().display())
            })?;
            let dst_path = dst_dir.join(entry.file_name());
            if entry_type.is_dir() {
                pending.push((entry.path(), dst_path));
            } else if entry_type.is_file() {
                tokio::fs::copy(entry.path(), &dst_path)
                    .await
                    .with_context(|| {
                        format!("failed to copy runtime asset to {}", dst_path.display())
                    })?;
            }
        }
    }

    Ok(())
}

fn parse_remote_publish_state(status: Option<&str>) -> anyhow::Result<ReleasePublishState> {
    let Some(status) = status.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(ReleasePublishState::Deploying);
    };

    match status.to_ascii_lowercase().as_str() {
        "pending" | "queued" | "accepted" | "deploying" | "in_progress" => {
            Ok(ReleasePublishState::Deploying)
        }
        "active" | "deployed" | "success" => Ok(ReleasePublishState::Active),
        "failed" | "error" => Ok(ReleasePublishState::Failed),
        _ => bail!("unsupported remote deployment_status '{status}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ReleasePublishState, RemoteReleasePublishRequest, binary_file_name, compiled_binary_path,
        container_image_reference, container_runtime_binary_name, container_runtime_dockerfile,
        externalized_path, parse_remote_publish_state, render_rollout_command,
        resolve_artifact_root, sanitize_image_tag_component, server_deployment_workspace,
    };
    use crate::common::settings::BuildDeploymentSettings;
    use rustok_build::release::Model as Release;
    use rustok_build::{BuildExecutionPlan, BuildRuntimeMode};
    use std::path::{Path, PathBuf};

    #[test]
    fn resolves_relative_artifact_root_inside_workspace() {
        let workspace = server_deployment_workspace();
        let root = resolve_artifact_root(&workspace, "artifacts/releases");
        assert!(root.ends_with(PathBuf::from("artifacts").join("releases")));
        assert!(root.is_absolute());
    }

    #[test]
    fn derives_binary_path_from_plan() {
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

        let workspace = server_deployment_workspace();
        let path = compiled_binary_path(&workspace, &plan);
        assert!(
            path.to_string_lossy()
                .contains("target\\x86_64-unknown-linux-gnu\\release")
                || path
                    .to_string_lossy()
                    .contains("target/x86_64-unknown-linux-gnu/release")
        );
        assert!(path.ends_with(binary_file_name(
            "rustok-server",
            Some("x86_64-unknown-linux-gnu")
        )));
    }

    #[test]
    fn externalized_path_uses_public_base_url_when_present() {
        let path = PathBuf::from("C:/repo/artifacts/releases/rel_1/artifacts/rustok-server.exe");
        let url = externalized_path(&path, Some("https://artifacts.example.com/releases"));
        assert_eq!(
            url,
            "https://artifacts.example.com/releases/rel_1/artifacts/rustok-server.exe"
        );
    }

    #[test]
    fn remote_publish_request_serializes_expected_fields() {
        let payload = RemoteReleasePublishRequest {
            release_id: "rel_1".to_string(),
            build_id: uuid::Uuid::nil(),
            environment: "prod".to_string(),
            manifest_hash: "hash".to_string(),
            generated_at: "2026-03-14T00:00:00Z".to_string(),
            runtime_mode: BuildRuntimeMode::AdminSsr,
            cargo_command: "cargo build -p rustok-server --release".to_string(),
            cargo_target: Some("x86_64-unknown-linux-gnu".to_string()),
            cargo_profile: "release".to_string(),
            cargo_features: vec!["embed-admin".to_string()],
            modules: vec!["blog".to_string(), "pages".to_string()],
            binary_file_name: "rustok-server.exe".to_string(),
        };

        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(json["release_id"], "rel_1");
        assert_eq!(json["environment"], "prod");
        assert_eq!(json["binary_file_name"], "rustok-server.exe");
        assert_eq!(json["runtime_mode"], "admin_ssr");
        assert_eq!(json["cargo_features"][0], "embed-admin");
    }

    #[test]
    fn binary_file_name_tracks_target_platform() {
        assert_eq!(
            binary_file_name("rustok-server", Some("x86_64-pc-windows-msvc")),
            "rustok-server.exe"
        );
        assert_eq!(
            binary_file_name("rustok-server", Some("x86_64-unknown-linux-gnu")),
            "rustok-server"
        );
    }

    #[test]
    fn container_backend_requires_non_windows_binary() {
        let plan = BuildExecutionPlan {
            runtime_mode: BuildRuntimeMode::Full,
            cargo_package: "rustok-server".to_string(),
            cargo_profile: "release".to_string(),
            cargo_target: Some("x86_64-pc-windows-msvc".to_string()),
            cargo_features: vec![],
            cargo_command: String::new(),
            admin_build: None,
            storefront_build: None,
        };

        let error = container_runtime_binary_name(&plan).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("container deployment backend requires a linux server artifact")
        );
    }

    #[test]
    fn container_dockerfile_sets_the_selected_runtime_mode() {
        let dockerfile = container_runtime_dockerfile("rustok-server", BuildRuntimeMode::Worker);

        assert!(dockerfile.contains("ENV RUSTOK_RUNTIME_HOST_MODE=worker"));
    }

    #[test]
    fn container_image_reference_uses_environment_and_release_id() {
        let config = BuildDeploymentSettings {
            image_repository: Some("registry.example.com/rustok/server".to_string()),
            ..BuildDeploymentSettings::default()
        };
        let release = Release::new(
            uuid::Uuid::nil(),
            "prod eu".to_string(),
            "hash".to_string(),
            1,
            serde_json::json!({}),
            vec!["blog".to_string()],
        );

        let image = container_image_reference(&config, &release).unwrap();
        assert!(image.starts_with("registry.example.com/rustok/server:prod-eu-rel_"));
    }

    #[test]
    fn rollout_command_renders_placeholders() {
        let rendered = render_rollout_command(
            "./deploy.sh {image} {release_id} {environment} {runtime_mode} {bundle_dir}",
            "registry.example.com/rustok/server:prod-rel_1",
            "rel_1",
            "prod",
            BuildRuntimeMode::Worker,
            Path::new("/tmp/release"),
        );

        assert_eq!(
            rendered,
            "./deploy.sh registry.example.com/rustok/server:prod-rel_1 rel_1 prod worker /tmp/release"
        );
    }

    #[test]
    fn sanitize_image_tag_component_replaces_invalid_characters() {
        assert_eq!(sanitize_image_tag_component("prod/eu"), "prod-eu");
        assert_eq!(sanitize_image_tag_component(" qa env "), "qa-env");
    }

    #[test]
    fn remote_publish_state_defaults_to_deploying() {
        assert_eq!(
            parse_remote_publish_state(None).unwrap(),
            ReleasePublishState::Deploying
        );
        assert_eq!(
            parse_remote_publish_state(Some(" accepted ")).unwrap(),
            ReleasePublishState::Deploying
        );
    }

    #[test]
    fn remote_publish_state_maps_active_and_failed_statuses() {
        assert_eq!(
            parse_remote_publish_state(Some("deployed")).unwrap(),
            ReleasePublishState::Active
        );
        assert_eq!(
            parse_remote_publish_state(Some("failed")).unwrap(),
            ReleasePublishState::Failed
        );
    }

    #[test]
    fn remote_publish_state_rejects_unknown_status() {
        let error = parse_remote_publish_state(Some("mystery")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported remote deployment_status 'mystery'")
        );
    }
}
