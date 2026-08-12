//! Owner-local authoring commands for standalone module components.

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use rustok_build_source::{
    ArchiveLimits, CasArchiveError, SourceArchiveBuilder, SourceArchiveError,
    SourceArchiveInspector,
};
use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_module_template::{
    ModuleTemplateInput, RUST_TOOLCHAIN, RenderedModule, TEMPLATE_VERSION, render,
};
use rustok_modules::{
    MODULE_ARTIFACT_SOURCE_MANIFEST_FILE, MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE,
    MODULE_BUILD_COMPONENT_TARGET, MODULE_BUILD_RUNTIME_ABI, MODULE_BUILD_WIT_VERSION,
    MODULE_BUILD_WIT_WORLD, ModuleArtifactSourceManifest, ModuleAuthoringBuildCommand,
    ModuleAuthoringPublishCommand, ModuleCommandContext, ModulePublishBundleFiles,
    SeaOrmModuleAuthoringBuildService, SeaOrmModuleAuthoringPublishService,
    SharedModuleAuthoringBuildControl, SharedModuleAuthoringPublishControl,
    build_module_publish_bundle,
};
use rustok_runtime::{RuntimeComposition, db_clone};
use rustok_sandbox::{
    ExecutionPhase, LocalSandboxHarness, LocalSandboxScenario, LocalSandboxScenarioOutcome,
    SandboxContext, SandboxExecutorKind, SandboxPayload, SandboxRequest, SandboxSubject,
};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use uuid::Uuid;

const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_CARGO_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_LOCKFILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_AUXILIARY_FILE_BYTES: u64 = 1024 * 1024;
const MAX_LOCK_PACKAGES: usize = 4_096;
const MAX_SOURCE_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SOURCE_ENTRIES: u32 = 65_536;
const MAX_LOCAL_SCENARIO_BYTES: u64 = 512 * 1024;
const MAX_LOCAL_COMPONENT_BYTES: u64 = 128 * 1024 * 1024;
const MAX_LOCAL_CARGO_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
const LOCAL_CARGO_COMMAND_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const DEFAULT_LOCAL_SCENARIO: &str = "tests/sandbox-scenario.json";
const FINAL_DESCRIPTOR_FILE: &str = "module-artifact-descriptor.json";

pub struct ModuleCommandProvider {
    runtime: RuntimeComposition,
}

impl ModuleCommandProvider {
    fn build_control(&self) -> CliCoreResult<SharedModuleAuthoringBuildControl> {
        let host = self.runtime.require_host().map_err(|error| {
            command_failed(format!("module build requires owner runtime: {error}"))
        })?;
        if let Some(shared) = host.shared_get::<SharedModuleAuthoringBuildControl>() {
            return Ok(shared);
        }
        let source_cas_root = self
            .runtime
            .instance_path("sources")
            .map_err(command_failed)?;
        let service = SeaOrmModuleAuthoringBuildService::new(db_clone(host), source_cas_root)
            .map_err(command_failed)?;
        let shared = SharedModuleAuthoringBuildControl(Arc::new(service));
        Ok(shared)
    }

    async fn publish_control(&self) -> CliCoreResult<SharedModuleAuthoringPublishControl> {
        let host = self.runtime.require_host().map_err(|error| {
            command_failed(format!("module publish requires owner runtime: {error}"))
        })?;
        if let Some(shared) = host.shared_get::<SharedModuleAuthoringPublishControl>() {
            return Ok(shared);
        }
        let storage_settings =
            self.runtime
                .settings()
                .get("storage")
                .cloned()
                .ok_or_else(|| {
                    command_failed("module publish requires storage in RUSTOK_SETTINGS_JSON")
                })?;
        let storage_root = self
            .runtime
            .instance_path("storage")
            .map_err(command_failed)?;
        let service = SeaOrmModuleAuthoringPublishService::from_storage_settings(
            db_clone(host),
            storage_settings,
            &storage_root,
        )
        .await
        .map_err(command_failed)?;
        let shared = SharedModuleAuthoringPublishControl(Arc::new(service));
        Ok(shared)
    }
}

#[async_trait::async_trait]
impl CommandProvider for ModuleCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new(
                "module",
                "init",
                "Create a locked standalone Rust module component",
            )
            .with_dry_run(),
            CommandDescriptor::new(
                "module",
                "validate",
                "Validate a standalone Rust module source tree",
            ),
            CommandDescriptor::new(
                "module",
                "test",
                "Build and execute a module through the local sandbox contract",
            )
            .with_dry_run(),
            CommandDescriptor::new(
                "module",
                "build",
                "Queue a trusted remote build through the module owner",
            )
            .with_dry_run(),
            CommandDescriptor::new(
                "module",
                "package",
                "Create a deterministic digest-addressed source archive",
            )
            .with_dry_run(),
            CommandDescriptor::new(
                "module",
                "publish",
                "Stage a completed remote build for governed publication",
            )
            .with_dry_run(),
            CommandDescriptor::new(
                "module",
                "inspect",
                "Inspect a module source project or strict source archive",
            ),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("module", "init") => init_project(request).await,
            ("module", "validate") => validate_project_command(request),
            ("module", "test") => test_project(request).await,
            ("module", "build") => self.build_project(request).await,
            ("module", "package") => package_project(request),
            ("module", "publish") => self.publish_project(request).await,
            ("module", "inspect") => inspect_path(request),
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(ModuleCommandProvider {
        runtime: runtime.clone(),
    })
}

impl ModuleCommandProvider {
    async fn build_project(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        let args = NormalizedArgs::parse(&request.args)?;
        args.reject_unknown_options(&[
            "tenant_id",
            "actor_id",
            "project_id",
            "trace_id",
            "correlation_id",
            "idempotency_key",
        ])?;
        let source = args.one_positional("module build")?;
        let validation = validate_project(Path::new(source))?;
        let context = ModuleCommandContext {
            actor_id: args.required_option("actor_id")?.to_string(),
            tenant_id: Some(parse_uuid_option(&args, "tenant_id")?),
            trace_id: args.required_option("trace_id")?.to_string(),
            correlation_id: args.required_option("correlation_id")?.to_string(),
            idempotency_key: args.required_option("idempotency_key")?.to_string(),
        };
        let project_id = args.required_option("project_id")?.to_string();

        if request.dry_run {
            context.validate().map_err(invalid_input)?;
            return Ok(
                CommandOutcome::success("Remote module build plan is valid").with_data(
                    serde_json::json!({
                        "kind": "remote_build_plan",
                        "project": validation.data(),
                        "project_id": project_id,
                        "tenant_id": context.tenant_id,
                        "actor_id": context.actor_id,
                        "trace_id": context.trace_id,
                        "correlation_id": context.correlation_id,
                        "idempotency_key": context.idempotency_key,
                        "source": "deterministic_temporary_archive",
                        "execution": "owner_queue_to_remote_isolated_worker"
                    }),
                ),
            );
        }

        let control = self.build_control()?;
        let temporary_root = reserve_build_archive_root()?;
        let archive_path = temporary_root.join("source.tar");
        let packaged = SourceArchiveBuilder::new(source_archive_limits()?)
            .write(&validation.path, &archive_path)
            .map_err(map_source_archive_error);
        let packaged = match packaged {
            Ok(receipt) => receipt,
            Err(error) => return cleanup_build_archive_error(&temporary_root, error),
        };
        let command = ModuleAuthoringBuildCommand {
            context,
            project_id,
            source_digest: packaged.source_digest.clone(),
            expected_module_slug: validation.slug.clone(),
            expected_version: validation.version.clone(),
            rust_toolchain: validation.rust_toolchain.clone(),
            sdk_version: validation.sdk_version.clone(),
            template_version: validation.template_version.clone(),
            dependency_lock_digest: validation.lock_digest.clone(),
        };
        let submitted = control.0.submit_build(command, archive_path).await;
        let cleanup = fs::remove_dir_all(&temporary_root);
        let submitted = match (submitted, cleanup) {
            (Ok(submitted), Ok(())) => submitted,
            (Err(error), Ok(())) => return Err(command_failed(error)),
            (Ok(_), Err(cleanup_error)) => {
                return Err(command_failed(format!(
                    "module build was queued but temporary source cleanup failed: {cleanup_error}"
                )));
            }
            (Err(error), Err(cleanup_error)) => {
                return Err(command_failed(format!(
                    "module build submission failed and temporary source cleanup also failed: {cleanup_error}; original error: {error}"
                )));
            }
        };

        Ok(CommandOutcome::success(if submitted.build_created {
            "Remote module build queued"
        } else {
            "Remote module build already queued"
        })
        .with_data(serde_json::json!({
            "kind": "remote_build_submission",
            "request_id": submitted.request_id,
            "build_created": submitted.build_created,
            "source_created": submitted.source_created,
            "source_reference": submitted.source_reference,
            "source_digest": submitted.source_digest,
            "archive_bytes": submitted.archive_bytes,
            "source_bytes": submitted.source_bytes,
            "entries": submitted.entries,
            "delivery": "transactional_outbox_to_remote_isolated_worker"
        })))
    }

    async fn publish_project(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        let args = NormalizedArgs::parse(&request.args)?;
        args.reject_unknown_options(&[
            "tenant_id",
            "actor_id",
            "build_request_id",
            "trace_id",
            "correlation_id",
            "idempotency_key",
            "name",
            "description",
            "license",
            "default_locale",
            "category",
            "tags",
        ])?;
        let source = args.one_positional("module publish")?;
        let validation = validate_project(Path::new(source))?;
        let context = ModuleCommandContext {
            actor_id: args.required_option("actor_id")?.to_string(),
            tenant_id: Some(parse_uuid_option(&args, "tenant_id")?),
            trace_id: args.required_option("trace_id")?.to_string(),
            correlation_id: args.required_option("correlation_id")?.to_string(),
            idempotency_key: parse_uuid_option(&args, "idempotency_key")?.to_string(),
        };
        let mut marketplace_tags = args
            .option("tags")
            .map(|tags| {
                tags.split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        marketplace_tags.sort();
        marketplace_tags.dedup();
        let command = ModuleAuthoringPublishCommand {
            context,
            build_request_id: parse_uuid_option(&args, "build_request_id")?,
            slug: validation.slug.clone(),
            version: validation.version.clone(),
            crate_name: validation.crate_name.clone(),
            default_locale: args.option("default_locale").unwrap_or("en").to_string(),
            name: args.required_option("name")?.to_string(),
            description: args.required_option("description")?.to_string(),
            license: args.required_option("license")?.to_string(),
            marketplace_category: args.option("category").map(ToString::to_string),
            marketplace_tags,
        };
        let source_manifest = String::from_utf8(read_regular_file(
            &validation.path,
            MODULE_ARTIFACT_SOURCE_MANIFEST_FILE,
            MAX_MANIFEST_BYTES,
        )?)
        .map_err(invalid_input)?;
        let crate_manifest = String::from_utf8(read_regular_file(
            &validation.path,
            "Cargo.toml",
            MAX_CARGO_MANIFEST_BYTES,
        )?)
        .map_err(invalid_input)?;
        let publish_contract = command.validation_contract().map_err(invalid_input)?;
        let bundle = build_module_publish_bundle(
            &publish_contract,
            ModulePublishBundleFiles {
                source_manifest,
                crate_manifest,
                admin_manifest: None,
                storefront_manifest: None,
            },
        )
        .map_err(|validation| {
            invalid_input(format!(
                "module publication bundle is invalid: {}",
                validation.errors.join("; ")
            ))
        })?;

        if request.dry_run {
            return Ok(
                CommandOutcome::success("Governed module publication plan is valid").with_data(
                    serde_json::json!({
                        "kind": "governed_publish_plan",
                        "project": validation.data(),
                        "build_request_id": command.build_request_id,
                        "tenant_id": command.context.tenant_id,
                        "actor_id": command.context.actor_id,
                        "bundle_bytes": bundle.len(),
                        "artifact_origin": "platform_built",
                        "ownership": publish_contract.ownership,
                        "trust_level": publish_contract.trust_level,
                        "execution": "owner_governance_review_staging"
                    }),
                ),
            );
        }

        let control = self.publish_control().await?;
        let submitted = control
            .0
            .submit_publish_request(command, bundle)
            .await
            .map_err(command_failed)?;
        Ok(CommandOutcome::success(if submitted.validation_queued {
            "Module publication review queued"
        } else {
            "Module publication review already queued"
        })
        .with_data(serde_json::json!({
            "kind": "governed_publish_submission",
            "request_id": submitted.request_id,
            "build_request_id": submitted.build_request_id,
            "staging_id": submitted.staging_id,
            "stage_created": submitted.stage_created,
            "validation_job_id": submitted.validation_job_id,
            "validation_queued": submitted.validation_queued,
            "bundle_storage_key": submitted.bundle_storage_key,
            "bundle_checksum_sha256": submitted.bundle_checksum_sha256,
            "bundle_bytes": submitted.bundle_bytes,
            "publication": "pending_governance_and_platform_admission"
        })))
    }
}

async fn init_project(request: CommandRequest) -> CliCoreResult<CommandOutcome> {
    let args = NormalizedArgs::parse(&request.args)?;
    args.reject_unknown_options(&["slug", "name", "version"])?;
    let raw_target = args.one_positional("module init")?;
    let slug = args.required_option("slug")?.to_string();
    let version = args.option("version").unwrap_or("0.1.0").to_string();
    let display_name = args
        .option("name")
        .map(str::to_string)
        .unwrap_or_else(|| default_display_name(&slug));
    let rendered = render(&ModuleTemplateInput {
        slug: slug.clone(),
        version: version.clone(),
        display_name,
    })
    .map_err(invalid_input)?;
    let target = resolve_new_target(raw_target)?;

    if request.dry_run {
        return Ok(
            CommandOutcome::success("Module project plan is valid").with_data(serde_json::json!({
                "path": target,
                "slug": slug,
                "version": version,
                "sdk_version": rustok_module_sdk::SDK_VERSION,
                "template_version": TEMPLATE_VERSION,
                "files": rendered.files().iter().map(|file| file.path).collect::<Vec<_>>(),
                "lockfile": "generated_by_pinned_cargo"
            })),
        );
    }

    fs::create_dir(&target).map_err(command_failed)?;
    let initialized = initialize_created_target(&target, &rendered).await;
    let report = match initialized {
        Ok(report) => report,
        Err(error) => {
            let cleanup = fs::remove_dir_all(&target);
            return match cleanup {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(CliCoreError::CommandFailed {
                    message: format!(
                        "module initialization failed and {} could not be removed: {cleanup_error}; original error: {error}",
                        target.display()
                    ),
                }),
            };
        }
    };
    Ok(report.outcome("Module project initialized"))
}

async fn initialize_created_target(
    target: &Path,
    rendered: &RenderedModule,
) -> CliCoreResult<ProjectValidation> {
    write_rendered_project(target, rendered)?;
    generate_lockfile(target).await?;
    validate_project(target)
}

fn validate_project_command(request: CommandRequest) -> CliCoreResult<CommandOutcome> {
    let args = NormalizedArgs::parse(&request.args)?;
    args.reject_unknown_options(&[])?;
    let target = args.one_positional("module validate")?;
    let report = validate_project(Path::new(target))?;
    Ok(report.outcome("Module project is valid"))
}

async fn test_project(request: CommandRequest) -> CliCoreResult<CommandOutcome> {
    let args = NormalizedArgs::parse(&request.args)?;
    args.reject_unknown_options(&["scenario"])?;
    let source = args.one_positional("module test")?;
    let validation = validate_project(Path::new(source))?;
    let scenario_relative = validate_project_relative_json_path(
        args.option("scenario").unwrap_or(DEFAULT_LOCAL_SCENARIO),
    )?;
    let scenario_bytes = read_regular_file(
        &validation.path,
        &scenario_relative,
        MAX_LOCAL_SCENARIO_BYTES,
    )?;
    let scenario = LocalSandboxScenario::parse(&scenario_bytes).map_err(invalid_input)?;
    validate_scenario_capabilities(&validation.capabilities, &scenario)?;
    let target_dir = validation.path.join("target/rustok-module-test");
    let cargo_stages = local_cargo_stages();

    if request.dry_run {
        return Ok(
            CommandOutcome::success("Module local sandbox test plan is valid").with_data(
                serde_json::json!({
                    "kind": "local_sandbox_test_plan",
                    "project": validation.data(),
                    "scenario": scenario_relative,
                    "target_dir": target_dir,
                    "cargo": cargo_stages.iter().map(|stage| serde_json::json!({
                        "stage": stage.name,
                        "arguments": &stage.arguments
                    })).collect::<Vec<_>>()
                }),
            ),
        );
    }

    prepare_local_target_directory(&validation.path, &target_dir)?;
    for stage in &cargo_stages {
        run_cargo_command(
            &validation.path,
            Some(&target_dir),
            stage.name,
            &stage.arguments,
            LOCAL_CARGO_COMMAND_TIMEOUT,
            true,
            &validation.rust_toolchain,
        )
        .await?;
    }

    let component_path = target_dir
        .join(MODULE_BUILD_COMPONENT_TARGET)
        .join("release")
        .join(format!("{}.wasm", validation.slug.replace('-', "_")));
    let component = read_regular_absolute_file(&component_path, MAX_LOCAL_COMPONENT_BYTES)?;
    let component_digest = format!("sha256:{}", hex::encode(Sha256::digest(&component)));
    let harness = LocalSandboxHarness::wasm_component().map_err(command_failed)?;
    scenario
        .configure(&harness.fixtures())
        .map_err(invalid_input)?;
    let context = SandboxContext::new(ExecutionPhase::Test);
    let sandbox_result = harness
        .execute(SandboxRequest {
            subject: SandboxSubject::ModuleArtifact {
                installation_id: context.execution_id,
                slug: validation.slug.clone(),
                version: validation.version.clone(),
                digest: component_digest.clone(),
            },
            context,
            payload: SandboxPayload {
                executor: SandboxExecutorKind::WasmComponent,
                media_type: MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE.to_string(),
                digest: component_digest.clone(),
                runtime_abi: MODULE_BUILD_RUNTIME_ABI.to_string(),
                entrypoint: validation.entrypoint.clone(),
                bytes: component,
            },
            input: scenario.input.clone(),
            rhai_scope: None,
            policy: scenario.policy.clone(),
        })
        .await;
    let evaluated = scenario.evaluate(sandbox_result).map_err(command_failed)?;
    let execution = match evaluated {
        LocalSandboxScenarioOutcome::Success(outcome) => serde_json::json!({
            "outcome": "success",
            "output": outcome.output,
            "metrics": outcome.metrics
        }),
        LocalSandboxScenarioOutcome::ExpectedError { code } => serde_json::json!({
            "outcome": "expected_error",
            "code": code
        }),
    };
    Ok(
        CommandOutcome::success("Module local sandbox test passed").with_data(serde_json::json!({
            "kind": "local_sandbox_test",
            "project": validation.data(),
            "scenario": scenario_relative,
            "component_path": component_path,
            "component_digest": component_digest,
            "cargo_stages": cargo_stages.iter().map(|stage| stage.name).collect::<Vec<_>>(),
            "execution": execution
        })),
    )
}

struct LocalCargoStage {
    name: &'static str,
    arguments: Vec<String>,
}

fn local_cargo_stages() -> Vec<LocalCargoStage> {
    vec![
        LocalCargoStage {
            name: "format",
            arguments: ["fmt", "--all", "--", "--check"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        },
        LocalCargoStage {
            name: "host_tests",
            arguments: ["test", "--locked", "--offline"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        },
        LocalCargoStage {
            name: "component_clippy",
            arguments: [
                "clippy",
                "--locked",
                "--offline",
                "--target",
                MODULE_BUILD_COMPONENT_TARGET,
                "--",
                "-D",
                "warnings",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        },
        LocalCargoStage {
            name: "component_build",
            arguments: [
                "build",
                "--locked",
                "--offline",
                "--release",
                "--target",
                MODULE_BUILD_COMPONENT_TARGET,
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        },
    ]
}

fn package_project(request: CommandRequest) -> CliCoreResult<CommandOutcome> {
    let args = NormalizedArgs::parse(&request.args)?;
    args.reject_unknown_options(&["output"])?;
    let source = args.one_positional("module package")?;
    let validation = validate_project(Path::new(source))?;
    let destination =
        resolve_new_archive_output(args.required_option("output")?, validation.path.as_path())?;
    let limits = source_archive_limits()?;

    if request.dry_run {
        return Ok(
            CommandOutcome::success("Module source archive plan is valid").with_data(
                serde_json::json!({
                    "kind": "source_archive_plan",
                    "project": validation.data(),
                    "output": destination,
                    "limits": {
                        "archive_bytes": limits.max_archive_bytes,
                        "source_bytes": limits.max_extracted_bytes,
                        "entries": limits.max_entries
                    }
                }),
            ),
        );
    }

    let receipt = SourceArchiveBuilder::new(limits)
        .write(&validation.path, &destination)
        .map_err(map_source_archive_error)?;
    let post_validation = match validate_project(&validation.path) {
        Ok(report) => report,
        Err(error) => return remove_failed_archive(&destination, error),
    };
    if post_validation != validation {
        return remove_failed_archive(
            &destination,
            invalid_input("module source identity changed while the archive was being created"),
        );
    }
    let source_reference = format!("cas://{}", receipt.source_digest);

    Ok(
        CommandOutcome::success("Module source archive created").with_data(serde_json::json!({
            "kind": "source_archive",
            "project": validation.data(),
            "path": destination,
            "source_digest": receipt.source_digest,
            "source_reference": source_reference,
            "archive_bytes": receipt.archive_bytes,
            "source_bytes": receipt.source_bytes,
            "entries": receipt.entries
        })),
    )
}

fn inspect_path(request: CommandRequest) -> CliCoreResult<CommandOutcome> {
    let args = NormalizedArgs::parse(&request.args)?;
    args.reject_unknown_options(&[])?;
    let path = Path::new(args.one_positional("module inspect")?);
    let metadata = fs::symlink_metadata(path).map_err(command_failed)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid_input("module inspect path must not be a symlink"));
    }
    if metadata.is_dir() {
        let report = validate_project(path)?;
        return Ok(
            CommandOutcome::success("Module source project is valid").with_data(
                serde_json::json!({
                    "kind": "source_project",
                    "project": report.data()
                }),
            ),
        );
    }
    if !metadata.is_file() {
        return Err(invalid_input(
            "module inspect path must be a regular project directory or source archive",
        ));
    }
    let archive = fs::canonicalize(path).map_err(command_failed)?;
    let inspection = SourceArchiveInspector::new(source_archive_limits()?)
        .inspect(&archive)
        .map_err(map_archive_error)?;
    let source_reference = format!("cas://{}", inspection.source_digest);
    Ok(
        CommandOutcome::success("Module source archive is valid").with_data(serde_json::json!({
            "kind": "source_archive",
            "path": archive,
            "source_digest": inspection.source_digest,
            "source_reference": source_reference,
            "archive_bytes": inspection.archive_bytes,
            "source_bytes": inspection.extracted_bytes,
            "entries": inspection.entries
        })),
    )
}

fn validate_project(path: &Path) -> CliCoreResult<ProjectValidation> {
    let metadata = fs::symlink_metadata(path).map_err(command_failed)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_input(
            "module project path must be a regular directory",
        ));
    }
    let root = fs::canonicalize(path).map_err(command_failed)?;
    reject_final_descriptor(&root)?;
    reject_source_cargo_config(&root)?;

    let source_bytes = read_regular_file(
        &root,
        MODULE_ARTIFACT_SOURCE_MANIFEST_FILE,
        MAX_MANIFEST_BYTES,
    )?;
    let source = ModuleArtifactSourceManifest::parse(&source_bytes).map_err(invalid_input)?;
    let cargo_bytes = read_regular_file(&root, "Cargo.toml", MAX_CARGO_MANIFEST_BYTES)?;
    let cargo: toml::Value =
        toml::from_str(std::str::from_utf8(&cargo_bytes).map_err(invalid_input)?)
            .map_err(invalid_input)?;
    let authoring = validate_cargo_manifest(&cargo, &source)?;

    let toolchain_bytes =
        read_regular_file(&root, "rust-toolchain.toml", MAX_AUXILIARY_FILE_BYTES)?;
    let toolchain: toml::Value =
        toml::from_str(std::str::from_utf8(&toolchain_bytes).map_err(invalid_input)?)
            .map_err(invalid_input)?;
    validate_toolchain(&toolchain, &authoring.rust_toolchain)?;

    let policy_bytes =
        read_regular_file(&root, "module-build-policy.toml", MAX_AUXILIARY_FILE_BYTES)?;
    let policy: ModuleBuildPolicy =
        toml::from_str(std::str::from_utf8(&policy_bytes).map_err(invalid_input)?)
            .map_err(invalid_input)?;
    policy.validate()?;

    for required in ["src/lib.rs", "tests/contract.rs", "locales/en.json"] {
        let bytes = read_regular_file(&root, required, MAX_AUXILIARY_FILE_BYTES)?;
        if required.ends_with(".json") {
            serde_json::from_slice::<serde_json::Value>(&bytes).map_err(invalid_input)?;
        }
    }
    let scenario_bytes =
        read_regular_file(&root, DEFAULT_LOCAL_SCENARIO, MAX_LOCAL_SCENARIO_BYTES)?;
    let scenario = LocalSandboxScenario::parse(&scenario_bytes).map_err(invalid_input)?;
    validate_scenario_capabilities(source.capabilities(), &scenario)?;

    let lock_bytes = read_regular_file(&root, "Cargo.lock", MAX_LOCKFILE_BYTES)?;
    validate_lockfile(&lock_bytes, &source, &authoring.sdk_version)?;
    let lock_digest = format!("sha256:{}", hex::encode(Sha256::digest(&lock_bytes)));
    Ok(ProjectValidation {
        path: root,
        slug: source.slug().to_string(),
        crate_name: source.slug().replace('_', "-"),
        version: source.version().to_string(),
        sdk_version: authoring.sdk_version,
        template_version: authoring.template_version,
        rust_toolchain: authoring.rust_toolchain,
        lock_digest,
        entrypoint: source.entrypoint().to_string(),
        capabilities: source.capabilities().to_vec(),
    })
}

fn validate_cargo_manifest(
    cargo: &toml::Value,
    source: &ModuleArtifactSourceManifest,
) -> CliCoreResult<AuthoringMetadata> {
    let package = cargo
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| invalid_input("Cargo.toml must contain [package]"))?;
    let expected_package_name = source.slug().replace('_', "-");
    for (key, expected) in [
        ("name", expected_package_name.as_str()),
        ("version", source.version()),
        ("edition", "2024"),
    ] {
        if package.get(key).and_then(toml::Value::as_str) != Some(expected) {
            return Err(invalid_input(format!(
                "Cargo.toml package.{key} must be `{expected}`"
            )));
        }
    }
    let rust_toolchain = package
        .get("rust-version")
        .and_then(toml::Value::as_str)
        .filter(|version| Version::parse(version).is_ok())
        .ok_or_else(|| invalid_input("Cargo.toml package.rust-version must be exact SemVer"))?
        .to_string();
    if package.contains_key("build") || package.contains_key("links") {
        return Err(invalid_input(
            "Cargo.toml cannot declare a build script or native links",
        ));
    }
    let crate_types = cargo
        .get("lib")
        .and_then(|lib| lib.get("crate-type"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| invalid_input("Cargo.toml must declare [lib] crate-type"))?;
    if crate_types.len() != 1 || crate_types[0].as_str() != Some("cdylib") {
        return Err(invalid_input("Cargo.toml must emit only a cdylib"));
    }
    let sdk = cargo
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("rustok-module-sdk"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| invalid_input("Cargo.toml must pin rustok-module-sdk"))?;
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(dependencies) = cargo.get(section).and_then(toml::Value::as_table) {
            validate_dependency_table(section, dependencies)?;
        }
    }
    if cargo.get("target").is_some() {
        return Err(invalid_input(
            "Cargo.toml target-specific dependency sections are forbidden",
        ));
    }
    if cargo.get("patch").is_some() || cargo.get("replace").is_some() {
        return Err(invalid_input(
            "Cargo.toml patch/replace sections are forbidden",
        ));
    }
    let metadata = package
        .get("metadata")
        .and_then(|metadata| metadata.get("rustok"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| invalid_input("Cargo.toml must contain [package.metadata.rustok]"))?;
    let sdk_version = metadata
        .get("sdk_version")
        .and_then(toml::Value::as_str)
        .filter(|version| Version::parse(version).is_ok())
        .ok_or_else(|| invalid_input("SDK provenance version must be exact SemVer"))?
        .to_string();
    let template_version = metadata
        .get("template_version")
        .and_then(toml::Value::as_str)
        .filter(|version| Version::parse(version).is_ok())
        .ok_or_else(|| invalid_input("template provenance version must be exact SemVer"))?
        .to_string();
    let expected_sdk = format!("={sdk_version}");
    if sdk.len() != 1
        || sdk.get("version").and_then(toml::Value::as_str) != Some(expected_sdk.as_str())
    {
        return Err(invalid_input(
            "rustok-module-sdk dependency must exactly match SDK provenance",
        ));
    }
    for (key, expected) in [
        ("runtime_abi", MODULE_BUILD_RUNTIME_ABI),
        ("wit_world", MODULE_BUILD_WIT_WORLD),
        ("wit_version", MODULE_BUILD_WIT_VERSION),
        ("component_target", MODULE_BUILD_COMPONENT_TARGET),
    ] {
        if metadata.get(key).and_then(toml::Value::as_str) != Some(expected) {
            return Err(invalid_input(format!(
                "Cargo.toml package.metadata.rustok.{key} must be `{expected}`"
            )));
        }
    }
    if source.runtime_abi() != MODULE_BUILD_RUNTIME_ABI {
        return Err(invalid_input("source manifest runtime ABI is not current"));
    }
    Ok(AuthoringMetadata {
        sdk_version,
        template_version,
        rust_toolchain,
    })
}

fn validate_dependency_table(
    section: &str,
    dependencies: &toml::map::Map<String, toml::Value>,
) -> CliCoreResult<()> {
    for (name, dependency) in dependencies {
        match dependency {
            toml::Value::String(version) if !version.trim().is_empty() => {}
            toml::Value::Table(specification)
                if specification
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|version| !version.trim().is_empty())
                    && !["path", "git", "registry", "workspace"]
                        .iter()
                        .any(|key| specification.contains_key(*key)) => {}
            _ => {
                return Err(invalid_input(format!(
                    "Cargo.toml {section}.{name} must be a crates.io version dependency"
                )));
            }
        }
    }
    Ok(())
}

fn validate_toolchain(toolchain: &toml::Value, rust_toolchain: &str) -> CliCoreResult<()> {
    let toolchain = toolchain
        .get("toolchain")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| invalid_input("rust-toolchain.toml must contain [toolchain]"))?;
    if toolchain.get("channel").and_then(toml::Value::as_str) != Some(rust_toolchain)
        || toolchain.get("profile").and_then(toml::Value::as_str) != Some("minimal")
    {
        return Err(invalid_input("Rust toolchain selection is not canonical"));
    }
    let targets = toolchain
        .get("targets")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| invalid_input("Rust toolchain target is missing"))?;
    if targets.len() != 1 || targets[0].as_str() != Some(MODULE_BUILD_COMPONENT_TARGET) {
        return Err(invalid_input("Rust toolchain target is not canonical"));
    }
    let components = toolchain
        .get("components")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| invalid_input("Rust toolchain components are missing"))?;
    if components.len() != 2
        || components[0].as_str() != Some("clippy")
        || components[1].as_str() != Some("rustfmt")
    {
        return Err(invalid_input("Rust toolchain components are not canonical"));
    }
    Ok(())
}

fn validate_lockfile(
    bytes: &[u8],
    source: &ModuleArtifactSourceManifest,
    sdk_version: &str,
) -> CliCoreResult<()> {
    let lock: toml::Value = toml::from_str(std::str::from_utf8(bytes).map_err(invalid_input)?)
        .map_err(invalid_input)?;
    if lock.get("version").and_then(toml::Value::as_integer) != Some(4) {
        return Err(invalid_input("Cargo.lock must use the current format"));
    }
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .filter(|packages| !packages.is_empty() && packages.len() <= MAX_LOCK_PACKAGES)
        .ok_or_else(|| invalid_input("Cargo.lock package graph is empty or oversized"))?;
    let expected_root_name = source.slug().replace('_', "-");
    let mut root_found = false;
    let mut sdk_found = false;
    let mut identities = BTreeSet::new();
    for package in packages {
        let package = package
            .as_table()
            .ok_or_else(|| invalid_input("Cargo.lock package entry is invalid"))?;
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| invalid_input("Cargo.lock package name is missing"))?;
        let version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| invalid_input("Cargo.lock package version is missing"))?;
        let identity = (name.to_string(), version.to_string());
        if !identities.insert(identity) {
            return Err(invalid_input(
                "Cargo.lock contains a duplicate package identity",
            ));
        }
        let registry_source = package.get("source").and_then(toml::Value::as_str);
        if name == expected_root_name && version == source.version() && registry_source.is_none() {
            root_found = true;
            continue;
        }
        if !registry_source
            .is_some_and(|source| source == "registry+https://github.com/rust-lang/crates.io-index")
            || !package
                .get("checksum")
                .and_then(toml::Value::as_str)
                .is_some_and(valid_checksum)
        {
            return Err(invalid_input(
                "Cargo.lock dependencies must be checksummed crates.io packages",
            ));
        }
        if name == "rustok-module-sdk" && version == sdk_version {
            sdk_found = true;
        }
    }
    if !root_found || !sdk_found {
        return Err(invalid_input(
            "Cargo.lock must bind the project and exact rustok-module-sdk release",
        ));
    }
    Ok(())
}

async fn generate_lockfile(root: &Path) -> CliCoreResult<()> {
    run_cargo_command(
        root,
        None,
        "lockfile",
        &["generate-lockfile".to_string(), "--quiet".to_string()],
        Duration::from_secs(5 * 60),
        false,
        RUST_TOOLCHAIN,
    )
    .await
}

async fn run_cargo_command(
    root: &Path,
    target_dir: Option<&Path>,
    stage: &str,
    arguments: &[String],
    timeout: Duration,
    offline: bool,
    rust_toolchain: &str,
) -> CliCoreResult<()> {
    let mut command = Command::new("cargo");
    command
        .args(arguments)
        .current_dir(root)
        .env_clear()
        .env("CARGO_TERM_COLOR", "never")
        .env("RUSTUP_TOOLCHAIN", rust_toolchain)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for name in [
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "WINDIR",
        "COMSPEC",
        "TEMP",
        "TMP",
        "TMPDIR",
        "HOME",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "VSINSTALLDIR",
        "VCINSTALLDIR",
        "VCToolsInstallDir",
        "WindowsSdkDir",
        "WindowsSDKVersion",
        "UCRTVersion",
        "UniversalCRTSdkDir",
        "INCLUDE",
        "LIB",
        "LIBPATH",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    if let Some(target_dir) = target_dir {
        command.env("CARGO_TARGET_DIR", target_dir);
    }
    if offline {
        command.env("CARGO_NET_OFFLINE", "true");
    }
    let mut child = command.spawn().map_err(command_failed)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| command_failed("Cargo stdout pipe is unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| command_failed("Cargo stderr pipe is unavailable"))?;
    let budget = Arc::new(AtomicUsize::new(0));
    let (failure_sender, mut failure_receiver) = tokio::sync::mpsc::channel(1);
    let stdout_task = tokio::spawn(read_cargo_output(
        stdout,
        Arc::clone(&budget),
        failure_sender.clone(),
    ));
    let stderr_task = tokio::spawn(read_cargo_output(stderr, budget, failure_sender.clone()));
    let status = tokio::select! {
        result = child.wait() => result.map_err(command_failed)?,
        _ = tokio::time::sleep(timeout) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(command_failed(format!("Cargo stage `{stage}` timed out")));
        }
        failure = failure_receiver.recv() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(command_failed(failure.unwrap_or_else(|| {
                format!("Cargo stage `{stage}` output reader stopped unexpectedly")
            })));
        }
    };
    drop(failure_sender);
    let stdout = collect_cargo_output(stdout_task).await?;
    let stderr = collect_cargo_output(stderr_task).await?;
    if !status.success() {
        let detail = if stderr.is_empty() { &stdout } else { &stderr };
        return Err(command_failed(format!(
            "Cargo stage `{stage}` failed with {status}: {}",
            bounded_process_text(detail)
        )));
    }
    Ok(())
}

async fn read_cargo_output<R>(
    mut reader: R,
    budget: Arc<AtomicUsize>,
    failure_sender: tokio::sync::mpsc::Sender<String>,
) -> Result<Vec<u8>, String>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(read) => read,
            Err(error) => {
                let message = format!("Cargo output read failed: {error}");
                let _ = failure_sender.send(message.clone()).await;
                return Err(message);
            }
        };
        if read == 0 {
            return Ok(output);
        }
        let previous = budget.fetch_add(read, Ordering::AcqRel);
        if previous.saturating_add(read) > MAX_LOCAL_CARGO_OUTPUT_BYTES {
            let message = "Cargo output exceeded the local authoring limit".to_string();
            let _ = failure_sender.send(message.clone()).await;
            return Err(message);
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

async fn collect_cargo_output(
    task: tokio::task::JoinHandle<Result<Vec<u8>, String>>,
) -> CliCoreResult<Vec<u8>> {
    task.await
        .map_err(|error| command_failed(format!("Cargo output task failed: {error}")))?
        .map_err(command_failed)
}

fn write_rendered_project(root: &Path, rendered: &RenderedModule) -> CliCoreResult<()> {
    for file in rendered.files() {
        let path = root.join(file.path);
        let parent = path
            .parent()
            .ok_or_else(|| invalid_input("rendered file has no parent"))?;
        fs::create_dir_all(parent).map_err(command_failed)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(command_failed)?;
        output.write_all(&file.contents).map_err(command_failed)?;
        output.sync_all().map_err(command_failed)?;
    }
    Ok(())
}

fn resolve_new_target(raw: &str) -> CliCoreResult<PathBuf> {
    let requested = Path::new(raw);
    if requested.as_os_str().is_empty()
        || requested
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid_input(
            "module init path must not contain current or parent traversal components",
        ));
    }
    let name = requested
        .file_name()
        .ok_or_else(|| invalid_input("module init path must name a new directory"))?;
    let parent = requested
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent =
        fs::canonicalize(parent.unwrap_or_else(|| Path::new("."))).map_err(command_failed)?;
    let target = parent.join(name);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(invalid_input("module init target already exists"));
    }
    Ok(target)
}

fn resolve_new_archive_output(raw: &str, source_root: &Path) -> CliCoreResult<PathBuf> {
    let requested = Path::new(raw);
    if requested.as_os_str().is_empty()
        || requested
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || requested
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("tar")
    {
        return Err(invalid_input(
            "module package output must be a .tar path without current or parent traversal",
        ));
    }
    let name = requested
        .file_name()
        .ok_or_else(|| invalid_input("module package output must name a new archive"))?;
    let parent = requested
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent =
        fs::canonicalize(parent.unwrap_or_else(|| Path::new("."))).map_err(command_failed)?;
    if parent.starts_with(source_root) {
        return Err(invalid_input(
            "module package output must be outside the source project",
        ));
    }
    let destination = parent.join(name);
    match fs::symlink_metadata(&destination) {
        Ok(_) => Err(invalid_input("module package output already exists")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(destination),
        Err(error) => Err(command_failed(error)),
    }
}

fn validate_project_relative_json_path(raw: &str) -> CliCoreResult<String> {
    let path = Path::new(raw);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.extension().and_then(|extension| extension.to_str()) != Some("json")
    {
        return Err(invalid_input(
            "module test scenario must be a project-relative .json path without traversal",
        ));
    }
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| invalid_input("module test scenario path must be UTF-8"))
}

fn validate_scenario_capabilities(
    declared_capabilities: &[rustok_sandbox::CapabilityName],
    scenario: &LocalSandboxScenario,
) -> CliCoreResult<()> {
    if scenario.policy.grants.iter().any(|grant| {
        !declared_capabilities
            .iter()
            .any(|declared| declared == &grant.name)
    }) {
        return Err(invalid_input(
            "module test scenario grants a capability absent from the source manifest",
        ));
    }
    Ok(())
}

fn prepare_local_target_directory(root: &Path, target_dir: &Path) -> CliCoreResult<()> {
    let target_root = root.join("target");
    ensure_regular_directory(&target_root)?;
    ensure_regular_directory(target_dir)
}

fn ensure_regular_directory(path: &Path) -> CliCoreResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_dir() => Ok(()),
        Ok(_) => Err(invalid_input(format!(
            "{} must be a regular non-symlink directory",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(command_failed)
        }
        Err(error) => Err(command_failed(error)),
    }
}

fn parse_uuid_option(args: &NormalizedArgs<'_>, name: &str) -> CliCoreResult<Uuid> {
    let raw = args.required_option(name)?;
    let parsed = Uuid::parse_str(raw)
        .map_err(|_| invalid_input(format!("--{} must be a UUID", name.replace('_', "-"))))?;
    if parsed.is_nil() {
        return Err(invalid_input(format!(
            "--{} must not be nil",
            name.replace('_', "-")
        )));
    }
    Ok(parsed)
}

fn reserve_build_archive_root() -> CliCoreResult<PathBuf> {
    let temporary = std::env::temp_dir();
    let metadata = fs::symlink_metadata(&temporary).map_err(command_failed)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(command_failed(
            "system temporary path must be a regular non-symlink directory",
        ));
    }
    for _ in 0..32 {
        let candidate = temporary.join(format!("rustok-module-build-{}", Uuid::new_v4()));
        match fs::create_dir(&candidate) {
            Ok(()) => return fs::canonicalize(candidate).map_err(command_failed),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(command_failed(error)),
        }
    }
    Err(command_failed(
        "could not reserve a unique temporary module build directory",
    ))
}

fn cleanup_build_archive_error(
    temporary_root: &Path,
    error: CliCoreError,
) -> CliCoreResult<CommandOutcome> {
    match fs::remove_dir_all(temporary_root) {
        Ok(()) => Err(error),
        Err(cleanup_error) => Err(command_failed(format!(
            "module build packaging failed and temporary source cleanup also failed: {cleanup_error}; original error: {error}"
        ))),
    }
}

fn source_archive_limits() -> CliCoreResult<ArchiveLimits> {
    ArchiveLimits::new(
        MAX_SOURCE_ARCHIVE_BYTES,
        MAX_SOURCE_BYTES,
        MAX_SOURCE_ENTRIES,
    )
    .map_err(map_archive_error)
}

fn map_source_archive_error(error: SourceArchiveError) -> CliCoreError {
    match error {
        SourceArchiveError::Io(message) => command_failed(message),
        other => invalid_input(other),
    }
}

fn map_archive_error(error: CasArchiveError) -> CliCoreError {
    match error {
        CasArchiveError::Io(message) | CasArchiveError::Unavailable(message) => {
            command_failed(message)
        }
        other => invalid_input(other),
    }
}

fn remove_failed_archive(destination: &Path, error: CliCoreError) -> CliCoreResult<CommandOutcome> {
    match fs::remove_file(destination) {
        Ok(()) => Err(error),
        Err(cleanup_error) => Err(command_failed(format!(
            "module package failed and {} could not be removed: {cleanup_error}; original error: {error}",
            destination.display()
        ))),
    }
}

fn read_regular_file(root: &Path, relative: &str, maximum: u64) -> CliCoreResult<Vec<u8>> {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(command_failed)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > maximum
    {
        return Err(invalid_input(format!(
            "{relative} must be a bounded regular non-symlink file"
        )));
    }
    fs::read(path).map_err(command_failed)
}

fn read_regular_absolute_file(path: &Path, maximum: u64) -> CliCoreResult<Vec<u8>> {
    if !path.is_absolute() {
        return Err(invalid_input("module test artifact path must be absolute"));
    }
    let metadata = fs::symlink_metadata(path).map_err(command_failed)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > maximum
    {
        return Err(invalid_input(
            "module test artifact must be a bounded regular non-symlink file",
        ));
    }
    fs::read(path).map_err(command_failed)
}

fn reject_final_descriptor(root: &Path) -> CliCoreResult<()> {
    match fs::symlink_metadata(root.join(FINAL_DESCRIPTOR_FILE)) {
        Ok(_) => Err(invalid_input(
            "source project must not contain module-artifact-descriptor.json",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(command_failed(error)),
    }
}

fn reject_source_cargo_config(root: &Path) -> CliCoreResult<()> {
    for relative in [".cargo/config", ".cargo/config.toml"] {
        match fs::symlink_metadata(root.join(relative)) {
            Ok(_) => {
                return Err(invalid_input(
                    "source-local Cargo configuration is forbidden",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(command_failed(error)),
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleBuildPolicy {
    schema_version: u32,
    allowed_registries: Vec<String>,
    allow_git_dependencies: bool,
    allow_build_scripts: bool,
    allow_native_links: bool,
}

impl ModuleBuildPolicy {
    fn validate(&self) -> CliCoreResult<()> {
        if self.schema_version != 1
            || self.allowed_registries.len() != 1
            || self.allowed_registries[0] != "https://crates.io"
            || self.allow_git_dependencies
            || self.allow_build_scripts
            || self.allow_native_links
        {
            return Err(invalid_input(
                "module build policy must use the current fail-closed crates.io profile",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ProjectValidation {
    path: PathBuf,
    slug: String,
    crate_name: String,
    version: String,
    sdk_version: String,
    template_version: String,
    rust_toolchain: String,
    lock_digest: String,
    entrypoint: String,
    capabilities: Vec<rustok_sandbox::CapabilityName>,
}

struct AuthoringMetadata {
    sdk_version: String,
    template_version: String,
    rust_toolchain: String,
}

impl ProjectValidation {
    fn data(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
            "slug": self.slug,
            "crate_name": self.crate_name,
            "version": self.version,
            "sdk_version": self.sdk_version,
            "template_version": self.template_version,
            "runtime_abi": MODULE_BUILD_RUNTIME_ABI,
            "wit_world": MODULE_BUILD_WIT_WORLD,
            "wit_version": MODULE_BUILD_WIT_VERSION,
            "component_target": MODULE_BUILD_COMPONENT_TARGET,
            "rust_toolchain": self.rust_toolchain,
            "dependency_lock_digest": self.lock_digest,
            "entrypoint": self.entrypoint,
            "capabilities": self.capabilities
        })
    }

    fn outcome(self, message: &str) -> CommandOutcome {
        CommandOutcome::success(message).with_data(self.data())
    }
}

struct NormalizedArgs<'a> {
    options: &'a serde_json::Map<String, serde_json::Value>,
    positionals: &'a [serde_json::Value],
}

impl<'a> NormalizedArgs<'a> {
    fn parse(value: &'a serde_json::Value) -> CliCoreResult<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_input("module command expects normalized arguments"))?;
        let options = object
            .get("options")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| invalid_input("module command options are missing"))?;
        let positionals = object
            .get("positionals")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_input("module command positionals are missing"))?;
        Ok(Self {
            options,
            positionals,
        })
    }

    fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).and_then(serde_json::Value::as_str)
    }

    fn required_option(&self, name: &str) -> CliCoreResult<&str> {
        self.option(name)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid_input(format!("--{} is required", name.replace('_', "-"))))
    }

    fn one_positional(&self, command: &str) -> CliCoreResult<&str> {
        if self.positionals.len() != 1 {
            return Err(invalid_input(format!(
                "{command} requires exactly one path"
            )));
        }
        self.positionals[0]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid_input(format!("{command} path is invalid")))
    }

    fn reject_unknown_options(&self, allowed: &[&str]) -> CliCoreResult<()> {
        if let Some(name) = self
            .options
            .keys()
            .find(|name| !allowed.contains(&name.as_str()))
        {
            return Err(invalid_input(format!(
                "unknown module command option --{}",
                name.replace('_', "-")
            )));
        }
        Ok(())
    }
}

fn default_display_name(slug: &str) -> String {
    slug.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + characters.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn valid_checksum(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn bounded_process_text(bytes: &[u8]) -> String {
    const LIMIT: usize = 4_096;
    String::from_utf8_lossy(&bytes[..bytes.len().min(LIMIT)])
        .trim()
        .to_string()
}

fn invalid_input(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::InvalidInput {
        message: error.to_string(),
    }
}

fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider() -> ModuleCommandProvider {
        ModuleCommandProvider {
            runtime: RuntimeComposition::without_database(serde_json::Value::Null),
        }
    }

    #[test]
    fn provider_exposes_authoring_commands() {
        let commands = test_provider().commands();
        assert_eq!(commands.len(), 7);
        assert_eq!(commands[0].namespace, "module");
        assert_eq!(commands[0].name, "init");
        assert!(commands[0].supports_dry_run);
        assert_eq!(commands[1].name, "validate");
        assert_eq!(commands[2].name, "test");
        assert!(commands[2].supports_dry_run);
        assert_eq!(commands[3].name, "build");
        assert!(commands[3].supports_dry_run);
        assert_eq!(commands[4].name, "package");
        assert!(commands[4].supports_dry_run);
        assert_eq!(commands[5].name, "publish");
        assert!(commands[5].supports_dry_run);
        assert_eq!(commands[6].name, "inspect");
        assert!(!commands[6].supports_dry_run);
    }

    #[tokio::test]
    async fn init_dry_run_validates_without_creating_the_target() {
        let target = std::env::temp_dir().join(format!("rustok-module-init-{}", Uuid::new_v4()));
        let request = CommandRequest {
            namespace: "module".to_string(),
            name: "init".to_string(),
            args: serde_json::json!({
                "options": { "slug": "sample_module" },
                "positionals": [target]
            }),
            dry_run: true,
        };
        let outcome = test_provider()
            .execute(request)
            .await
            .expect("dry-run init");
        assert_eq!(outcome.exit_code, 0);
        assert!(!target.exists());
    }

    #[test]
    fn traversal_target_is_rejected() {
        assert!(resolve_new_target("../sample-module").is_err());
        assert!(resolve_new_target("./sample-module").is_err());
        assert!(validate_project_relative_json_path("../scenario.json").is_err());
        assert!(validate_project_relative_json_path("/scenario.json").is_err());
    }

    #[tokio::test]
    async fn inspect_reports_a_canonical_source_archive() {
        let root = std::env::temp_dir().join(format!("rustok-module-inspect-{}", Uuid::new_v4()));
        let source = root.join("source");
        let archive = root.join("source.tar");
        fs::create_dir_all(&source).expect("source directory");
        fs::write(source.join("source.txt"), b"source\n").expect("source file");
        SourceArchiveBuilder::new(source_archive_limits().expect("limits"))
            .write(&source, &archive)
            .expect("source archive");

        let outcome = test_provider()
            .execute(CommandRequest {
                namespace: "module".to_string(),
                name: "inspect".to_string(),
                args: serde_json::json!({
                    "options": {},
                    "positionals": [archive]
                }),
                dry_run: false,
            })
            .await
            .expect("inspect source archive");
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.data["kind"], "source_archive");
        assert!(
            outcome.data["source_digest"]
                .as_str()
                .is_some_and(|digest| digest.starts_with("sha256:"))
        );

        fs::remove_dir_all(root).expect("remove test directory");
    }
}
