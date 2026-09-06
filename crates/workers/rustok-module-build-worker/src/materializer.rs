use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rustok_modules::{ModuleBuildNetworkPolicy, ModuleBuildRequest};
use serde::Deserialize;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};

const MATERIALIZER_PROTOCOL_VERSION: u32 = 1;
const MAX_CACHE_ENTRIES: usize = 65_536;

/// Deployment-owned client for a separately isolated OCI dependency
/// materializer. This adapter never lets Cargo in the build worker use egress.
pub struct OciScopedDependencyMaterializer {
    executable_path: PathBuf,
}

#[derive(Debug)]
pub enum DependencyMaterializationError {
    EndpointDenied,
    ResourceLimit,
    Internal(String),
}

impl OciScopedDependencyMaterializer {
    pub fn new(executable_path: PathBuf) -> Result<Self, String> {
        if !executable_path.is_absolute() {
            return Err("module dependency materializer path must be absolute".to_string());
        }
        let metadata = fs::symlink_metadata(&executable_path).map_err(|error| {
            format!(
                "module dependency materializer {} cannot be inspected: {error}",
                executable_path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(
                "module dependency materializer must be a regular non-symlink file".to_string(),
            );
        }
        Ok(Self { executable_path })
    }

    pub async fn materialize(
        &self,
        source_dir: &Path,
        job_dir: &Path,
        request: &ModuleBuildRequest,
        execution_timeout: Duration,
    ) -> Result<PathBuf, DependencyMaterializationError> {
        let ModuleBuildNetworkPolicy::ScopedDependencyMaterialization { endpoints } =
            &request.network_policy
        else {
            return Err(DependencyMaterializationError::EndpointDenied);
        };
        if execution_timeout.is_zero() {
            return Err(DependencyMaterializationError::ResourceLimit);
        }
        let cargo_home = job_dir.join("materialized-cargo-home");
        prepare_empty_cache(&cargo_home)?;
        let request_json = serde_json::to_vec(request)
            .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?;
        let endpoints_json = serde_json::to_string(endpoints)
            .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?;
        let budget = Arc::new(MaterializerOutputBudget::new(request.limits.output_bytes));
        let mut child = Command::new(&self.executable_path)
            .current_dir(job_dir)
            .env_clear()
            .env(
                "RUSTOK_MODULE_DEPENDENCY_MATERIALIZER_PROTOCOL_VERSION",
                MATERIALIZER_PROTOCOL_VERSION.to_string(),
            )
            .env(
                "RUSTOK_MODULE_DEPENDENCY_MATERIALIZER_SOURCE_DIR",
                source_dir,
            )
            .env(
                "RUSTOK_MODULE_DEPENDENCY_MATERIALIZER_CARGO_HOME",
                &cargo_home,
            )
            .env(
                "RUSTOK_MODULE_DEPENDENCY_MATERIALIZER_ENDPOINTS_JSON",
                endpoints_json,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            DependencyMaterializationError::Internal(
                "dependency materializer stdin is unavailable".to_string(),
            )
        })?;
        stdin
            .write_all(&request_json)
            .await
            .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?;
        drop(stdin);
        let stdout = child.stdout.take().ok_or_else(|| {
            DependencyMaterializationError::Internal(
                "dependency materializer stdout is unavailable".to_string(),
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            DependencyMaterializationError::Internal(
                "dependency materializer stderr is unavailable".to_string(),
            )
        })?;
        let stdout_task = tokio::spawn(read_materializer_output(stdout, Arc::clone(&budget), true));
        let stderr_task = tokio::spawn(read_materializer_output(stderr, budget, false));
        let status = match timeout(execution_timeout, child.wait()).await {
            Ok(status) => status
                .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?,
            Err(_) => {
                let _ = child.kill().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(DependencyMaterializationError::ResourceLimit);
            }
        };
        let stdout = collect_materializer_output(stdout_task).await?;
        collect_materializer_output(stderr_task).await?;
        if !status.success() {
            return Err(DependencyMaterializationError::Internal(format!(
                "dependency materializer exited with {status}"
            )));
        }
        let receipt: MaterializationReceipt = serde_json::from_slice(&stdout).map_err(|_| {
            DependencyMaterializationError::Internal(
                "dependency materializer returned invalid receipt".to_string(),
            )
        })?;
        if receipt.protocol_version != MATERIALIZER_PROTOCOL_VERSION
            || receipt.source_digest != request.source.digest
            || receipt.dependency_lock_digest != request.dependency_policy.lock_digest
            || receipt.endpoints.as_slice() != endpoints.as_slice()
        {
            return Err(DependencyMaterializationError::EndpointDenied);
        }
        if !matches!(receipt.outcome, MaterializationOutcome::Materialized) {
            return Err(DependencyMaterializationError::EndpointDenied);
        }
        validate_materialized_cache(&cargo_home, request.limits.disk_bytes)?;
        Ok(cargo_home)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum MaterializationOutcome {
    Materialized,
    EndpointDenied,
}

#[derive(Deserialize)]
struct MaterializationReceipt {
    protocol_version: u32,
    source_digest: String,
    dependency_lock_digest: String,
    endpoints: Vec<String>,
    outcome: MaterializationOutcome,
}

fn prepare_empty_cache(path: &Path) -> Result<(), DependencyMaterializationError> {
    if path.exists() {
        return Err(DependencyMaterializationError::Internal(format!(
            "dependency materializer cache path {} already exists",
            path.display()
        )));
    }
    fs::create_dir(path)
        .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))
}

fn validate_materialized_cache(
    path: &Path,
    maximum_bytes: u64,
) -> Result<(), DependencyMaterializationError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(DependencyMaterializationError::EndpointDenied);
    }
    for restricted_name in ["config", "config.toml", "credentials", "credentials.toml"] {
        if path.join(restricted_name).exists() {
            return Err(DependencyMaterializationError::EndpointDenied);
        }
    }
    let mut pending = vec![path.to_path_buf()];
    let mut entries = 0_usize;
    let mut bytes = 0_u64;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?
        {
            let entry = entry
                .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?;
            entries = entries
                .checked_add(1)
                .ok_or(DependencyMaterializationError::ResourceLimit)?;
            if entries > MAX_CACHE_ENTRIES {
                return Err(DependencyMaterializationError::ResourceLimit);
            }
            let entry_path = entry.path();
            let entry_metadata = fs::symlink_metadata(&entry_path)
                .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?;
            if entry_metadata.file_type().is_symlink() {
                return Err(DependencyMaterializationError::EndpointDenied);
            }
            if entry_metadata.is_dir() {
                pending.push(entry_path);
            } else if entry_metadata.is_file() {
                bytes = bytes
                    .checked_add(entry_metadata.len())
                    .ok_or(DependencyMaterializationError::ResourceLimit)?;
                if bytes > maximum_bytes {
                    return Err(DependencyMaterializationError::ResourceLimit);
                }
            } else {
                return Err(DependencyMaterializationError::EndpointDenied);
            }
        }
    }
    Ok(())
}

struct MaterializerOutputBudget {
    limit: u64,
    consumed: AtomicU64,
}

impl MaterializerOutputBudget {
    fn new(limit: u64) -> Self {
        Self {
            limit,
            consumed: AtomicU64::new(0),
        }
    }

    fn reserve(&self, bytes: usize) -> bool {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        let previous = self.consumed.fetch_add(bytes, Ordering::Relaxed);
        previous.saturating_add(bytes) <= self.limit
    }
}

async fn read_materializer_output<R>(
    mut reader: R,
    budget: Arc<MaterializerOutputBudget>,
    retain: bool,
) -> Result<Vec<u8>, DependencyMaterializationError>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut exceeded = false;
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| DependencyMaterializationError::Internal(error.to_string()))?;
        if read == 0 {
            return if exceeded {
                Err(DependencyMaterializationError::ResourceLimit)
            } else {
                Ok(output)
            };
        }
        if !budget.reserve(read) {
            exceeded = true;
        } else if retain {
            output.extend_from_slice(&buffer[..read]);
        }
    }
}

async fn collect_materializer_output(
    task: tokio::task::JoinHandle<Result<Vec<u8>, DependencyMaterializationError>>,
) -> Result<Vec<u8>, DependencyMaterializationError> {
    task.await.map_err(|error| {
        DependencyMaterializationError::Internal(format!(
            "dependency materializer output reader failed: {error}"
        ))
    })?
}
