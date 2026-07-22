use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use rustok_build_publication::{
    CommandRegistryCredentialBroker, CosignArtifactSigner, CosignSigningError,
    RegistryCredentialBroker, RegistryCredentialError,
};
use rustok_modules::{
    ArtifactAdmissionLimits, ModuleBuildDiagnostic, ModuleBuildEvidence, ModuleBuildFailureCode,
    ModuleBuildMetrics, ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildProtocolError,
    ModuleBuildPublicationReceipt, ModuleBuildRequest, ModuleBuildResult,
    ModuleBuildSignatureAuthority, ModuleBuildWorker, ModuleBuildWorkerReadiness,
    OciArtifactPublicationError, OciArtifactPublicationTarget, OciArtifactPublisher,
    OciDistributionArtifactPublisher,
};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};

use crate::{
    BuildEvidenceError, BuildEvidenceInspector, CargoMetadataError, CargoMetadataInspector,
    ComponentArtifactError, ComponentArtifactInspector, DependencyMaterializationError,
    OciScopedDependencyMaterializer, PublicationBundleCollector, PublicationBundleError,
    SourceMaterializationError, SourceMaterializer, SourcePolicyError, SourcePolicyPreflight,
    WitContractError, WitContractInspector,
};

const MAX_PUBLICATION_WINDOW: Duration = Duration::from_secs(14 * 60);
const CREDENTIAL_LEASE_SAFETY_MARGIN: Duration = Duration::from_secs(30);
const MAX_ISOLATION_ATTESTATION_BYTES: u64 = 16 * 1024;

/// Deployment-owned OCI job runtime required for untrusted build execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OciJobRuntime {
    Gvisor,
    Kata,
}

impl OciJobRuntime {
    fn from_env() -> Result<Self, String> {
        match std::env::var("RUSTOK_MODULE_BUILD_JOB_RUNTIME")
            .map_err(|_| "RUSTOK_MODULE_BUILD_JOB_RUNTIME must be configured".to_string())?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "gvisor" => Ok(Self::Gvisor),
            "kata" => Ok(Self::Kata),
            _ => Err("RUSTOK_MODULE_BUILD_JOB_RUNTIME must be one of: gvisor, kata".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Gvisor => "gvisor",
            Self::Kata => "kata",
        }
    }
}

/// Fixed deployment-owned OCI-job launcher. It receives one immutable request
/// on standard input, launches the build in the configured hardened runtime,
/// and returns exactly one JSON `ModuleBuildResult` on standard output. It is
/// never selected by request data.
pub struct OciJobBuildWorker {
    job_launcher_path: PathBuf,
    job_runtime: OciJobRuntime,
    job_image_digest: String,
    cargo_metadata: CargoMetadataInspector,
    source_materializer: SourceMaterializer,
    dependency_materializer: Option<OciScopedDependencyMaterializer>,
    wit_contract: WitContractInspector,
    publication_target: OciArtifactPublicationTarget,
    registry_credentials: Arc<dyn RegistryCredentialBroker>,
    signer: CosignArtifactSigner,
    request_timeout: Duration,
    isolation_attestation: Option<OciJobIsolationAttestation>,
}

#[derive(Debug, Clone)]
struct OciJobIsolationAttestation {
    runtime: String,
    image_digest: String,
    privileged: bool,
    host_mounts: bool,
    container_socket: bool,
    host_pid: bool,
    host_network: bool,
    network_mode: String,
    resource_limits: bool,
    ephemeral_job: bool,
}

impl OciJobBuildWorker {
    pub fn from_env(request_timeout: Duration) -> Result<Self, String> {
        let job_launcher_path = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_JOB_LAUNCHER")
                .map_err(|_| "RUSTOK_MODULE_BUILD_JOB_LAUNCHER must be configured".to_string())?,
        );
        let job_runtime = OciJobRuntime::from_env()?;
        let job_image_digest = std::env::var("RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST")
            .map_err(|_| "RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST must be configured".to_string())?;
        if !is_sha256_digest(&job_image_digest) {
            return Err("RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST must be a sha256 digest".to_string());
        }
        let workdir = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_WORKDIR")
                .map_err(|_| "RUSTOK_MODULE_BUILD_WORKDIR must be configured".to_string())?,
        );
        let source_root = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_SOURCE_ROOT")
                .map_err(|_| "RUSTOK_MODULE_BUILD_SOURCE_ROOT must be configured".to_string())?,
        );
        let cargo_path = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_CARGO")
                .map_err(|_| "RUSTOK_MODULE_BUILD_CARGO must be configured".to_string())?,
        );
        let cargo_home = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_CARGO_HOME")
                .map_err(|_| "RUSTOK_MODULE_BUILD_CARGO_HOME must be configured".to_string())?,
        );
        let wasm_tools_path = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_WASM_TOOLS")
                .map_err(|_| "RUSTOK_MODULE_BUILD_WASM_TOOLS must be configured".to_string())?,
        );
        let dependency_materializer = std::env::var("RUSTOK_MODULE_BUILD_DEPENDENCY_MATERIALIZER")
            .ok()
            .map(PathBuf::from)
            .map(OciScopedDependencyMaterializer::new)
            .transpose()?;
        let publication_target = OciArtifactPublicationTarget {
            registry: std::env::var("RUSTOK_MODULE_BUILD_PUBLICATION_REGISTRY").map_err(|_| {
                "RUSTOK_MODULE_BUILD_PUBLICATION_REGISTRY must be configured".to_string()
            })?,
            repository: std::env::var("RUSTOK_MODULE_BUILD_PUBLICATION_REPOSITORY").map_err(
                |_| "RUSTOK_MODULE_BUILD_PUBLICATION_REPOSITORY must be configured".to_string(),
            )?,
        };
        publication_target
            .validate()
            .map_err(|error| error.to_string())?;
        let credential_broker_path = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_REGISTRY_CREDENTIAL_BROKER").map_err(|_| {
                "RUSTOK_MODULE_BUILD_REGISTRY_CREDENTIAL_BROKER must be configured".to_string()
            })?,
        );
        let credential_broker_digest = std::env::var(
            "RUSTOK_MODULE_BUILD_REGISTRY_CREDENTIAL_BROKER_DIGEST",
        )
        .map_err(|_| {
            "RUSTOK_MODULE_BUILD_REGISTRY_CREDENTIAL_BROKER_DIGEST must be configured".to_string()
        })?;
        let registry_credentials = Arc::new(CommandRegistryCredentialBroker::new(
            credential_broker_path,
            credential_broker_digest,
        )?);
        let cosign_path = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_COSIGN_PROGRAM")
                .map_err(|_| "RUSTOK_MODULE_BUILD_COSIGN_PROGRAM must be configured".to_string())?,
        );
        let cosign_digest =
            std::env::var("RUSTOK_MODULE_BUILD_COSIGN_PROGRAM_DIGEST").map_err(|_| {
                "RUSTOK_MODULE_BUILD_COSIGN_PROGRAM_DIGEST must be configured".to_string()
            })?;
        let cosign_key_reference = std::env::var("RUSTOK_MODULE_BUILD_COSIGN_KEY_REFERENCE")
            .map_err(|_| {
                "RUSTOK_MODULE_BUILD_COSIGN_KEY_REFERENCE must be configured".to_string()
            })?;
        let signer = CosignArtifactSigner::new(cosign_path, cosign_digest, cosign_key_reference)?;
        let isolation_attestation_path = std::env::var("RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION")
            .map_err(|_| {
                "RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION must be configured".to_string()
            })?;
        let isolation_attestation = load_isolation_attestation(
            &isolation_attestation_path,
            job_runtime,
            &job_image_digest,
        )?;
        Self::new_with_attestation(
            job_launcher_path,
            job_runtime,
            job_image_digest,
            workdir,
            source_root,
            cargo_path,
            cargo_home,
            dependency_materializer,
            wasm_tools_path,
            publication_target,
            registry_credentials,
            signer,
            request_timeout,
            Some(isolation_attestation),
        )
    }

    pub fn new(
        job_launcher_path: PathBuf,
        job_runtime: OciJobRuntime,
        job_image_digest: String,
        workdir: PathBuf,
        source_root: PathBuf,
        cargo_path: PathBuf,
        cargo_home: PathBuf,
        dependency_materializer: Option<OciScopedDependencyMaterializer>,
        wasm_tools_path: PathBuf,
        publication_target: OciArtifactPublicationTarget,
        registry_credentials: Arc<dyn RegistryCredentialBroker>,
        signer: CosignArtifactSigner,
        request_timeout: Duration,
    ) -> Result<Self, String> {
        Self::new_with_attestation(
            job_launcher_path,
            job_runtime,
            job_image_digest,
            workdir,
            source_root,
            cargo_path,
            cargo_home,
            dependency_materializer,
            wasm_tools_path,
            publication_target,
            registry_credentials,
            signer,
            request_timeout,
            None,
        )
    }

    fn new_with_attestation(
        job_launcher_path: PathBuf,
        job_runtime: OciJobRuntime,
        job_image_digest: String,
        workdir: PathBuf,
        source_root: PathBuf,
        cargo_path: PathBuf,
        cargo_home: PathBuf,
        dependency_materializer: Option<OciScopedDependencyMaterializer>,
        wasm_tools_path: PathBuf,
        publication_target: OciArtifactPublicationTarget,
        registry_credentials: Arc<dyn RegistryCredentialBroker>,
        signer: CosignArtifactSigner,
        request_timeout: Duration,
        isolation_attestation: Option<OciJobIsolationAttestation>,
    ) -> Result<Self, String> {
        if !job_launcher_path.is_absolute()
            || !workdir.is_absolute()
            || !is_sha256_digest(&job_image_digest)
        {
            return Err(
                "module build job launcher and workdir must be absolute and the job image must be digest-pinned"
                    .to_string(),
            );
        }
        publication_target
            .validate()
            .map_err(|error| format!("module build publication target is invalid: {error}"))?;
        let metadata = std::fs::symlink_metadata(&job_launcher_path).map_err(|error| {
            format!(
                "module build job launcher {} cannot be inspected: {error}",
                job_launcher_path.display()
            )
        })?;
        let workdir_metadata = std::fs::metadata(&workdir).map_err(|error| {
            format!(
                "module build workdir {} cannot be inspected: {error}",
                workdir.display()
            )
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || !workdir_metadata.is_dir()
            || request_timeout.is_zero()
        {
            return Err("module build job launcher configuration is invalid".to_string());
        }
        Ok(Self {
            job_launcher_path,
            job_runtime,
            job_image_digest,
            cargo_metadata: CargoMetadataInspector::new(cargo_path, cargo_home)?,
            source_materializer: SourceMaterializer::new(source_root, workdir)?,
            dependency_materializer,
            wit_contract: WitContractInspector::new(wasm_tools_path)?,
            publication_target,
            registry_credentials,
            signer,
            request_timeout,
            isolation_attestation,
        })
    }
}

#[async_trait]
impl ModuleBuildWorker for OciJobBuildWorker {
    async fn execute_build(
        &self,
        request: ModuleBuildRequest,
    ) -> Result<ModuleBuildResult, ModuleBuildProtocolError> {
        request.validate()?;
        let request_json = serde_json::to_vec(&request)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        let request_digest = oci_job_request_digest(&request_json);
        let output_limit = usize::try_from(request.limits.output_bytes)
            .map_err(|_| ModuleBuildProtocolError::InvalidLimits)?;
        let execution_timeout = self
            .request_timeout
            .min(Duration::from_millis(request.limits.wall_clock_ms));
        let execution_deadline = Instant::now() + execution_timeout;
        let output_budget = Arc::new(OutputBudget::new(output_limit));
        let source = match self.source_materializer.materialize(&request).await {
            Ok(source) => source,
            Err(SourceMaterializationError::DigestMismatch) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::SourceDigestMismatch,
                ));
            }
            Err(SourceMaterializationError::UnsafeArchive) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::UnsafeArchive,
                ));
            }
            Err(SourceMaterializationError::ResourceLimit) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            }
            Err(SourceMaterializationError::Internal(error)) => {
                return Err(ModuleBuildProtocolError::Transport(error));
            }
        };
        match SourcePolicyPreflight::inspect(source.source_dir(), &request).await {
            Ok(()) => {}
            Err(SourcePolicyError::DependencyPolicyDenied) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::DependencyPolicyDenied,
                ));
            }
            Err(SourcePolicyError::BuildScriptDenied) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::BuildScriptDenied,
                ));
            }
            Err(SourcePolicyError::NativeLinkDenied) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::NativeLinkDenied,
                ));
            }
            Err(SourcePolicyError::Internal(error)) => {
                return Err(ModuleBuildProtocolError::Transport(error));
            }
        }
        let cargo_home = match &request.network_policy {
            rustok_modules::ModuleBuildNetworkPolicy::Denied => {
                self.cargo_metadata.default_cargo_home().to_path_buf()
            }
            rustok_modules::ModuleBuildNetworkPolicy::ScopedDependencyMaterialization {
                ..
            } => {
                let Some(materializer) = &self.dependency_materializer else {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::NetworkPolicyDenied,
                    ));
                };
                let Some(materialization_timeout) = remaining_timeout(execution_deadline) else {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ResourceLimitExceeded,
                    ));
                };
                match materializer
                    .materialize(
                        source.source_dir(),
                        source.job_dir(),
                        &request,
                        materialization_timeout,
                    )
                    .await
                {
                    Ok(cargo_home) => cargo_home,
                    Err(DependencyMaterializationError::EndpointDenied) => {
                        return Ok(failed_result(
                            &request,
                            ModuleBuildFailureCode::NetworkPolicyDenied,
                        ));
                    }
                    Err(DependencyMaterializationError::ResourceLimit) => {
                        return Ok(failed_result(
                            &request,
                            ModuleBuildFailureCode::ResourceLimitExceeded,
                        ));
                    }
                    Err(DependencyMaterializationError::Internal(error)) => {
                        return Err(ModuleBuildProtocolError::Transport(error));
                    }
                }
            }
        };
        let Some(metadata_timeout) = remaining_timeout(execution_deadline) else {
            return Ok(failed_result(
                &request,
                ModuleBuildFailureCode::ResourceLimitExceeded,
            ));
        };
        match self
            .cargo_metadata
            .inspect(source.source_dir(), &request, &cargo_home, metadata_timeout)
            .await
        {
            Ok(()) => {}
            Err(CargoMetadataError::DependencyPolicyDenied) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::DependencyPolicyDenied,
                ));
            }
            Err(CargoMetadataError::BuildScriptDenied) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::BuildScriptDenied,
                ));
            }
            Err(CargoMetadataError::NativeLinkDenied) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::NativeLinkDenied,
                ));
            }
            Err(CargoMetadataError::ResourceLimit) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            }
            Err(CargoMetadataError::NetworkPolicyDenied) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::NetworkPolicyDenied,
                ));
            }
            Err(CargoMetadataError::Internal(error)) => {
                return Err(ModuleBuildProtocolError::Transport(error));
            }
        }
        let output_dir = source.job_dir().join("output");
        let target_dir = source.job_dir().join("target");
        let home_dir = source.job_dir().join("home");
        tokio::fs::create_dir_all(&output_dir)
            .await
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        tokio::fs::create_dir_all(&target_dir)
            .await
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        tokio::fs::create_dir_all(&home_dir)
            .await
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        let mut child = Command::new(&self.job_launcher_path)
            .current_dir(source.job_dir())
            .env_clear()
            .env("RUSTOK_MODULE_BUILD_OCI_RUNTIME", self.job_runtime.as_str())
            .env(
                "RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST",
                &self.job_image_digest,
            )
            .env("RUSTOK_MODULE_BUILD_REQUEST_DIGEST", &request_digest)
            .env(
                "RUSTOK_MODULE_BUILD_PROTOCOL_VERSION",
                request.protocol_version.to_string(),
            )
            .env("RUSTOK_MODULE_BUILD_SOURCE_DIR", source.source_dir())
            .env("RUSTOK_MODULE_BUILD_OUTPUT_DIR", &output_dir)
            .env(
                "RUSTOK_MODULE_BUILD_CARGO",
                self.cargo_metadata.cargo_path(),
            )
            .env("CARGO_HOME", &cargo_home)
            .env("CARGO_NET_OFFLINE", "true")
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("CARGO_TERM_COLOR", "never")
            .env("HOME", &home_dir)
            .env("RUSTUP_TOOLCHAIN", &request.toolchain.rust_toolchain)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            ModuleBuildProtocolError::Transport("runner stdin is unavailable".to_string())
        })?;
        stdin
            .write_all(&request_json)
            .await
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        drop(stdin);
        let stdout = child.stdout.take().ok_or_else(|| {
            ModuleBuildProtocolError::Transport("runner stdout is unavailable".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            ModuleBuildProtocolError::Transport("runner stderr is unavailable".to_string())
        })?;
        let stdout_task = tokio::spawn(read_with_budget(stdout, Arc::clone(&output_budget)));
        let stderr_task = tokio::spawn(read_with_budget(stderr, output_budget));
        let Some(job_timeout) = remaining_timeout(execution_deadline) else {
            return Ok(failed_result(
                &request,
                ModuleBuildFailureCode::ResourceLimitExceeded,
            ));
        };
        let status = match timeout(job_timeout, child.wait()).await {
            Ok(status) => {
                status.map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?
            }
            Err(_) => {
                let _ = child.kill().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(ModuleBuildProtocolError::Transport(
                    "module build OCI job launcher timed out".to_string(),
                ));
            }
        };
        let stdout = collect_job_output(stdout_task).await?;
        let _stderr = collect_job_output(stderr_task).await?;
        if !status.success() {
            return Err(ModuleBuildProtocolError::Transport(format!(
                "module build OCI job launcher exited with {}",
                status
            )));
        }
        let mut result: ModuleBuildResult = serde_json::from_slice(&stdout)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        if result.publication.is_some() {
            return Err(ModuleBuildProtocolError::Transport(
                "module build OCI job launcher must not supply publication identity".to_string(),
            ));
        }
        verify_oci_job_receipt(
            &output_dir,
            &request,
            self.job_runtime,
            &self.job_image_digest,
            &request_digest,
        )
        .await?;
        result.validate_against(&request)?;
        if matches!(&result.outcome, ModuleBuildOutcome::Succeeded) {
            match ComponentArtifactInspector::inspect(&output_dir, &request, &result).await {
                Ok(()) => {}
                Err(ComponentArtifactError::InspectionFailed) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ComponentInspectionFailed,
                    ));
                }
                Err(ComponentArtifactError::ResourceLimit) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ResourceLimitExceeded,
                    ));
                }
                Err(ComponentArtifactError::Internal(error)) => {
                    return Err(ModuleBuildProtocolError::Transport(error));
                }
            }
            let Some(wit_timeout) = remaining_timeout(execution_deadline) else {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            };
            match self
                .wit_contract
                .inspect(&output_dir, &request, &result, wit_timeout)
                .await
            {
                Ok(()) => {}
                Err(WitContractError::Mismatch) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::WitContractMismatch,
                    ));
                }
                Err(WitContractError::ResourceLimit) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ResourceLimitExceeded,
                    ));
                }
                Err(WitContractError::Internal(error)) => {
                    return Err(ModuleBuildProtocolError::Transport(error));
                }
            }
            match BuildEvidenceInspector::inspect(&output_dir, &request, &result).await {
                Ok(()) => {}
                Err(BuildEvidenceError::SbomInvalid) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::SbomGenerationFailed,
                    ));
                }
                Err(BuildEvidenceError::ProvenanceInvalid) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ProvenanceGenerationFailed,
                    ));
                }
                Err(BuildEvidenceError::ResourceLimit) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ResourceLimitExceeded,
                    ));
                }
                Err(BuildEvidenceError::Internal(error)) => {
                    return Err(ModuleBuildProtocolError::Transport(error));
                }
            }
            let publication_bundle =
                match PublicationBundleCollector::collect(&output_dir, &request, &result).await {
                    Ok(bundle) => bundle,
                    Err(PublicationBundleError::Invalid) => {
                        return Ok(failed_result(
                            &request,
                            ModuleBuildFailureCode::PublicationFailed,
                        ));
                    }
                    Err(PublicationBundleError::ResourceLimit) => {
                        return Ok(failed_result(
                            &request,
                            ModuleBuildFailureCode::ResourceLimitExceeded,
                        ));
                    }
                    Err(PublicationBundleError::Internal(error)) => {
                        return Err(ModuleBuildProtocolError::Transport(error));
                    }
                };
            let Some(publication_timeout) = remaining_timeout(execution_deadline)
                .map(|timeout| timeout.min(MAX_PUBLICATION_WINDOW))
            else {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            };
            if publication_timeout.is_zero() {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            }
            let publication_deadline = Instant::now() + publication_timeout;
            let publication_limits = ArtifactAdmissionLimits {
                max_descriptor_bytes: ArtifactAdmissionLimits::default().max_descriptor_bytes,
                max_payload_bytes: request
                    .limits
                    .disk_bytes
                    .min(request.limits.memory_bytes / 4)
                    .min(64 * 1024 * 1024),
            };
            let credentials = match timeout(
                publication_timeout,
                self.registry_credentials.acquire(
                    &self.publication_target,
                    publication_timeout + CREDENTIAL_LEASE_SAFETY_MARGIN,
                ),
            )
            .await
            {
                Ok(Ok(credentials)) => credentials,
                Ok(Err(RegistryCredentialError::Rejected)) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::PublicationFailed,
                    ));
                }
                Ok(Err(RegistryCredentialError::TimedOut)) | Err(_) => {
                    return Err(ModuleBuildProtocolError::Transport(
                        "module registry credential broker timed out".to_string(),
                    ));
                }
                Ok(Err(RegistryCredentialError::Unavailable(error))) => {
                    return Err(ModuleBuildProtocolError::Transport(format!(
                        "module registry credential broker unavailable: {error}"
                    )));
                }
            };
            if credentials.ensure_valid().is_err() {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::PublicationFailed,
                ));
            }
            let publisher =
                OciDistributionArtifactPublisher::strict(credentials.registry_auth())
                    .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
            let Some(remaining_publication_timeout) = remaining_timeout(publication_deadline)
            else {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            };
            let receipt = match timeout(
                remaining_publication_timeout,
                publisher.publish(
                    self.publication_target.clone(),
                    publication_bundle,
                    publication_limits,
                ),
            )
            .await
            {
                Ok(Ok(receipt)) => receipt,
                Ok(Err(OciArtifactPublicationError::InvalidTarget(_)))
                | Ok(Err(OciArtifactPublicationError::InvalidBundle(_)))
                | Ok(Err(OciArtifactPublicationError::ManifestDigestMismatch { .. })) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::PublicationFailed,
                    ));
                }
                Ok(Err(OciArtifactPublicationError::Registry(error))) => {
                    return Err(ModuleBuildProtocolError::Transport(format!(
                        "module artifact publication failed: {error}"
                    )));
                }
                Err(_) => {
                    return Err(ModuleBuildProtocolError::Transport(
                        "module artifact publication timed out".to_string(),
                    ));
                }
            };
            let Some(signature_timeout) = remaining_timeout(publication_deadline) else {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            };
            match self
                .signer
                .sign(&receipt.artifact, &credentials, signature_timeout)
                .await
            {
                Ok(()) => {}
                Err(CosignSigningError::Rejected) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::PublicationFailed,
                    ));
                }
                Err(CosignSigningError::TimedOut) => {
                    return Err(ModuleBuildProtocolError::Transport(
                        "module artifact signature publication timed out".to_string(),
                    ));
                }
                Err(CosignSigningError::Unavailable(error)) => {
                    return Err(ModuleBuildProtocolError::Transport(format!(
                        "module artifact signature publication unavailable: {error}"
                    )));
                }
                Err(CosignSigningError::Credential(RegistryCredentialError::Rejected)) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::PublicationFailed,
                    ));
                }
                Err(CosignSigningError::Credential(RegistryCredentialError::TimedOut)) => {
                    return Err(ModuleBuildProtocolError::Transport(
                        "module artifact signature credential lease expired".to_string(),
                    ));
                }
                Err(CosignSigningError::Credential(RegistryCredentialError::Unavailable(
                    error,
                ))) => {
                    return Err(ModuleBuildProtocolError::Transport(format!(
                        "module artifact signature credential unavailable: {error}"
                    )));
                }
            }
            let Some(signature_resolution_timeout) = remaining_timeout(publication_deadline) else {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            };
            let signature_manifest = match timeout(
                signature_resolution_timeout,
                publisher.resolve_cosign_signature(&self.publication_target, &receipt.artifact),
            )
            .await
            {
                Ok(Ok(signature_manifest)) => signature_manifest,
                Ok(Err(OciArtifactPublicationError::InvalidTarget(_)))
                | Ok(Err(OciArtifactPublicationError::InvalidBundle(_)))
                | Ok(Err(OciArtifactPublicationError::ManifestDigestMismatch { .. })) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::PublicationFailed,
                    ));
                }
                Ok(Err(OciArtifactPublicationError::Registry(error))) => {
                    return Err(ModuleBuildProtocolError::Transport(format!(
                        "module artifact signature manifest resolution failed: {error}"
                    )));
                }
                Err(_) => {
                    return Err(ModuleBuildProtocolError::Transport(
                        "module artifact signature manifest resolution timed out".to_string(),
                    ));
                }
            };
            result.publication = Some(ModuleBuildPublicationReceipt {
                artifact: receipt.artifact,
                sbom_referrer: receipt.sbom_referrer,
                provenance_referrer: receipt.provenance_referrer,
                signature_manifest,
                signature_authority: ModuleBuildSignatureAuthority::BuildService,
            });
            result.validate_against(&request)?;
        }
        Ok(result)
    }
}

impl ModuleBuildWorkerReadiness for OciJobBuildWorker {
    fn is_ready(&self) -> bool {
        std::fs::symlink_metadata(&self.job_launcher_path).is_ok_and(|metadata| {
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && matches!(
                    self.job_runtime,
                    OciJobRuntime::Gvisor | OciJobRuntime::Kata
                )
                && is_sha256_digest(&self.job_image_digest)
                && self.registry_credentials.is_ready()
                && self.signer.is_ready()
                && self
                    .isolation_attestation
                    .as_ref()
                    .is_some_and(|attestation| {
                        attestation.matches(self.job_runtime, &self.job_image_digest)
                    })
        })
    }
}

impl OciJobIsolationAttestation {
    fn matches(&self, runtime: OciJobRuntime, image_digest: &str) -> bool {
        self.runtime == runtime.as_str()
            && self.image_digest == image_digest
            && !self.privileged
            && !self.host_mounts
            && !self.container_socket
            && !self.host_pid
            && !self.host_network
            && self.network_mode == "none"
            && self.resource_limits
            && self.ephemeral_job
    }
}

fn load_isolation_attestation(
    path: &str,
    runtime: OciJobRuntime,
    image_digest: &str,
) -> Result<OciJobIsolationAttestation, String> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err("RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION must be absolute".to_string());
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!("module build isolation attestation cannot be inspected: {error}")
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("module build isolation attestation must be a regular file".to_string());
    }
    if metadata.len() > MAX_ISOLATION_ATTESTATION_BYTES {
        return Err("module build isolation attestation exceeds its size limit".to_string());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("module build isolation attestation cannot be read: {error}"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("module build isolation attestation is invalid JSON: {error}"))?;
    let protocol_version = value
        .get("protocol_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            "module build isolation attestation protocol_version is missing".to_string()
        })?;
    let attestation = OciJobIsolationAttestation {
        runtime: required_string(&value, "runtime")?,
        image_digest: required_string(&value, "image_digest")?,
        privileged: required_bool(&value, "privileged")?,
        host_mounts: required_bool(&value, "host_mounts")?,
        container_socket: required_bool(&value, "container_socket")?,
        host_pid: required_bool(&value, "host_pid")?,
        host_network: required_bool(&value, "host_network")?,
        network_mode: required_string(&value, "network_mode")?,
        resource_limits: required_bool(&value, "resource_limits")?,
        ephemeral_job: required_bool(&value, "ephemeral_job")?,
    };
    if protocol_version != 1
        || !is_sha256_digest(&attestation.image_digest)
        || !attestation.matches(runtime, image_digest)
    {
        return Err(
            "module build isolation attestation does not match the configured hardened job"
                .to_string(),
        );
    }
    Ok(attestation)
}

fn required_string(value: &serde_json::Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("module build isolation attestation {key} is missing"))
}

fn required_bool(value: &serde_json::Value, key: &str) -> Result<bool, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("module build isolation attestation {key} is missing"))
}

async fn verify_oci_job_receipt(
    output_dir: &Path,
    request: &ModuleBuildRequest,
    runtime: OciJobRuntime,
    image_digest: &str,
    request_digest: &str,
) -> Result<(), ModuleBuildProtocolError> {
    const MAX_OCI_JOB_RECEIPT_BYTES: u64 = 8 * 1024;
    const OCI_JOB_RECEIPT_PROTOCOL_VERSION: u64 = 2;

    let path = output_dir.join("oci-job-receipt.json");
    let metadata = tokio::fs::symlink_metadata(&path)
        .await
        .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_OCI_JOB_RECEIPT_BYTES
    {
        return Err(ModuleBuildProtocolError::Transport(
            "OCI job receipt is invalid".to_string(),
        ));
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
    let receipt: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
    let request_id = request.request_id.to_string();
    let source_digest = request.source.digest.as_str();
    let dependency_lock_digest = request.dependency_policy.lock_digest.as_str();
    let toolchain_digest = request.toolchain.protocol_digest();
    let wit_digest = request.wit.protocol_digest();
    let matches_request = receipt
        .get("protocol_version")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|version| version == OCI_JOB_RECEIPT_PROTOCOL_VERSION)
        && receipt
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == request_id.as_str())
        && receipt
            .get("source_digest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == source_digest)
        && receipt
            .get("attempt")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|value| value == u64::from(request.attempt))
        && receipt
            .get("dependency_lock_digest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == dependency_lock_digest)
        && receipt
            .get("toolchain_digest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == toolchain_digest)
        && receipt
            .get("wit_digest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == wit_digest)
        && receipt
            .get("request_digest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == request_digest)
        && receipt
            .get("runtime")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == runtime.as_str())
        && receipt
            .get("image_digest")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value == image_digest)
        && receipt
            .get("job_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(is_valid_oci_job_id);
    if !matches_request {
        return Err(ModuleBuildProtocolError::Transport(
            "OCI job receipt does not match the immutable build request".to_string(),
        ));
    }
    Ok(())
}

/// Binds OCI-job evidence to the exact canonical request bytes sent to its
/// fixed launcher, including every protocol field and future additive field.
fn oci_job_request_digest(request_json: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok.module.build.oci-job-request.v1");
    hasher.update([0]);
    hasher.update((request_json.len() as u64).to_be_bytes());
    hasher.update(request_json);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == "sha256:".len() + 64
        && value.starts_with("sha256:")
        && value["sha256:".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_valid_oci_job_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/' | b':' | b'=')
        })
}

fn remaining_timeout(deadline: Instant) -> Option<Duration> {
    deadline.checked_duration_since(Instant::now())
}

fn failed_result(
    request: &ModuleBuildRequest,
    failure: ModuleBuildFailureCode,
) -> ModuleBuildResult {
    ModuleBuildResult {
        protocol_version: request.protocol_version,
        request_id: request.request_id,
        tenant_id: request
            .context
            .tenant_id
            .expect("validated module build request"),
        attempt: request.attempt,
        outcome: ModuleBuildOutcome::Failed(failure),
        source_digest: request.source.digest.clone(),
        dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
        toolchain_digest: request.toolchain.protocol_digest(),
        wit_digest: request.wit.protocol_digest(),
        component_digest: None,
        sbom_digest: None,
        provenance_digest: None,
        component_interface: None,
        evidence: ModuleBuildEvidence {
            log_reference: format!("worker://module-build/{}/log", request.request_id),
            policy_report_reference: format!("worker://module-build/{}/policy", request.request_id),
            validation_results: Vec::new(),
            diagnostics: vec![ModuleBuildDiagnostic {
                stage: failure.diagnostic_stage(),
                code: failure,
            }],
        },
        publication: None,
        metrics: ModuleBuildMetrics {
            duration_ms: 0,
            peak_memory_bytes: 0,
            output_bytes: 0,
        },
        retryable: false,
        next_action: ModuleBuildNextAction::ReviseSource,
    }
}

struct OutputBudget {
    limit: usize,
    consumed: AtomicUsize,
}

impl OutputBudget {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            consumed: AtomicUsize::new(0),
        }
    }

    fn reserve(&self, bytes: usize) -> Result<(), ModuleBuildProtocolError> {
        let previous = self.consumed.fetch_add(bytes, Ordering::Relaxed);
        if previous.saturating_add(bytes) > self.limit {
            return Err(ModuleBuildProtocolError::Transport(
                "module build runner exceeded its aggregate output limit".to_string(),
            ));
        }
        Ok(())
    }
}

async fn read_with_budget<R>(
    mut reader: R,
    budget: Arc<OutputBudget>,
) -> Result<Vec<u8>, ModuleBuildProtocolError>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        if read == 0 {
            return Ok(output);
        }
        budget.reserve(read)?;
        output.extend_from_slice(&buffer[..read]);
    }
}

async fn collect_job_output(
    task: tokio::task::JoinHandle<Result<Vec<u8>, ModuleBuildProtocolError>>,
) -> Result<Vec<u8>, ModuleBuildProtocolError> {
    task.await.map_err(|error| {
        ModuleBuildProtocolError::Transport(format!("module build output reader failed: {error}"))
    })?
}

#[cfg(test)]
mod tests {
    use super::is_sha256_digest;

    #[test]
    fn sha256_digest_requires_canonical_lowercase_hex() {
        assert!(is_sha256_digest(&format!("sha256:{}", "a".repeat(64))));
        assert!(!is_sha256_digest(&format!("sha256:{}", "A".repeat(64))));
    }
}
