use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use rustok_build_source::{ArchiveLimits, CasArchiveError, CasArchiveReceipt, CasArchiveStore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{process::Command, time::timeout};

use crate::{
    StaticDistributionJobReceipt, StaticDistributionJobRequest, executor::validate_evidence,
};

mod pipeline;
mod workspace;
use pipeline::{CargoPipelineEvidence, run_cargo_pipeline, validate_cargo_home};
pub use workspace::materialize_static_distribution_workspace;

const JOB_CONFIG_CONTRACT: &str = "rustok.static_distribution.job_config";
const MAX_JOB_CONFIG_BYTES: u64 = 64 * 1024;
pub(super) const MAX_CARGO_LOCK_BYTES: u64 = 32 * 1024 * 1024;
const MAX_COMMAND_TIMEOUT_SECONDS: u64 = 2 * 60 * 60;
const MAX_JOB_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_GENERATED_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PUBLICATION_RECEIPT_BYTES: u64 = 128 * 1024;
const TEST_EVIDENCE_FILE: &str = "test-evidence.json";
const PUBLISHER_REQUEST_FILE: &str = "publisher-request.json";
const PUBLISHER_RECEIPT_FILE: &str = "publisher-receipt.json";
pub(super) const WORKSPACE_LOCK_FILE: &str = "Cargo.lock";

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
        self.validate_fields()?;
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

    fn validate_fields(&self) -> Result<(), StaticDistributionJobError> {
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

struct LoadedJobInputs {
    job_dir: PathBuf,
    request: StaticDistributionJobRequest,
    job_request_digest: String,
    config: StaticDistributionJobConfig,
    publisher_config: crate::publisher::StaticDistributionPublisherConfig,
    generated_manifest_bytes: Vec<u8>,
    cargo_dependencies_bytes: Vec<u8>,
    registry_source_bytes: Vec<u8>,
}

fn load_job_inputs(
    paths: &StaticDistributionJobPaths,
) -> Result<LoadedJobInputs, StaticDistributionJobError> {
    let job_dir = validate_job_paths(paths)?;
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
    Ok(LoadedJobInputs {
        job_dir,
        request,
        job_request_digest,
        config,
        publisher_config,
        generated_manifest_bytes,
        cargo_dependencies_bytes,
        registry_source_bytes,
    })
}

fn prepare_and_materialize_workspace(
    inputs: &LoadedJobInputs,
    job_receipt: &Path,
) -> Result<Option<PreparedStaticDistributionWorkspace>, StaticDistributionJobError> {
    prepare_derived_workspace(&inputs.job_dir)?;
    match materialize_static_distribution_workspace(
        &inputs.job_dir,
        &inputs.request,
        &inputs.generated_manifest_bytes,
        &inputs.cargo_dependencies_bytes,
        &inputs.registry_source_bytes,
        &inputs.config,
    ) {
        Ok(prepared) => Ok(Some(prepared)),
        Err(error) if terminal_source_error(&error) => {
            write_terminal_receipt(
                job_receipt,
                &inputs.request,
                &inputs.job_request_digest,
                failed_outcome(
                    "static_source_invalid",
                    "static distribution source materialization was rejected",
                ),
            )?;
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

async fn publish_and_write_success_receipt(
    inputs: &LoadedJobInputs,
    receipt_path: &Path,
    workspace: &Path,
    pipeline: &CargoPipelineEvidence,
) -> Result<(), StaticDistributionJobError> {
    let publication = run_publisher_and_receipt(
        &inputs.job_dir,
        workspace,
        &inputs.config,
        &inputs.publisher_config,
        &inputs.request,
        &inputs.job_request_digest,
        pipeline,
    )
    .await?;

    write_terminal_receipt(
        receipt_path,
        &inputs.request,
        &inputs.job_request_digest,
        rustok_modules::ModuleStaticDistributionCompletionOutcome::Succeeded {
            evidence: Box::new(publication.evidence),
        },
    )
}

pub async fn run_static_distribution_job(
    paths: StaticDistributionJobPaths,
) -> Result<(), StaticDistributionJobError> {
    let inputs = load_job_inputs(&paths)?;
    let prepared = match prepare_and_materialize_workspace(&inputs, &paths.job_receipt)? {
        Some(prepared) => prepared,
        None => return Ok(()),
    };

    let Some(pipeline) = run_cargo_pipeline(
        &inputs.config,
        &prepared.workspace,
        &paths.job_receipt,
        &inputs.request,
        &inputs.job_request_digest,
    )
    .await?
    else {
        return Ok(());
    };

    publish_and_write_success_receipt(
        &inputs,
        &paths.job_receipt,
        &prepared.workspace,
        &pipeline,
    )
    .await
}

fn write_test_evidence(
    job_dir: &Path,
    config: &StaticDistributionJobConfig,
    request: &StaticDistributionJobRequest,
    job_request_digest: &str,
    pipeline: &CargoPipelineEvidence,
) -> Result<(PathBuf, String), StaticDistributionJobError> {
    let test_evidence = StaticDistributionTestEvidence {
        contract: "rustok.static_distribution.test_evidence".to_string(),
        job_request_digest: job_request_digest.to_string(),
        generated_output_digest: request.generated_output_digest.clone(),
        composition_digest: request.composition_digest.clone(),
        toolchain_digest: request.toolchain_digest.clone(),
        build_target: request.build_target.clone(),
        cargo_digest: config.cargo_digest.clone(),
        rustc_digest: config.rustc_digest.clone(),
        lock_command: pipeline.lock_command.clone(),
        test_command: pipeline.test_command.clone(),
        build_command: pipeline.build_command.clone(),
        resolved_lock_digest: pipeline.resolved_lock_digest.clone(),
        tests_passed: true,
        build_succeeded: true,
    };
    let test_evidence_bytes = serde_json::to_vec_pretty(&test_evidence)
        .map_err(|error| StaticDistributionJobError::Io(error.to_string()))?;
    let test_evidence_path = job_dir.join(TEST_EVIDENCE_FILE);
    write_new_or_verify_file(&test_evidence_path, &test_evidence_bytes)?;
    let test_evidence_digest = digest_bytes(&test_evidence_bytes);
    Ok((test_evidence_path, test_evidence_digest))
}

fn write_publisher_request(
    job_dir: &Path,
    request: &StaticDistributionJobRequest,
    job_request_digest: &str,
    pipeline: &CargoPipelineEvidence,
    test_evidence_digest: &str,
) -> Result<(StaticDistributionPublisherRequest, PathBuf, String), StaticDistributionJobError> {
    let publisher_request = StaticDistributionPublisherRequest {
        contract: "rustok.static_distribution.publisher_request".to_string(),
        distribution_build_id: request.distribution_build_id,
        claim_id: request.claim_id,
        attempt_number: request.attempt_number,
        job_request_digest: job_request_digest.to_string(),
        generated_output_digest: request.generated_output_digest.clone(),
        composition_digest: request.composition_digest.clone(),
        toolchain_digest: request.toolchain_digest.clone(),
        build_target: request.build_target.clone(),
        resolved_lock_digest: pipeline.resolved_lock_digest.clone(),
        test_evidence_digest: test_evidence_digest.to_string(),
    };
    let publisher_request_bytes = serde_json::to_vec_pretty(&publisher_request)
        .map_err(|error| StaticDistributionJobError::Io(error.to_string()))?;
    let publisher_request_path = job_dir.join(PUBLISHER_REQUEST_FILE);
    write_new_or_verify_file(&publisher_request_path, &publisher_request_bytes)?;
    let publisher_request_digest = digest_bytes(&publisher_request_bytes);
    Ok((
        publisher_request,
        publisher_request_path,
        publisher_request_digest,
    ))
}

async fn run_publisher_and_receipt(
    job_dir: &Path,
    workspace: &Path,
    config: &StaticDistributionJobConfig,
    publisher_config: &crate::publisher::StaticDistributionPublisherConfig,
    request: &StaticDistributionJobRequest,
    job_request_digest: &str,
    pipeline: &CargoPipelineEvidence,
) -> Result<StaticDistributionPublicationReceipt, StaticDistributionJobError> {
    let (test_evidence_path, test_evidence_digest) =
        write_test_evidence(job_dir, config, request, job_request_digest, pipeline)?;
    let (publisher_request, publisher_request_path, publisher_request_digest) =
        write_publisher_request(
            job_dir,
            request,
            job_request_digest,
            pipeline,
            &test_evidence_digest,
        )?;
    let publisher_receipt_path = job_dir.join(PUBLISHER_RECEIPT_FILE);
    if !path_entry_exists(&publisher_receipt_path)? {
        run_publisher(
            config,
            &publisher_request_path,
            workspace,
            &test_evidence_path,
            &publisher_receipt_path,
        )
        .await?;
    }
    load_publication_receipt(
        &publisher_receipt_path,
        &publisher_request,
        &publisher_request_digest,
        &test_evidence_digest,
        publisher_config,
    )
}

pub(super) fn create_directory_path(path: &Path) -> Result<(), StaticDistributionJobError> {
    fs::create_dir_all(path).map_err(io_error)?;
    validate_directory(path, "generated directory")
}

pub(super) fn validate_directory(path: &Path, label: &str) -> Result<(), StaticDistributionJobError> {
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
    let mut file = fs::File::open(path).map_err(io_error)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(io_error)?;
    Ok(bytes)
}

pub(super) fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), StaticDistributionJobError> {
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

pub(super) fn digest_bounded_regular(
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

pub(super) fn digest_bytes(bytes: &[u8]) -> String {
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

async fn run_publisher(
    config: &StaticDistributionJobConfig,
    publisher_request: &Path,
    workspace: &Path,
    test_evidence: &Path,
    publisher_receipt: &Path,
) -> Result<(), StaticDistributionJobError> {
    config.validate_runtime()?;
    let mut command = build_publisher_command(
        config,
        publisher_request,
        workspace,
        test_evidence,
        publisher_receipt,
    );
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

fn build_publisher_command(
    config: &StaticDistributionJobConfig,
    publisher_request: &Path,
    workspace: &Path,
    test_evidence: &Path,
    publisher_receipt: &Path,
) -> Command {
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
    command
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
        (&evidence.bundle_reference, &evidence.bundle_root_digest),
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

pub(super) fn write_terminal_receipt(
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

pub(super) fn failed_outcome(
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

pub(super) fn remove_owned_workspace(job_dir: &Path, workspace: &Path) {
    if workspace.is_absolute()
        && workspace.parent() == Some(job_dir)
        && fs::symlink_metadata(workspace)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        let _ = fs::remove_dir_all(workspace);
    }
}

pub(super) fn io_error(error: impl std::fmt::Display) -> StaticDistributionJobError {
    StaticDistributionJobError::Io(error.to_string())
}
