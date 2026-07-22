use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use rustok_build_source::{ArchiveLimits, CasArchiveError, CasArchiveReceipt, CasArchiveStore};
use rustok_distribution::generate_static_distribution;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{process::Command, time::timeout};

use crate::{
    executor::validate_evidence, StaticDistributionJobReceipt, StaticDistributionJobRequest,
};

const JOB_CONFIG_CONTRACT: &str = "rustok.static_distribution.job_config";
const MAX_JOB_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_CARGO_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CARGO_LOCK_BYTES: u64 = 32 * 1024 * 1024;
const MAX_COMMAND_TIMEOUT_SECONDS: u64 = 2 * 60 * 60;
const MAX_JOB_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_GENERATED_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PUBLICATION_RECEIPT_BYTES: u64 = 128 * 1024;
const TEST_EVIDENCE_FILE: &str = "test-evidence.json";
const PUBLISHER_REQUEST_FILE: &str = "publisher-request.json";
const PUBLISHER_RECEIPT_FILE: &str = "publisher-receipt.json";
const WORKSPACE_LOCK_FILE: &str = "Cargo.lock";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticDistributionJobConfig {
    pub contract: String,
    pub cas_root: PathBuf,
    pub cargo_path: PathBuf,
    pub cargo_digest: String,
    pub rustc_path: PathBuf,
    pub rustc_digest: String,
    pub cargo_home: PathBuf,
    pub publisher_path: PathBuf,
    pub publisher_digest: String,
    pub publisher_config_path: PathBuf,
    pub publisher_config_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub max_archive_bytes: u64,
    pub max_source_extracted_bytes: u64,
    pub max_total_extracted_bytes: u64,
    pub max_archive_entries: u32,
    pub command_timeout_seconds: u64,
}

impl StaticDistributionJobConfig {
    pub fn load(path: &Path, expected_digest: &str) -> Result<Self, StaticDistributionJobError> {
        if !valid_digest(expected_digest) {
            return Err(StaticDistributionJobError::InvalidConfig(
                "job config digest is invalid".to_string(),
            ));
        }
        let bytes = read_bounded_regular(path, MAX_JOB_CONFIG_BYTES)?;
        if digest_bytes(&bytes) != expected_digest {
            return Err(StaticDistributionJobError::InvalidConfig(
                "job config digest does not match".to_string(),
            ));
        }
        let config: Self = serde_json::from_slice(&bytes).map_err(|error| {
            StaticDistributionJobError::InvalidConfig(format!(
                "job config JSON is invalid: {error}"
            ))
        })?;
        config.validate_runtime()?;
        Ok(config)
    }

    pub fn validate_runtime(&self) -> Result<(), StaticDistributionJobError> {
        if self.contract != JOB_CONFIG_CONTRACT
            || !valid_digest(&self.cargo_digest)
            || !valid_digest(&self.rustc_digest)
            || !valid_digest(&self.publisher_digest)
            || !valid_digest(&self.publisher_config_digest)
            || !valid_digest(&self.toolchain_digest)
            || !valid_build_target(&self.build_target)
            || !self.cargo_home.is_absolute()
            || self.max_total_extracted_bytes < self.max_source_extracted_bytes
            || self.command_timeout_seconds == 0
            || self.command_timeout_seconds > MAX_COMMAND_TIMEOUT_SECONDS
        {
            return Err(StaticDistributionJobError::InvalidConfig(
                "job config fields are invalid".to_string(),
            ));
        }
        ArchiveLimits::new(
            self.max_archive_bytes,
            self.max_source_extracted_bytes,
            self.max_archive_entries,
        )?;
        CasArchiveStore::new(self.cas_root.clone())?;
        validate_fixed_file(&self.cargo_path, &self.cargo_digest, "Cargo executable")?;
        validate_fixed_file(&self.rustc_path, &self.rustc_digest, "Rustc executable")?;
        validate_cargo_home(&self.cargo_home)?;
        validate_fixed_file(
            &self.publisher_path,
            &self.publisher_digest,
            "evidence publisher",
        )?;
        validate_fixed_file(
            &self.publisher_config_path,
            &self.publisher_config_digest,
            "publisher config",
        )?;
        crate::publisher::StaticDistributionPublisherConfig::load(
            &self.publisher_config_path,
            &self.publisher_config_digest,
        )
        .map_err(|error| StaticDistributionJobError::InvalidConfig(error.to_string()))?;
        Ok(())
    }

    pub fn command_timeout(&self) -> Duration {
        Duration::from_secs(self.command_timeout_seconds)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedStaticDistributionWorkspace {
    pub workspace: PathBuf,
    pub platform_source: CasArchiveReceipt,
    pub promoted_sources: Vec<CasArchiveReceipt>,
    pub total_extracted_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticDistributionJobPaths {
    pub job_request: PathBuf,
    pub generated_manifest: PathBuf,
    pub cargo_dependencies: PathBuf,
    pub registry_source: PathBuf,
    pub job_config: PathBuf,
    pub job_receipt: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticDistributionTestEvidence {
    pub contract: String,
    pub job_request_digest: String,
    pub generated_output_digest: String,
    pub composition_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub cargo_digest: String,
    pub rustc_digest: String,
    pub lock_command: Vec<String>,
    pub test_command: Vec<String>,
    pub build_command: Vec<String>,
    pub resolved_lock_digest: String,
    pub tests_passed: bool,
    pub build_succeeded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticDistributionPublisherRequest {
    pub contract: String,
    pub distribution_build_id: uuid::Uuid,
    pub claim_id: uuid::Uuid,
    pub attempt_number: u32,
    pub job_request_digest: String,
    pub generated_output_digest: String,
    pub composition_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub resolved_lock_digest: String,
    pub test_evidence_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticDistributionPublicationReceipt {
    pub contract: String,
    pub publisher_request_digest: String,
    pub job_request_digest: String,
    pub generated_output_digest: String,
    pub composition_digest: String,
    pub resolved_lock_digest: String,
    pub test_evidence_payload_digest: String,
    pub evidence: rustok_modules::ModuleStaticDistributionBuildEvidence,
}

#[derive(Debug, Error)]
pub enum StaticDistributionJobError {
    #[error("static distribution job config is invalid: {0}")]
    InvalidConfig(String),
    #[error("static distribution job input is invalid: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Source(#[from] CasArchiveError),
    #[error("static distribution workspace operation failed: {0}")]
    Io(String),
    #[error("static distribution fixed command failed: {0}")]
    Command(String),
}

pub async fn run_static_distribution_job(
    paths: StaticDistributionJobPaths,
) -> Result<(), StaticDistributionJobError> {
    let job_dir = validate_job_paths(&paths)?;
    let request_bytes = read_bounded_regular(&paths.job_request, MAX_JOB_REQUEST_BYTES)?;
    let request: StaticDistributionJobRequest =
        serde_json::from_slice(&request_bytes).map_err(|error| {
            StaticDistributionJobError::InvalidInput(format!(
                "job request JSON is invalid: {error}"
            ))
        })?;
    if request.contract != "rustok.static_distribution.job" {
        return Err(StaticDistributionJobError::InvalidInput(
            "job request contract is invalid".to_string(),
        ));
    }
    let job_request_digest = digest_bytes(&request_bytes);
    let config = StaticDistributionJobConfig::load(&paths.job_config, &request.job_config_digest)?;
    let publisher_config = crate::publisher::StaticDistributionPublisherConfig::load(
        &config.publisher_config_path,
        &config.publisher_config_digest,
    )
    .map_err(|error| StaticDistributionJobError::InvalidConfig(error.to_string()))?;
    let generated_manifest_bytes =
        read_bounded_regular(&paths.generated_manifest, MAX_GENERATED_FILE_BYTES)?;
    let cargo_dependencies_bytes =
        read_bounded_regular(&paths.cargo_dependencies, MAX_GENERATED_FILE_BYTES)?;
    let registry_source_bytes =
        read_bounded_regular(&paths.registry_source, MAX_GENERATED_FILE_BYTES)?;
    prepare_derived_workspace(&job_dir)?;
    let prepared = match materialize_static_distribution_workspace(
        &job_dir,
        &request,
        &generated_manifest_bytes,
        &cargo_dependencies_bytes,
        &registry_source_bytes,
        &config,
    ) {
        Ok(prepared) => prepared,
        Err(error) if terminal_source_error(&error) => {
            return write_terminal_receipt(
                &paths.job_receipt,
                &request,
                &job_request_digest,
                rustok_modules::ModuleStaticDistributionCompletionOutcome::Failed {
                    failure_code: "static_source_invalid".to_string(),
                    failure_detail: "static distribution source materialization was rejected"
                        .to_string(),
                },
            );
        }
        Err(error) => return Err(error),
    };

    let lock_command = cargo_lock_command();
    match run_fixed_command(
        &config.cargo_path,
        &lock_command,
        &prepared.workspace,
        &config,
    )
    .await?
    {
        FixedCommandOutcome::Succeeded => {}
        FixedCommandOutcome::Failed => {
            return write_terminal_receipt(
                &paths.job_receipt,
                &request,
                &job_request_digest,
                failed_outcome(
                    "static_lock_resolution_failed",
                    "static distribution dependency lock resolution failed",
                ),
            );
        }
        FixedCommandOutcome::TimedOut => {
            return write_terminal_receipt(
                &paths.job_receipt,
                &request,
                &job_request_digest,
                failed_outcome(
                    "static_lock_resolution_timed_out",
                    "static distribution dependency lock resolution exceeded the command deadline",
                ),
            );
        }
    }
    let resolved_lock_digest = digest_bounded_regular(
        &prepared.workspace.join(WORKSPACE_LOCK_FILE),
        MAX_CARGO_LOCK_BYTES,
    )?;

    let test_command = cargo_test_command(&config);
    match run_fixed_command(
        &config.cargo_path,
        &test_command,
        &prepared.workspace,
        &config,
    )
    .await?
    {
        FixedCommandOutcome::Succeeded => {}
        FixedCommandOutcome::Failed => {
            return write_terminal_receipt(
                &paths.job_receipt,
                &request,
                &job_request_digest,
                failed_outcome("static_tests_failed", "static distribution tests failed"),
            );
        }
        FixedCommandOutcome::TimedOut => {
            return write_terminal_receipt(
                &paths.job_receipt,
                &request,
                &job_request_digest,
                failed_outcome(
                    "static_tests_timed_out",
                    "static distribution tests exceeded the command deadline",
                ),
            );
        }
    }

    let build_command = cargo_build_command(&config);
    match run_fixed_command(
        &config.cargo_path,
        &build_command,
        &prepared.workspace,
        &config,
    )
    .await?
    {
        FixedCommandOutcome::Succeeded => {}
        FixedCommandOutcome::Failed => {
            return write_terminal_receipt(
                &paths.job_receipt,
                &request,
                &job_request_digest,
                failed_outcome(
                    "static_build_failed",
                    "static distribution release build failed",
                ),
            );
        }
        FixedCommandOutcome::TimedOut => {
            return write_terminal_receipt(
                &paths.job_receipt,
                &request,
                &job_request_digest,
                failed_outcome(
                    "static_build_timed_out",
                    "static distribution build exceeded the command deadline",
                ),
            );
        }
    }

    let test_evidence = StaticDistributionTestEvidence {
        contract: "rustok.static_distribution.test_evidence".to_string(),
        job_request_digest: job_request_digest.clone(),
        generated_output_digest: request.generated_output_digest.clone(),
        composition_digest: request.composition_digest.clone(),
        toolchain_digest: request.toolchain_digest.clone(),
        build_target: request.build_target.clone(),
        cargo_digest: config.cargo_digest.clone(),
        rustc_digest: config.rustc_digest.clone(),
        lock_command: lock_command.clone(),
        test_command: test_command.clone(),
        build_command: build_command.clone(),
        resolved_lock_digest: resolved_lock_digest.clone(),
        tests_passed: true,
        build_succeeded: true,
    };
    let test_evidence_bytes = serde_json::to_vec_pretty(&test_evidence)
        .map_err(|error| StaticDistributionJobError::Io(error.to_string()))?;
    let test_evidence_path = job_dir.join(TEST_EVIDENCE_FILE);
    write_new_or_verify_file(&test_evidence_path, &test_evidence_bytes)?;
    let test_evidence_digest = digest_bytes(&test_evidence_bytes);
    let publisher_request = StaticDistributionPublisherRequest {
        contract: "rustok.static_distribution.publisher_request".to_string(),
        distribution_build_id: request.distribution_build_id,
        claim_id: request.claim_id,
        attempt_number: request.attempt_number,
        job_request_digest: job_request_digest.clone(),
        generated_output_digest: request.generated_output_digest.clone(),
        composition_digest: request.composition_digest.clone(),
        toolchain_digest: request.toolchain_digest.clone(),
        build_target: request.build_target.clone(),
        resolved_lock_digest: resolved_lock_digest.clone(),
        test_evidence_digest: test_evidence_digest.clone(),
    };
    let publisher_request_bytes = serde_json::to_vec_pretty(&publisher_request)
        .map_err(|error| StaticDistributionJobError::Io(error.to_string()))?;
    let publisher_request_path = job_dir.join(PUBLISHER_REQUEST_FILE);
    write_new_or_verify_file(&publisher_request_path, &publisher_request_bytes)?;
    let publisher_request_digest = digest_bytes(&publisher_request_bytes);
    let publisher_receipt_path = job_dir.join(PUBLISHER_RECEIPT_FILE);
    if !path_entry_exists(&publisher_receipt_path)? {
        run_publisher(
            &config,
            &publisher_request_path,
            &prepared.workspace,
            &test_evidence_path,
            &publisher_receipt_path,
        )
        .await?;
    }
    let publication = load_publication_receipt(
        &publisher_receipt_path,
        &publisher_request,
        &publisher_request_digest,
        &test_evidence_digest,
        &publisher_config,
    )?;
    write_terminal_receipt(
        &paths.job_receipt,
        &request,
        &job_request_digest,
        rustok_modules::ModuleStaticDistributionCompletionOutcome::Succeeded {
            evidence: publication.evidence,
        },
    )
}

pub fn materialize_static_distribution_workspace(
    job_dir: &Path,
    request: &StaticDistributionJobRequest,
    generated_manifest_bytes: &[u8],
    cargo_dependencies_bytes: &[u8],
    registry_source_bytes: &[u8],
    config: &StaticDistributionJobConfig,
) -> Result<PreparedStaticDistributionWorkspace, StaticDistributionJobError> {
    config.validate_runtime()?;
    request.work_item.validate().map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!("work item is invalid: {error}"))
    })?;
    if request.toolchain_digest != config.toolchain_digest
        || request.build_target != config.build_target
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "request does not match the job-config toolchain and target".to_string(),
        ));
    }
    validate_directory(job_dir, "job directory")?;
    let job_dir = fs::canonicalize(job_dir).map_err(io_error)?;
    let generated = generate_static_distribution(&request.work_item).map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!(
            "generated distribution is invalid: {error}"
        ))
    })?;
    if generated.manifest.output_digest != request.generated_output_digest
        || generated.manifest_json != generated_manifest_bytes
        || generated.cargo_dependencies_toml.as_bytes() != cargo_dependencies_bytes
        || generated.registry_source.as_bytes() != registry_source_bytes
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "generated files do not match the immutable request".to_string(),
        ));
    }

    let workspace = job_dir.join("workspace");
    let source_store = CasArchiveStore::new(config.cas_root.clone())?;
    let limits = ArchiveLimits::new(
        config.max_archive_bytes,
        config.max_source_extracted_bytes,
        config.max_archive_entries,
    )?;
    let result = materialize_sources_and_apply(
        &source_store,
        &workspace,
        request,
        &generated,
        generated_manifest_bytes,
        cargo_dependencies_bytes,
        registry_source_bytes,
        config,
        limits,
    );
    if result.is_err() {
        remove_owned_workspace(&job_dir, &workspace);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn materialize_sources_and_apply(
    source_store: &CasArchiveStore,
    workspace: &Path,
    request: &StaticDistributionJobRequest,
    generated: &rustok_distribution::GeneratedStaticDistributionFiles,
    generated_manifest_bytes: &[u8],
    cargo_dependencies_bytes: &[u8],
    registry_source_bytes: &[u8],
    config: &StaticDistributionJobConfig,
    limits: ArchiveLimits,
) -> Result<PreparedStaticDistributionWorkspace, StaticDistributionJobError> {
    let platform_source = source_store.materialize(
        &request.work_item.build.platform_source_reference,
        &request.work_item.build.platform_source_digest,
        workspace,
        limits,
    )?;
    let mut total_extracted_bytes = platform_source.extracted_bytes;
    if total_extracted_bytes > config.max_total_extracted_bytes {
        return Err(StaticDistributionJobError::Source(
            CasArchiveError::ResourceLimit,
        ));
    }
    let source_parent = workspace.join(".rustok").join("static-sources");
    create_directory_path(&source_parent)?;
    let mut promoted_sources = Vec::with_capacity(generated.manifest.sources.len());
    for source in &generated.manifest.sources {
        let relative = validated_relative_path(&source.materialization_path)?;
        let destination = workspace.join(relative);
        if destination.parent() != Some(source_parent.as_path()) {
            return Err(StaticDistributionJobError::InvalidInput(
                "generated source path escaped the fixed materialization root".to_string(),
            ));
        }
        let receipt = source_store.materialize(
            &source.source_reference,
            &source.source_digest,
            &destination,
            limits,
        )?;
        total_extracted_bytes = total_extracted_bytes
            .checked_add(receipt.extracted_bytes)
            .ok_or(CasArchiveError::ResourceLimit)?;
        if total_extracted_bytes > config.max_total_extracted_bytes {
            return Err(StaticDistributionJobError::Source(
                CasArchiveError::ResourceLimit,
            ));
        }
        validate_promoted_package(&destination, source)?;
        promoted_sources.push(receipt);
    }
    apply_cargo_dependencies(
        workspace,
        &generated.manifest.cargo_manifest_path,
        cargo_dependencies_bytes,
    )?;
    replace_generated_file(
        workspace,
        &generated.manifest.registry_source_path,
        registry_source_bytes,
        false,
    )?;
    replace_generated_file(
        workspace,
        &generated.manifest.manifest_path,
        generated_manifest_bytes,
        true,
    )?;
    Ok(PreparedStaticDistributionWorkspace {
        workspace: workspace.to_path_buf(),
        platform_source,
        promoted_sources,
        total_extracted_bytes,
    })
}

fn validate_promoted_package(
    source_root: &Path,
    source: &rustok_distribution::GeneratedStaticDistributionSource,
) -> Result<(), StaticDistributionJobError> {
    let manifest_path = source_root.join("Cargo.toml");
    let manifest_bytes = read_bounded_regular(&manifest_path, MAX_CARGO_MANIFEST_BYTES)?;
    let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|_| {
        StaticDistributionJobError::InvalidInput("promoted Cargo manifest is not UTF-8".to_string())
    })?;
    let manifest = manifest_text.parse::<toml::Table>().map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!(
            "promoted Cargo manifest is invalid: {error}"
        ))
    })?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            StaticDistributionJobError::InvalidInput(
                "promoted Cargo package table is missing".to_string(),
            )
        })?;
    if package.get("name").and_then(toml::Value::as_str) != Some(source.cargo_package.as_str())
        || package.get("version").and_then(toml::Value::as_str)
            != Some(source.module_version.as_str())
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "promoted Cargo package identity does not match the reviewed release".to_string(),
        ));
    }
    let lock_path = source_root.join("Cargo.lock");
    let lock_bytes = read_bounded_regular(&lock_path, MAX_CARGO_LOCK_BYTES)?;
    if digest_bytes(&lock_bytes) != source.dependency_lock_digest {
        return Err(StaticDistributionJobError::InvalidInput(
            "promoted Cargo.lock does not match the reviewed dependency graph".to_string(),
        ));
    }
    Ok(())
}

fn apply_cargo_dependencies(
    workspace: &Path,
    relative_manifest_path: &str,
    cargo_dependencies_bytes: &[u8],
) -> Result<(), StaticDistributionJobError> {
    let relative = validated_relative_path(relative_manifest_path)?;
    let manifest_path = workspace.join(relative);
    let manifest_bytes = read_bounded_regular(&manifest_path, MAX_CARGO_MANIFEST_BYTES)?;
    let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|_| {
        StaticDistributionJobError::InvalidInput(
            "distribution Cargo manifest is not UTF-8".to_string(),
        )
    })?;
    let mut manifest = manifest_text.parse::<toml::Table>().map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!(
            "distribution Cargo manifest is invalid: {error}"
        ))
    })?;
    let fragment_text = std::str::from_utf8(cargo_dependencies_bytes).map_err(|_| {
        StaticDistributionJobError::InvalidInput(
            "generated Cargo dependency fragment is not UTF-8".to_string(),
        )
    })?;
    let fragment = format!("[dependencies]\n{fragment_text}")
        .parse::<toml::Table>()
        .map_err(|error| {
            StaticDistributionJobError::InvalidInput(format!(
                "generated Cargo dependency fragment is invalid: {error}"
            ))
        })?;
    let generated_dependencies = fragment
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            StaticDistributionJobError::InvalidInput(
                "generated Cargo dependencies are missing".to_string(),
            )
        })?;
    let dependencies = manifest
        .entry("dependencies")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| {
            StaticDistributionJobError::InvalidInput(
                "distribution dependencies are not a table".to_string(),
            )
        })?;
    for (alias, dependency) in generated_dependencies {
        if dependencies.contains_key(alias) {
            return Err(StaticDistributionJobError::InvalidInput(format!(
                "generated dependency alias already exists: {alias}"
            )));
        }
        dependencies.insert(alias.clone(), dependency.clone());
    }
    let output = toml::to_string_pretty(&manifest).map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!(
            "distribution Cargo manifest could not be serialized: {error}"
        ))
    })?;
    overwrite_regular_file(&manifest_path, output.as_bytes())
}

fn replace_generated_file(
    workspace: &Path,
    relative_path: &str,
    bytes: &[u8],
    create_parent: bool,
) -> Result<(), StaticDistributionJobError> {
    let relative = validated_relative_path(relative_path)?;
    let path = workspace.join(relative);
    let parent = path.parent().ok_or_else(|| {
        StaticDistributionJobError::InvalidInput("generated output path has no parent".to_string())
    })?;
    if create_parent {
        create_directory_path(parent)?;
    } else {
        validate_directory(parent, "generated output parent")?;
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(StaticDistributionJobError::InvalidInput(
                "generated output target is not a regular file".to_string(),
            ));
        }
        Ok(_) => overwrite_regular_file(&path, bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_parent => {
            write_new_file(&path, bytes)
        }
        Err(error) => Err(io_error(error)),
    }
}

fn validated_relative_path(value: &str) -> Result<PathBuf, StaticDistributionJobError> {
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "generated output path is unsafe".to_string(),
        ));
    }
    Ok(path)
}

fn create_directory_path(path: &Path) -> Result<(), StaticDistributionJobError> {
    fs::create_dir_all(path).map_err(io_error)?;
    validate_directory(path, "generated directory")
}

fn validate_directory(path: &Path, label: &str) -> Result<(), StaticDistributionJobError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(StaticDistributionJobError::InvalidInput(format!(
            "{label} must be a non-symlink directory"
        )));
    }
    Ok(())
}

fn validate_fixed_file(
    path: &Path,
    expected_digest: &str,
    label: &str,
) -> Result<(), StaticDistributionJobError> {
    if !path.is_absolute() {
        return Err(StaticDistributionJobError::InvalidConfig(format!(
            "{label} path must be absolute"
        )));
    }
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StaticDistributionJobError::InvalidConfig(format!(
            "{label} must be a non-symlink file"
        )));
    }
    if digest_file(path)? != expected_digest {
        return Err(StaticDistributionJobError::InvalidConfig(format!(
            "{label} digest does not match"
        )));
    }
    Ok(())
}

fn read_bounded_regular(
    path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, StaticDistributionJobError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes {
        return Err(StaticDistributionJobError::InvalidInput(
            "job file is not a bounded regular file".to_string(),
        ));
    }
    fs::read(path).map_err(io_error)
}

fn overwrite_regular_file(path: &Path, bytes: &[u8]) -> Result<(), StaticDistributionJobError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StaticDistributionJobError::InvalidInput(
            "workspace output target is not a regular file".to_string(),
        ));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(io_error)?;
    file.write_all(bytes).map_err(io_error)?;
    file.sync_all().map_err(io_error)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), StaticDistributionJobError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io_error)?;
    file.write_all(bytes).map_err(io_error)?;
    file.sync_all().map_err(io_error)
}

fn write_new_or_verify_file(path: &Path, bytes: &[u8]) -> Result<(), StaticDistributionJobError> {
    match write_new_file(path, bytes) {
        Ok(()) => Ok(()),
        Err(StaticDistributionJobError::Io(_)) => {
            let existing = read_bounded_regular(path, bytes.len() as u64)?;
            if existing == bytes {
                Ok(())
            } else {
                Err(StaticDistributionJobError::InvalidInput(
                    "derived job file conflicts with the immutable request".to_string(),
                ))
            }
        }
        Err(error) => Err(error),
    }
}

fn digest_bounded_regular(
    path: &Path,
    max_bytes: u64,
) -> Result<String, StaticDistributionJobError> {
    let bytes = read_bounded_regular(path, max_bytes)?;
    Ok(digest_bytes(&bytes))
}

fn digest_file(path: &Path) -> Result<String, StaticDistributionJobError> {
    let mut file = fs::File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.trim() == value
        && value.len() <= max_bytes
        && !value.chars().any(char::is_control)
}

fn valid_build_target(value: &str) -> bool {
    valid_text(value, 128)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !value.starts_with('.')
        && !value.ends_with('.')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FixedCommandOutcome {
    Succeeded,
    Failed,
    TimedOut,
}

fn validate_job_paths(
    paths: &StaticDistributionJobPaths,
) -> Result<PathBuf, StaticDistributionJobError> {
    let request = fs::canonicalize(&paths.job_request).map_err(io_error)?;
    let job_dir = request
        .parent()
        .ok_or_else(|| {
            StaticDistributionJobError::InvalidInput(
                "job request has no parent directory".to_string(),
            )
        })?
        .to_path_buf();
    validate_directory(&job_dir, "job directory")?;
    for path in [
        &paths.generated_manifest,
        &paths.cargo_dependencies,
        &paths.registry_source,
    ] {
        let canonical = fs::canonicalize(path).map_err(io_error)?;
        if canonical.parent() != Some(job_dir.as_path()) {
            return Err(StaticDistributionJobError::InvalidInput(
                "job input escaped its attempt directory".to_string(),
            ));
        }
    }
    if !paths.job_receipt.is_absolute() || paths.job_receipt.parent() != Some(job_dir.as_path()) {
        return Err(StaticDistributionJobError::InvalidInput(
            "job receipt escaped its attempt directory".to_string(),
        ));
    }
    match fs::symlink_metadata(&paths.job_receipt) {
        Ok(_) => Err(StaticDistributionJobError::InvalidInput(
            "job receipt already exists".to_string(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(job_dir),
        Err(error) => Err(io_error(error)),
    }
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

fn validate_cargo_home(path: &Path) -> Result<(), StaticDistributionJobError> {
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

async fn run_publisher(
    config: &StaticDistributionJobConfig,
    publisher_request: &Path,
    workspace: &Path,
    test_evidence: &Path,
    publisher_receipt: &Path,
) -> Result<(), StaticDistributionJobError> {
    config.validate_runtime()?;
    let mut command = Command::new(&config.publisher_path);
    command
        .arg("--request")
        .arg(publisher_request)
        .arg("--workspace")
        .arg(workspace)
        .arg("--test-evidence")
        .arg(test_evidence)
        .arg("--config")
        .arg(&config.publisher_config_path)
        .arg("--config-digest")
        .arg(&config.publisher_config_digest)
        .arg("--receipt")
        .arg(publisher_receipt)
        .current_dir(workspace)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let status = timeout(config.command_timeout(), command.status())
        .await
        .map_err(|_| {
            StaticDistributionJobError::Command(
                "evidence publisher exceeded the command deadline".to_string(),
            )
        })?
        .map_err(|error| StaticDistributionJobError::Command(error.to_string()))?;
    if !status.success() {
        return Err(StaticDistributionJobError::Command(format!(
            "evidence publisher exited with status {status}"
        )));
    }
    Ok(())
}

fn load_publication_receipt(
    path: &Path,
    request: &StaticDistributionPublisherRequest,
    publisher_request_digest: &str,
    test_evidence_digest: &str,
    publisher_config: &crate::publisher::StaticDistributionPublisherConfig,
) -> Result<StaticDistributionPublicationReceipt, StaticDistributionJobError> {
    let bytes = read_bounded_regular(path, MAX_PUBLICATION_RECEIPT_BYTES)?;
    let receipt: StaticDistributionPublicationReceipt =
        serde_json::from_slice(&bytes).map_err(|error| {
            StaticDistributionJobError::InvalidInput(format!(
                "publisher receipt JSON is invalid: {error}"
            ))
        })?;
    if receipt.contract != "rustok.static_distribution.publication_receipt"
        || receipt.publisher_request_digest != publisher_request_digest
        || receipt.job_request_digest != request.job_request_digest
        || receipt.generated_output_digest != request.generated_output_digest
        || receipt.composition_digest != request.composition_digest
        || receipt.resolved_lock_digest != request.resolved_lock_digest
        || receipt.test_evidence_payload_digest != test_evidence_digest
        || validate_evidence(&receipt.evidence).is_err()
        || !evidence_matches_target(&receipt.evidence, publisher_config)
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "publisher receipt does not match the immutable request".to_string(),
        ));
    }
    Ok(receipt)
}

fn evidence_matches_target(
    evidence: &rustok_modules::ModuleStaticDistributionBuildEvidence,
    publisher_config: &crate::publisher::StaticDistributionPublisherConfig,
) -> bool {
    let target = publisher_config.publication_target();
    [
        (&evidence.artifact_reference, &evidence.artifact_digest),
        (&evidence.sbom_reference, &evidence.sbom_digest),
        (&evidence.provenance_reference, &evidence.provenance_digest),
        (&evidence.signature_reference, &evidence.signature_digest),
        (
            &evidence.test_evidence_reference,
            &evidence.test_evidence_digest,
        ),
    ]
    .into_iter()
    .all(|(reference, digest)| {
        reference == &format!("{}/{}@{}", target.registry, target.repository, digest)
    })
}

fn prepare_derived_workspace(job_dir: &Path) -> Result<(), StaticDistributionJobError> {
    let workspace = job_dir.join("workspace");
    match fs::symlink_metadata(&workspace) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(&workspace).map_err(io_error)?;
            if path_entry_exists(&workspace)? {
                return Err(StaticDistributionJobError::Io(
                    "stale derived workspace could not be removed".to_string(),
                ));
            }
            Ok(())
        }
        Ok(_) => Err(StaticDistributionJobError::InvalidInput(
            "derived workspace is not an owned directory".to_string(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

fn path_entry_exists(path: &Path) -> Result<bool, StaticDistributionJobError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(error)),
    }
}

fn write_terminal_receipt(
    path: &Path,
    request: &StaticDistributionJobRequest,
    job_request_digest: &str,
    outcome: rustok_modules::ModuleStaticDistributionCompletionOutcome,
) -> Result<(), StaticDistributionJobError> {
    let receipt = StaticDistributionJobReceipt {
        contract: "rustok.static_distribution.job_receipt".to_string(),
        distribution_build_id: request.distribution_build_id,
        claim_id: request.claim_id,
        attempt_number: request.attempt_number,
        composition_revision: request.composition_revision,
        composition_digest: request.composition_digest.clone(),
        generated_output_digest: request.generated_output_digest.clone(),
        job_request_digest: job_request_digest.to_string(),
        runner_digest: request.runner_digest.clone(),
        job_config_digest: request.job_config_digest.clone(),
        toolchain_digest: request.toolchain_digest.clone(),
        build_target: request.build_target.clone(),
        outcome,
    };
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| StaticDistributionJobError::Io(error.to_string()))?;
    write_new_file(path, &bytes)
}

fn failed_outcome(
    code: &str,
    detail: &str,
) -> rustok_modules::ModuleStaticDistributionCompletionOutcome {
    rustok_modules::ModuleStaticDistributionCompletionOutcome::Failed {
        failure_code: code.to_string(),
        failure_detail: detail.to_string(),
    }
}

fn terminal_source_error(error: &StaticDistributionJobError) -> bool {
    matches!(
        error,
        StaticDistributionJobError::InvalidInput(_)
            | StaticDistributionJobError::Source(CasArchiveError::InvalidReference)
            | StaticDistributionJobError::Source(CasArchiveError::InvalidDigest)
            | StaticDistributionJobError::Source(CasArchiveError::DigestMismatch)
            | StaticDistributionJobError::Source(CasArchiveError::UnsafeArchive)
            | StaticDistributionJobError::Source(CasArchiveError::ResourceLimit)
    )
}

fn remove_owned_workspace(job_dir: &Path, workspace: &Path) {
    if workspace.is_absolute()
        && workspace.parent() == Some(job_dir)
        && fs::symlink_metadata(workspace)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        let _ = fs::remove_dir_all(workspace);
    }
}

fn io_error(error: impl std::fmt::Display) -> StaticDistributionJobError {
    StaticDistributionJobError::Io(error.to_string())
}
