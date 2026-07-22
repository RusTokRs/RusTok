use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use rustok_distribution::{generate_static_distribution, GeneratedStaticDistributionManifest};
use rustok_modules::{
    ModuleStaticDistributionBuildEvidence, ModuleStaticDistributionCompletionOutcome,
    ModuleStaticDistributionExecutor, ModuleStaticDistributionExecutorError,
    ModuleStaticDistributionExecutorReadiness, ModuleStaticDistributionWorkItem,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{process::Command, time::timeout};
use uuid::Uuid;

use crate::StaticDistributionJobConfig;

const JOB_REQUEST_FILE: &str = "job-request.json";
const GENERATED_MANIFEST_FILE: &str = "static-distribution.json";
const CARGO_DEPENDENCIES_FILE: &str = "cargo-dependencies.toml";
const REGISTRY_SOURCE_FILE: &str = "generated-promotions.rs";
const JOB_RECEIPT_FILE: &str = "job-receipt.json";
const MAX_JOB_INPUT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_JOB_RECEIPT_BYTES: u64 = 128 * 1024;
const MAX_REFERENCE_BYTES: usize = 512;
const MAX_FAILURE_CODE_BYTES: usize = 128;
const MAX_FAILURE_DETAIL_BYTES: usize = 2_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticDistributionJobRequest {
    pub contract: String,
    pub distribution_build_id: Uuid,
    pub claim_id: Uuid,
    pub attempt_number: u32,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub generated_output_digest: String,
    pub runner_digest: String,
    pub job_config_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub work_item: ModuleStaticDistributionWorkItem,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticDistributionJobReceipt {
    pub contract: String,
    pub distribution_build_id: Uuid,
    pub claim_id: Uuid,
    pub attempt_number: u32,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub generated_output_digest: String,
    pub job_request_digest: String,
    pub runner_digest: String,
    pub job_config_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub outcome: ModuleStaticDistributionCompletionOutcome,
}

pub struct StaticDistributionWorker {
    launcher_path: PathBuf,
    launcher_digest: String,
    job_config_path: PathBuf,
    job_config_digest: String,
    work_root: PathBuf,
    toolchain_digest: String,
    build_target: String,
    execution_timeout: Duration,
    active_jobs: Arc<Mutex<HashSet<PathBuf>>>,
}

impl StaticDistributionWorker {
    pub fn from_env(execution_timeout: Duration) -> Result<Self, String> {
        let launcher_path = required_absolute_path("RUSTOK_STATIC_DISTRIBUTION_JOB_LAUNCHER")?;
        let launcher_digest = required_digest("RUSTOK_STATIC_DISTRIBUTION_JOB_LAUNCHER_DIGEST")?;
        let job_config_path = required_absolute_path("RUSTOK_STATIC_DISTRIBUTION_JOB_CONFIG")?;
        let job_config_digest = required_digest("RUSTOK_STATIC_DISTRIBUTION_JOB_CONFIG_DIGEST")?;
        let work_root = required_absolute_path("RUSTOK_STATIC_DISTRIBUTION_WORK_ROOT")?;
        let toolchain_digest = required_digest("RUSTOK_STATIC_DISTRIBUTION_TOOLCHAIN_DIGEST")?;
        let build_target = required_text("RUSTOK_STATIC_DISTRIBUTION_BUILD_TARGET", 128)?;
        Self::new(
            launcher_path,
            launcher_digest,
            job_config_path,
            job_config_digest,
            work_root,
            toolchain_digest,
            build_target,
            execution_timeout,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        launcher_path: PathBuf,
        launcher_digest: String,
        job_config_path: PathBuf,
        job_config_digest: String,
        work_root: PathBuf,
        toolchain_digest: String,
        build_target: String,
        execution_timeout: Duration,
    ) -> Result<Self, String> {
        if !launcher_path.is_absolute()
            || !job_config_path.is_absolute()
            || !work_root.is_absolute()
            || !valid_digest(&launcher_digest)
            || !valid_digest(&job_config_digest)
            || !valid_digest(&toolchain_digest)
            || !valid_text(&build_target, 128)
            || execution_timeout.is_zero()
        {
            return Err("static distribution worker configuration is invalid".to_string());
        }
        validate_regular_file(&launcher_path, "job launcher")?;
        validate_regular_file(&job_config_path, "job config")?;
        validate_directory(&work_root, "work root")?;
        let launcher_path = canonical_path(&launcher_path, "job launcher")?;
        let job_config_path = canonical_path(&job_config_path, "job config")?;
        let work_root = canonical_path(&work_root, "work root")?;
        verify_file_digest(&launcher_path, &launcher_digest, "job launcher")?;
        verify_file_digest(&job_config_path, &job_config_digest, "job config")?;
        let job_config = StaticDistributionJobConfig::load(&job_config_path, &job_config_digest)
            .map_err(|error| error.to_string())?;
        if job_config.toolchain_digest != toolchain_digest
            || job_config.build_target != build_target
            || job_config.command_timeout() > execution_timeout
        {
            return Err(
                "static distribution worker and job config execution identities differ".to_string(),
            );
        }
        Ok(Self {
            launcher_path,
            launcher_digest,
            job_config_path,
            job_config_digest,
            work_root,
            toolchain_digest,
            build_target,
            execution_timeout,
            active_jobs: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    fn validate_runtime(&self) -> Result<(), String> {
        validate_regular_file(&self.launcher_path, "job launcher")?;
        validate_regular_file(&self.job_config_path, "job config")?;
        validate_directory(&self.work_root, "work root")?;
        verify_file_digest(&self.launcher_path, &self.launcher_digest, "job launcher")?;
        verify_file_digest(&self.job_config_path, &self.job_config_digest, "job config")?;
        let job_config =
            StaticDistributionJobConfig::load(&self.job_config_path, &self.job_config_digest)
                .map_err(|error| error.to_string())?;
        if job_config.toolchain_digest != self.toolchain_digest
            || job_config.build_target != self.build_target
            || job_config.command_timeout() > self.execution_timeout
        {
            return Err(
                "static distribution worker and job config execution identities differ".to_string(),
            );
        }
        Ok(())
    }

    async fn execute_inner(
        &self,
        work_item: ModuleStaticDistributionWorkItem,
    ) -> Result<ModuleStaticDistributionCompletionOutcome, ModuleStaticDistributionExecutorError>
    {
        work_item
            .validate()
            .map_err(|error| ModuleStaticDistributionExecutorError::Rejected(error.to_string()))?;
        if work_item.build.toolchain_digest != self.toolchain_digest
            || work_item.build.build_target != self.build_target
        {
            return Err(ModuleStaticDistributionExecutorError::Rejected(
                "work item does not match the deployment-pinned toolchain and target".to_string(),
            ));
        }
        self.validate_runtime()
            .map_err(ModuleStaticDistributionExecutorError::Transport)?;
        let generated = generate_static_distribution(&work_item)
            .map_err(|error| ModuleStaticDistributionExecutorError::Rejected(error.to_string()))?;
        let request = StaticDistributionJobRequest {
            contract: "rustok.static_distribution.job".to_string(),
            distribution_build_id: work_item.build.distribution_build_id,
            claim_id: work_item.claim_id,
            attempt_number: work_item.attempt_number,
            composition_revision: work_item.build.composition_revision,
            composition_digest: work_item.build.composition_digest.clone(),
            generated_output_digest: generated.manifest.output_digest.clone(),
            runner_digest: self.launcher_digest.clone(),
            job_config_digest: self.job_config_digest.clone(),
            toolchain_digest: self.toolchain_digest.clone(),
            build_target: self.build_target.clone(),
            work_item,
        };
        let request_bytes = serde_json::to_vec_pretty(&request).map_err(transport_error)?;
        if request_bytes.len() as u64 > MAX_JOB_INPUT_BYTES {
            return Err(ModuleStaticDistributionExecutorError::Rejected(
                "static distribution job request exceeds the input bound".to_string(),
            ));
        }
        let request_digest = digest_bytes(&request_bytes);
        let job_dir = self.work_root.join(format!(
            "{}-{}-{}",
            request.distribution_build_id, request.attempt_number, request.claim_id
        ));
        prepare_job_directory(&self.work_root, &job_dir)
            .map_err(ModuleStaticDistributionExecutorError::Transport)?;
        let _active_job = ActiveJobGuard::acquire(self.active_jobs.clone(), job_dir.clone())
            .map_err(ModuleStaticDistributionExecutorError::Transport)?;
        let request_path = job_dir.join(JOB_REQUEST_FILE);
        let generated_manifest_path = job_dir.join(GENERATED_MANIFEST_FILE);
        let cargo_dependencies_path = job_dir.join(CARGO_DEPENDENCIES_FILE);
        let registry_source_path = job_dir.join(REGISTRY_SOURCE_FILE);
        let receipt_path = job_dir.join(JOB_RECEIPT_FILE);
        write_new_or_verify(&request_path, &request_bytes, MAX_JOB_INPUT_BYTES)
            .map_err(ModuleStaticDistributionExecutorError::Transport)?;
        write_new_or_verify(
            &generated_manifest_path,
            &generated.manifest_json,
            MAX_JOB_INPUT_BYTES,
        )
        .map_err(ModuleStaticDistributionExecutorError::Transport)?;
        write_new_or_verify(
            &cargo_dependencies_path,
            generated.cargo_dependencies_toml.as_bytes(),
            MAX_JOB_INPUT_BYTES,
        )
        .map_err(ModuleStaticDistributionExecutorError::Transport)?;
        write_new_or_verify(
            &registry_source_path,
            generated.registry_source.as_bytes(),
            MAX_JOB_INPUT_BYTES,
        )
        .map_err(ModuleStaticDistributionExecutorError::Transport)?;

        if path_entry_exists(&receipt_path)
            .map_err(ModuleStaticDistributionExecutorError::Transport)?
        {
            return load_and_validate_receipt(
                &receipt_path,
                &request,
                &request_digest,
                &generated.manifest,
            );
        }

        let mut command = Command::new(&self.launcher_path);
        command
            .arg("--job-request")
            .arg(&request_path)
            .arg("--generated-manifest")
            .arg(&generated_manifest_path)
            .arg("--cargo-dependencies")
            .arg(&cargo_dependencies_path)
            .arg("--registry-source")
            .arg(&registry_source_path)
            .arg("--job-config")
            .arg(&self.job_config_path)
            .arg("--receipt")
            .arg(&receipt_path)
            .current_dir(&job_dir)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let status = timeout(self.execution_timeout, command.status())
            .await
            .map_err(|_| {
                ModuleStaticDistributionExecutorError::Transport(
                    "static distribution job launcher timed out".to_string(),
                )
            })?
            .map_err(transport_error)?;
        if !status.success() {
            return Err(ModuleStaticDistributionExecutorError::Transport(format!(
                "static distribution job launcher exited with status {status}"
            )));
        }
        load_and_validate_receipt(
            &receipt_path,
            &request,
            &request_digest,
            &generated.manifest,
        )
    }
}

struct ActiveJobGuard {
    active_jobs: Arc<Mutex<HashSet<PathBuf>>>,
    job_dir: PathBuf,
}

impl ActiveJobGuard {
    fn acquire(
        active_jobs: Arc<Mutex<HashSet<PathBuf>>>,
        job_dir: PathBuf,
    ) -> Result<Self, String> {
        let mut active = active_jobs
            .lock()
            .map_err(|_| "static distribution active-job state is unavailable".to_string())?;
        if !active.insert(job_dir.clone()) {
            return Err("static distribution job attempt is already running".to_string());
        }
        drop(active);
        Ok(Self {
            active_jobs,
            job_dir,
        })
    }
}

impl Drop for ActiveJobGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_jobs.lock() {
            active.remove(&self.job_dir);
        }
    }
}

#[async_trait]
impl ModuleStaticDistributionExecutor for StaticDistributionWorker {
    async fn execute(
        &self,
        work_item: ModuleStaticDistributionWorkItem,
    ) -> Result<ModuleStaticDistributionCompletionOutcome, ModuleStaticDistributionExecutorError>
    {
        self.execute_inner(work_item).await
    }
}

impl ModuleStaticDistributionExecutorReadiness for StaticDistributionWorker {
    fn is_ready(&self) -> bool {
        self.validate_runtime().is_ok()
    }
}

fn load_and_validate_receipt(
    path: &Path,
    request: &StaticDistributionJobRequest,
    request_digest: &str,
    manifest: &GeneratedStaticDistributionManifest,
) -> Result<ModuleStaticDistributionCompletionOutcome, ModuleStaticDistributionExecutorError> {
    let bytes = read_bounded_regular(path, MAX_JOB_RECEIPT_BYTES)
        .map_err(ModuleStaticDistributionExecutorError::Transport)?;
    let receipt: StaticDistributionJobReceipt =
        serde_json::from_slice(&bytes).map_err(transport_error)?;
    if receipt.contract != "rustok.static_distribution.job_receipt"
        || receipt.distribution_build_id != request.distribution_build_id
        || receipt.claim_id != request.claim_id
        || receipt.attempt_number != request.attempt_number
        || receipt.composition_revision != request.composition_revision
        || receipt.composition_digest != request.composition_digest
        || receipt.generated_output_digest != manifest.output_digest
        || receipt.generated_output_digest != request.generated_output_digest
        || receipt.job_request_digest != request_digest
        || receipt.runner_digest != request.runner_digest
        || receipt.job_config_digest != request.job_config_digest
        || receipt.toolchain_digest != request.toolchain_digest
        || receipt.build_target != request.build_target
        || validate_outcome(&receipt.outcome).is_err()
    {
        return Err(ModuleStaticDistributionExecutorError::Transport(
            "static distribution job receipt does not match the immutable request".to_string(),
        ));
    }
    Ok(receipt.outcome)
}

fn validate_outcome(outcome: &ModuleStaticDistributionCompletionOutcome) -> Result<(), ()> {
    match outcome {
        ModuleStaticDistributionCompletionOutcome::Succeeded { evidence } => {
            validate_evidence(evidence)
        }
        ModuleStaticDistributionCompletionOutcome::Failed {
            failure_code,
            failure_detail,
        }
        | ModuleStaticDistributionCompletionOutcome::Cancelled {
            failure_code,
            failure_detail,
        } => {
            if valid_text(failure_code, MAX_FAILURE_CODE_BYTES)
                && valid_text(failure_detail, MAX_FAILURE_DETAIL_BYTES)
            {
                Ok(())
            } else {
                Err(())
            }
        }
    }
}

pub(crate) fn validate_evidence(
    evidence: &ModuleStaticDistributionBuildEvidence,
) -> Result<(), ()> {
    for (reference, digest) in [
        (&evidence.artifact_reference, &evidence.artifact_digest),
        (&evidence.sbom_reference, &evidence.sbom_digest),
        (&evidence.provenance_reference, &evidence.provenance_digest),
        (&evidence.signature_reference, &evidence.signature_digest),
        (
            &evidence.test_evidence_reference,
            &evidence.test_evidence_digest,
        ),
    ] {
        if !valid_text(reference, MAX_REFERENCE_BYTES) || !valid_digest(digest) {
            return Err(());
        }
    }
    Ok(())
}

fn prepare_job_directory(root: &Path, job_dir: &Path) -> Result<(), String> {
    validate_directory(root, "work root")?;
    if job_dir.parent() != Some(root) {
        return Err("static distribution job directory escaped its work root".to_string());
    }
    match fs::create_dir(job_dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_directory(job_dir, "job directory")
        }
        Err(error) => Err(format!(
            "static distribution job directory could not be created: {error}"
        )),
    }
}

fn write_new_or_verify(path: &Path, bytes: &[u8], max_bytes: u64) -> Result<(), String> {
    if bytes.len() as u64 > max_bytes {
        return Err("static distribution job input exceeds its byte bound".to_string());
    }
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(bytes).map_err(|error| {
                format!("static distribution job input could not be written: {error}")
            })?;
            file.sync_all().map_err(|error| {
                format!("static distribution job input could not be synced: {error}")
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = read_bounded_regular(path, max_bytes)?;
            if existing == bytes {
                Ok(())
            } else {
                Err("static distribution job input conflicts with an existing attempt".to_string())
            }
        }
        Err(error) => Err(format!(
            "static distribution job input could not be created: {error}"
        )),
    }
}

fn read_bounded_regular(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("static distribution job file could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes {
        return Err("static distribution job file is not a bounded regular file".to_string());
    }
    fs::read(path)
        .map_err(|error| format!("static distribution job file could not be read: {error}"))
}

fn path_entry_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "static distribution job path could not be inspected: {error}"
        )),
    }
}

fn required_absolute_path(name: &str) -> Result<PathBuf, String> {
    let path =
        PathBuf::from(std::env::var(name).map_err(|_| format!("{name} must be configured"))?);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!("{name} must be an absolute path"))
    }
}

fn required_digest(name: &str) -> Result<String, String> {
    let value = std::env::var(name).map_err(|_| format!("{name} must be configured"))?;
    if valid_digest(&value) {
        Ok(value)
    } else {
        Err(format!("{name} must be a lowercase sha256 digest"))
    }
}

fn required_text(name: &str, max_bytes: usize) -> Result<String, String> {
    let value = std::env::var(name).map_err(|_| format!("{name} must be configured"))?;
    if valid_text(&value, max_bytes) {
        Ok(value)
    } else {
        Err(format!("{name} is invalid"))
    }
}

fn validate_regular_file(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("static distribution {label} could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "static distribution {label} must be a non-symlink file"
        ));
    }
    Ok(())
}

fn validate_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("static distribution {label} could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "static distribution {label} must be a non-symlink directory"
        ));
    }
    Ok(())
}

fn canonical_path(path: &Path, label: &str) -> Result<PathBuf, String> {
    fs::canonicalize(path)
        .map_err(|error| format!("static distribution {label} could not be resolved: {error}"))
}

fn verify_file_digest(path: &Path, expected: &str, label: &str) -> Result<(), String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("static distribution {label} could not be opened: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("static distribution {label} could not be hashed: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("sha256:{}", hex::encode(hasher.finalize()));
    if actual == expected {
        Ok(())
    } else {
        Err(format!("static distribution {label} digest mismatch"))
    }
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

fn transport_error(error: impl std::fmt::Display) -> ModuleStaticDistributionExecutorError {
    ModuleStaticDistributionExecutorError::Transport(error.to_string())
}
