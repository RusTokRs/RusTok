//! Deployment isolation policy for the standalone Rhai worker.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rustok_sandbox::{
    SandboxExecutor, SandboxExecutorKind, SandboxHost, SandboxLimits, SandboxOutcome,
    SandboxRequest, SandboxResult,
};
use rustok_sandbox_transport::SandboxWorkerReadiness;
use serde::Deserialize;

const MAX_ATTESTATION_BYTES: u64 = 16 * 1024;
const MIN_CPU_MILLIS: u32 = 100;
const MAX_CPU_MILLIS: u32 = 4_000;
const MIN_MEMORY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const MIN_PIDS: u32 = 4;
const MAX_PIDS: u32 = 64;
const MIN_EPHEMERAL_STORAGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EPHEMERAL_STORAGE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_WALL_CLOCK_MS: u64 = 30_000;
const CGROUP_MEMORY_CURRENT_PATH: &str = "/sys/fs/cgroup/memory.current";
const MEMORY_SAMPLE_INTERVAL: Duration = Duration::from_millis(1);

trait MemoryProbe: Send + Sync {
    fn current_bytes(&self) -> Result<u64, String>;
}

struct CgroupMemoryProbe;

impl CgroupMemoryProbe {
    fn new() -> Result<Self, String> {
        let probe = Self;
        probe.current_bytes()?;
        Ok(probe)
    }
}

impl MemoryProbe for CgroupMemoryProbe {
    fn current_bytes(&self) -> Result<u64, String> {
        let path = Path::new(CGROUP_MEMORY_CURRENT_PATH);
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|_| "sandbox worker cgroup memory observer is unavailable".to_string())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("sandbox worker cgroup memory observer is invalid".to_string());
        }
        let value = std::fs::read_to_string(path)
            .map_err(|_| "sandbox worker cgroup memory observation failed".to_string())?;
        value
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| "sandbox worker cgroup memory observation is invalid".to_string())
    }
}

/// Deployment-local observer for the worker cgroup. The same observer is
/// shared by readiness and execution so a worker cannot remain ready after its
/// measured-resource evidence disappears.
#[derive(Clone)]
pub struct WorkerMemoryObserver {
    probe: Arc<dyn MemoryProbe>,
}

impl WorkerMemoryObserver {
    pub fn cgroup_v2() -> Result<Self, String> {
        Ok(Self {
            probe: Arc::new(CgroupMemoryProbe::new()?),
        })
    }

    fn current_bytes(&self) -> Result<u64, String> {
        self.probe.current_bytes()
    }

    #[cfg(test)]
    fn with_probe(probe: Arc<dyn MemoryProbe>) -> Self {
        Self { probe }
    }
}

/// Adds request-bounded observed cgroup memory evidence to the neutral Rhai
/// outcome. The worker has one execution permit, so the sampled cgroup belongs
/// to at most one guest request at a time.
pub struct ObservedRhaiExecutor<E> {
    inner: E,
    memory: WorkerMemoryObserver,
}

impl<E> ObservedRhaiExecutor<E> {
    pub fn new(inner: E, memory: WorkerMemoryObserver) -> Self {
        Self { inner, memory }
    }
}

/// Extends the deployment isolation check with the same cgroup observation
/// used to report request peak memory.
pub struct ObservedWorkerReadiness<R> {
    inner: R,
    memory: WorkerMemoryObserver,
}

impl<R> ObservedWorkerReadiness<R> {
    pub fn new(inner: R, memory: WorkerMemoryObserver) -> Self {
        Self { inner, memory }
    }
}

#[async_trait]
impl<R> SandboxWorkerReadiness for ObservedWorkerReadiness<R>
where
    R: SandboxWorkerReadiness,
{
    async fn check_readiness(&self) -> Result<(), String> {
        self.memory.current_bytes().map(|_| ())?;
        self.inner.check_readiness().await
    }

    async fn admit_limits(&self, limits: &SandboxLimits) -> Result<(), String> {
        self.memory.current_bytes().map(|_| ())?;
        self.inner.admit_limits(limits).await
    }
}

#[async_trait]
impl<E> SandboxExecutor for ObservedRhaiExecutor<E>
where
    E: SandboxExecutor,
{
    fn kind(&self) -> SandboxExecutorKind {
        self.inner.kind()
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        let initial = observed_memory(&self.memory)?;
        let peak = Arc::new(AtomicU64::new(initial));
        let failed = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let sampler = {
            let memory = self.memory.clone();
            let peak = Arc::clone(&peak);
            let failed = Arc::clone(&failed);
            let stop = Arc::clone(&stop);
            tokio::spawn(async move {
                while !stop.load(Ordering::Acquire) {
                    match memory.current_bytes() {
                        Ok(value) => update_peak(&peak, value),
                        Err(_) => {
                            failed.store(true, Ordering::Release);
                            return;
                        }
                    }
                    tokio::time::sleep(MEMORY_SAMPLE_INTERVAL).await;
                }
            })
        };

        let result = self.inner.execute(request, host).await;
        stop.store(true, Ordering::Release);
        if sampler.await.is_err() {
            failed.store(true, Ordering::Release);
        }
        match self.memory.current_bytes() {
            Ok(value) => update_peak(&peak, value),
            Err(_) => failed.store(true, Ordering::Release),
        }
        if failed.load(Ordering::Acquire) {
            return Err(rustok_sandbox::SandboxError::Internal(
                "sandbox worker memory observation unavailable".to_string(),
            ));
        }

        let mut outcome = result?;
        outcome.metrics.peak_memory_bytes = Some(peak.load(Ordering::Acquire));
        Ok(outcome)
    }
}

fn observed_memory(memory: &WorkerMemoryObserver) -> SandboxResult<u64> {
    memory.current_bytes().map_err(|_| {
        rustok_sandbox::SandboxError::Internal(
            "sandbox worker memory observation unavailable".to_string(),
        )
    })
}

fn update_peak(peak: &AtomicU64, value: u64) {
    let mut current = peak.load(Ordering::Acquire);
    while value > current {
        match peak.compare_exchange_weak(current, value, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HardenedRuntime {
    Gvisor,
    Kata,
}

impl HardenedRuntime {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gvisor" => Ok(Self::Gvisor),
            "kata" => Ok(Self::Kata),
            _ => Err("RUSTOK_SANDBOX_RUNTIME must be one of: gvisor, kata".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Gvisor => "gvisor",
            Self::Kata => "kata",
        }
    }
}

/// Deployment-owned proof that the worker process is placed in a hardened,
/// RPC-only container with denied egress and finite process-level resources.
/// The worker validates the mounted proof at startup and on every readiness
/// probe.
pub struct IsolationPolicy {
    runtime: HardenedRuntime,
    image_digest: String,
    attestation_path: PathBuf,
}

impl IsolationPolicy {
    pub fn from_env() -> Result<Self, String> {
        let runtime = std::env::var("RUSTOK_SANDBOX_RUNTIME")
            .map_err(|_| "RUSTOK_SANDBOX_RUNTIME must be configured".to_string())?;
        let image_digest = std::env::var("RUSTOK_SANDBOX_IMAGE_DIGEST")
            .map_err(|_| "RUSTOK_SANDBOX_IMAGE_DIGEST must be configured".to_string())?;
        let attestation_path = std::env::var("RUSTOK_SANDBOX_ISOLATION_ATTESTATION")
            .map(PathBuf::from)
            .map_err(|_| "RUSTOK_SANDBOX_ISOLATION_ATTESTATION must be configured".to_string())?;
        Self::load(
            HardenedRuntime::parse(&runtime)?,
            image_digest,
            attestation_path,
        )
    }

    fn load(
        runtime: HardenedRuntime,
        image_digest: String,
        attestation_path: PathBuf,
    ) -> Result<Self, String> {
        if !is_sha256_digest(&image_digest) {
            return Err("RUSTOK_SANDBOX_IMAGE_DIGEST must be a sha256 digest".to_string());
        }
        load_attestation(&attestation_path, runtime, &image_digest)?;
        Ok(Self {
            runtime,
            image_digest,
            attestation_path,
        })
    }

    fn validate_current(&self) -> Result<IsolationAttestation, String> {
        load_attestation(&self.attestation_path, self.runtime, &self.image_digest)
    }
}

#[async_trait]
impl SandboxWorkerReadiness for IsolationPolicy {
    async fn check_readiness(&self) -> Result<(), String> {
        self.validate_current().map(|_| ())
    }

    async fn admit_limits(&self, limits: &SandboxLimits) -> Result<(), String> {
        let attestation = self.validate_current()?;
        if limits.wall_clock_ms == 0
            || limits.wall_clock_ms > attestation.resource_limits.wall_clock_ms
            || limits.max_memory_bytes == 0
            || limits.max_memory_bytes > attestation.resource_limits.memory_bytes
            || limits.max_output_bytes == 0
            || limits.max_output_bytes > attestation.resource_limits.file_bytes
            || limits.max_concurrency != 1
        {
            return Err("sandbox limits exceed the deployment isolation envelope".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IsolationAttestation {
    protocol_revision: u32,
    runtime: String,
    image_digest: String,
    privileged: bool,
    host_mounts: bool,
    container_socket: bool,
    host_pid: bool,
    host_network: bool,
    network_mode: String,
    ingress_mode: String,
    egress_denied: bool,
    database_access: bool,
    secret_access: bool,
    read_only_root: bool,
    resource_limits: ResourceLimits,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceLimits {
    cpu_millis: u32,
    memory_bytes: u64,
    pids: u32,
    ephemeral_storage_bytes: u64,
    file_bytes: u64,
    wall_clock_ms: u64,
}

fn load_attestation(
    path: &Path,
    runtime: HardenedRuntime,
    image_digest: &str,
) -> Result<IsolationAttestation, String> {
    if !path.is_absolute() {
        return Err("sandbox isolation attestation path must be absolute".to_string());
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("sandbox isolation attestation cannot be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("sandbox isolation attestation must be a regular file".to_string());
    }
    if metadata.len() == 0 || metadata.len() > MAX_ATTESTATION_BYTES {
        return Err("sandbox isolation attestation size is invalid".to_string());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("sandbox isolation attestation cannot be read: {error}"))?;
    let attestation: IsolationAttestation = serde_json::from_slice(&bytes)
        .map_err(|error| format!("sandbox isolation attestation is invalid JSON: {error}"))?;
    if attestation.protocol_revision != 1
        || attestation.runtime != runtime.as_str()
        || attestation.image_digest != image_digest
        || !is_sha256_digest(&attestation.image_digest)
        || attestation.privileged
        || attestation.host_mounts
        || attestation.container_socket
        || attestation.host_pid
        || attestation.host_network
        || attestation.network_mode != "rpc_only"
        || attestation.ingress_mode != "mtls_grpc"
        || !attestation.egress_denied
        || attestation.database_access
        || attestation.secret_access
        || !attestation.read_only_root
        || !attestation.resource_limits.is_bounded()
    {
        return Err(
            "sandbox isolation attestation does not match the hardened worker policy".to_string(),
        );
    }
    Ok(attestation)
}

impl ResourceLimits {
    fn is_bounded(&self) -> bool {
        (MIN_CPU_MILLIS..=MAX_CPU_MILLIS).contains(&self.cpu_millis)
            && (MIN_MEMORY_BYTES..=MAX_MEMORY_BYTES).contains(&self.memory_bytes)
            && (MIN_PIDS..=MAX_PIDS).contains(&self.pids)
            && (MIN_EPHEMERAL_STORAGE_BYTES..=MAX_EPHEMERAL_STORAGE_BYTES)
                .contains(&self.ephemeral_storage_bytes)
            && (1..=MAX_FILE_BYTES).contains(&self.file_bytes)
            && (1..=MAX_WALL_CLOCK_MS).contains(&self.wall_clock_ms)
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == "sha256:".len() + 64
        && value.starts_with("sha256:")
        && value["sha256:".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use async_trait::async_trait;
    use rustok_sandbox::{
        CapabilityBrokerRouter, ExecutionMetrics, ExecutionPhase, ExecutorRegistry, SandboxContext,
        SandboxExecutor, SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxPayload,
        SandboxPolicy, SandboxRequest, SandboxResult, SandboxRuntime, SandboxSubject,
    };
    use rustok_sandbox_transport::SandboxWorkerReadiness;

    use super::{
        HardenedRuntime, IsolationPolicy, MemoryProbe, ObservedRhaiExecutor,
        ObservedWorkerReadiness, WorkerMemoryObserver,
    };

    struct AtomicMemoryProbe(AtomicU64);

    impl MemoryProbe for AtomicMemoryProbe {
        fn current_bytes(&self) -> Result<u64, String> {
            Ok(self.0.load(Ordering::Acquire))
        }
    }

    struct FailingMemoryProbe;

    impl MemoryProbe for FailingMemoryProbe {
        fn current_bytes(&self) -> Result<u64, String> {
            Err("unavailable".to_string())
        }
    }

    struct MemoryFixtureExecutor(Arc<AtomicMemoryProbe>);

    #[async_trait]
    impl SandboxExecutor for MemoryFixtureExecutor {
        fn kind(&self) -> SandboxExecutorKind {
            SandboxExecutorKind::Rhai
        }

        async fn execute(
            &self,
            request: &SandboxRequest,
            _host: SandboxHost,
        ) -> SandboxResult<SandboxOutcome> {
            self.0.0.store(4_096, Ordering::Release);
            tokio::time::sleep(Duration::from_millis(5)).await;
            self.0.0.store(2_048, Ordering::Release);
            Ok(SandboxOutcome {
                execution_id: request.context.execution_id,
                output: serde_json::Value::Null,
                rhai_scope: None,
                metrics: ExecutionMetrics::default(),
            })
        }
    }

    struct ImmediateExecutor;

    #[async_trait]
    impl SandboxExecutor for ImmediateExecutor {
        fn kind(&self) -> SandboxExecutorKind {
            SandboxExecutorKind::Rhai
        }

        async fn execute(
            &self,
            request: &SandboxRequest,
            _host: SandboxHost,
        ) -> SandboxResult<SandboxOutcome> {
            Ok(SandboxOutcome {
                execution_id: request.context.execution_id,
                output: serde_json::Value::Null,
                rhai_scope: None,
                metrics: ExecutionMetrics::default(),
            })
        }
    }

    fn sandbox_request() -> SandboxRequest {
        SandboxRequest {
            subject: SandboxSubject::AlloyDraft {
                draft_id: uuid::Uuid::new_v4(),
                revision: 1,
            },
            context: SandboxContext::new(ExecutionPhase::Test),
            payload: SandboxPayload {
                executor: SandboxExecutorKind::Rhai,
                media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
                digest: format!("sha256:{}", "a".repeat(64)),
                runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
                entrypoint: "main".to_string(),
                bytes: b"fixture".to_vec(),
            },
            input: serde_json::Value::Null,
            rhai_scope: None,
            policy: SandboxPolicy::default(),
        }
    }

    async fn execute_observed(
        executor: impl SandboxExecutor + 'static,
    ) -> SandboxResult<SandboxOutcome> {
        let mut executors = ExecutorRegistry::new();
        executors.register_in_process(executor)?;
        SandboxRuntime::new(executors, Arc::new(CapabilityBrokerRouter::new()))
            .execute(sandbox_request())
            .await
    }

    fn attestation_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "rustok-sandbox-attestation-{}.json",
            uuid::Uuid::new_v4()
        ))
    }

    fn valid_attestation(digest: &str, memory_bytes: u64) -> String {
        format!(
            r#"{{
                "protocol_revision": 1,
                "runtime": "gvisor",
                "image_digest": "{digest}",
                "privileged": false,
                "host_mounts": false,
                "container_socket": false,
                "host_pid": false,
                "host_network": false,
                "network_mode": "rpc_only",
                "ingress_mode": "mtls_grpc",
                "egress_denied": true,
                "database_access": false,
                "secret_access": false,
                "read_only_root": true,
                "resource_limits": {{
                    "cpu_millis": 1000,
                    "memory_bytes": {memory_bytes},
                    "pids": 16,
                    "ephemeral_storage_bytes": 67108864,
                    "file_bytes": 8388608,
                    "wall_clock_ms": 5000
                }}
            }}"#
        )
    }

    #[test]
    fn policy_revalidates_exact_hardened_attestation() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let path = attestation_path();
        std::fs::write(&path, valid_attestation(&digest, 134_217_728)).expect("write attestation");
        let policy = IsolationPolicy::load(HardenedRuntime::Gvisor, digest, path.clone())
            .expect("valid policy");
        assert!(policy.validate_current().is_ok());
        std::fs::remove_file(path).expect("remove attestation");
    }

    #[test]
    fn policy_rejects_unbounded_memory() {
        let digest = format!("sha256:{}", "b".repeat(64));
        let path = attestation_path();
        std::fs::write(&path, valid_attestation(&digest, u64::MAX)).expect("write attestation");
        let result = IsolationPolicy::load(HardenedRuntime::Gvisor, digest, path.clone());
        assert!(result.is_err());
        std::fs::remove_file(path).expect("remove attestation");
    }

    #[tokio::test]
    async fn policy_rejects_request_limits_above_the_attested_envelope() {
        let digest = format!("sha256:{}", "c".repeat(64));
        let path = attestation_path();
        std::fs::write(&path, valid_attestation(&digest, 134_217_728)).expect("write attestation");
        let policy = IsolationPolicy::load(HardenedRuntime::Gvisor, digest, path.clone())
            .expect("valid policy");
        let limits = rustok_sandbox::SandboxLimits {
            max_memory_bytes: 256 * 1024 * 1024,
            ..Default::default()
        };
        assert!(policy.admit_limits(&limits).await.is_err());
        std::fs::remove_file(path).expect("remove attestation");
        assert!(policy.check_readiness().await.is_err());
    }

    #[tokio::test]
    async fn executor_reports_observed_request_peak_memory() {
        let memory = Arc::new(AtomicMemoryProbe(AtomicU64::new(1_024)));
        let observer = WorkerMemoryObserver::with_probe(memory.clone());
        let executor = ObservedRhaiExecutor::new(MemoryFixtureExecutor(memory), observer);

        let outcome = execute_observed(executor)
            .await
            .expect("observed execution");

        assert_eq!(outcome.metrics.peak_memory_bytes, Some(4_096));
    }

    #[tokio::test]
    async fn executor_fails_closed_without_memory_observation() {
        let observer = WorkerMemoryObserver::with_probe(Arc::new(FailingMemoryProbe));
        let executor = ObservedRhaiExecutor::new(ImmediateExecutor, observer);

        let error = execute_observed(executor)
            .await
            .expect_err("missing memory evidence");

        assert_eq!(error.code(), "INTERNAL_ERROR");
    }

    #[tokio::test]
    async fn readiness_fails_closed_without_memory_observation() {
        let digest = format!("sha256:{}", "d".repeat(64));
        let path = attestation_path();
        std::fs::write(&path, valid_attestation(&digest, 134_217_728)).expect("write attestation");
        let policy = IsolationPolicy::load(HardenedRuntime::Gvisor, digest, path.clone())
            .expect("valid policy");
        let readiness = ObservedWorkerReadiness::new(
            policy,
            WorkerMemoryObserver::with_probe(Arc::new(FailingMemoryProbe)),
        );

        assert!(readiness.check_readiness().await.is_err());

        std::fs::remove_file(path).expect("remove attestation");
    }
}
