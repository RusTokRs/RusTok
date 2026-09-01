use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use rustok_build_publication::{
    CommandRegistryCredentialBroker, CosignArtifactSigner, CosignSigningError,
    RegistryCredentialBroker, RegistryCredentialError, validate_fixed_program,
};
use rustok_modules::{
    ArtifactAdmissionLimits, ModuleBuildDiagnostic, ModuleBuildEvidence, ModuleBuildFailureCode,
    ModuleBuildMetrics, ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildProtocolError,
    ModuleBuildPublicationReceipt, ModuleBuildRequest, ModuleBuildResult, ModuleBuildScenario,
    ModuleBuildSignatureAuthority, ModuleBuildWorker, ModuleBuildWorkerReadiness,
    OciArtifactPublicationError, OciArtifactPublicationTarget, OciArtifactPublisher,
    OciDistributionArtifactPublisher,
};
use rustok_sandbox::LocalSandboxScenario;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};

use crate::{
    ArtifactDescriptorError, ArtifactDescriptorFinalizer, BuildEvidenceError,
    BuildEvidenceInspector, CargoMetadataError, CargoMetadataInspector, ComponentArtifactError,
    ComponentArtifactInspector, DependencyMaterializationError, OciScopedDependencyMaterializer,
    PublicationBundleCollector, PublicationBundleError, SourceMaterializationError,
    SourceMaterializer, SourcePolicyError, SourcePolicyPreflight, WitContractError,
    WitContractInspector,
};

const MAX_PUBLICATION_WINDOW: Duration = Duration::from_secs(14 * 60);
const CREDENTIAL_LEASE_SAFETY_MARGIN: Duration = Duration::from_secs(30);
const MAX_ISOLATION_ATTESTATION_BYTES: u64 = 16 * 1024;
const MAX_OCI_JOB_RECEIPT_BYTES: u64 = 8 * 1024;
const MAX_SCENARIO_BYTES: u64 = 512 * 1024;
/// External receipt contract emitted by the independently deployed OCI-job
/// launcher. A new version is required whenever a launcher claim becomes part
/// of the worker's acceptance decision.
const OCI_JOB_RECEIPT_PROTOCOL_VERSION: u32 = 4;

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
    job_launcher_digest: String,
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
    isolation_attestation_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OciJobIsolationAttestation {
    protocol_version: u32,
    runtime: String,
    image_digest: String,
    launcher_digest: String,
    privileged: bool,
    host_mounts: bool,
    container_socket: bool,
    host_pid: bool,
    host_network: bool,
    tenant_database_access: bool,
    general_platform_secret_access: bool,
    network_mode: String,
    resource_limits: bool,
    pid_limit: u32,
    file_limit: u32,
    ephemeral_job: bool,
}

/// Terminal evidence written by the deployment-owned OCI launcher into the
/// request-scoped output directory. This is intentionally a closed schema:
/// the worker must not accept launcher-controlled extensions that could be
/// mistaken for an attested isolation fact by a future caller.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OciJobReceipt {
    protocol_version: u32,
    request_id: String,
    source_digest: String,
    scenario_digest: String,
    attempt: u32,
    dependency_lock_digest: String,
    toolchain_digest: String,
    wit_digest: String,
    component_target: String,
    request_digest: String,
    runtime: String,
    image_digest: String,
    job_id: String,
}

impl OciJobReceipt {
    fn matches_request(
        &self,
        request: &ModuleBuildRequest,
        runtime: OciJobRuntime,
        image_digest: &str,
        request_digest: &str,
    ) -> bool {
        self.protocol_version == OCI_JOB_RECEIPT_PROTOCOL_VERSION
            && self.request_id == request.request_id.to_string()
            && self.source_digest == request.source.digest.as_str()
            && self.scenario_digest == request.scenario.digest
            && self.attempt == request.attempt
            && self.dependency_lock_digest == request.dependency_policy.lock_digest.as_str()
            && self.toolchain_digest == request.toolchain.protocol_digest()
            && self.wit_digest == request.wit.protocol_digest()
            && self.component_target == request.toolchain.component_target
            && self.request_digest == request_digest
            && self.runtime == runtime.as_str()
            && self.image_digest == image_digest
            && is_valid_oci_job_id(&self.job_id)
    }
}

impl OciJobBuildWorker {
    pub fn from_env(request_timeout: Duration) -> Result<Self, String> {
        let job_launcher_path = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_JOB_LAUNCHER")
                .map_err(|_| "RUSTOK_MODULE_BUILD_JOB_LAUNCHER must be configured".to_string())?,
        );
        let job_launcher_digest = std::env::var("RUSTOK_MODULE_BUILD_JOB_LAUNCHER_DIGEST")
            .map_err(|_| {
                "RUSTOK_MODULE_BUILD_JOB_LAUNCHER_DIGEST must be configured".to_string()
            })?;
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
        let isolation_attestation_path = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION").map_err(|_| {
                "RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION must be configured".to_string()
            })?,
        );
        load_isolation_attestation(
            &isolation_attestation_path,
            job_runtime,
            &job_image_digest,
            &job_launcher_digest,
        )?;
        Self::new(
            job_launcher_path,
            job_launcher_digest,
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
            isolation_attestation_path,
        )
    }

    fn new(
        job_launcher_path: PathBuf,
        job_launcher_digest: String,
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
        isolation_attestation_path: PathBuf,
    ) -> Result<Self, String> {
        if !workdir.is_absolute()
            || !isolation_attestation_path.is_absolute()
            || !is_sha256_digest(&job_image_digest)
        {
            return Err(
                "module build workdir must be absolute and the job image must be digest-pinned"
                    .to_string(),
            );
        }
        validate_fixed_program(
            &job_launcher_path,
            &job_launcher_digest,
            "module build OCI job launcher",
        )?;
        publication_target
            .validate()
            .map_err(|error| format!("module build publication target is invalid: {error}"))?;
        let workdir_metadata = std::fs::metadata(&workdir).map_err(|error| {
            format!(
                "module build workdir {} cannot be inspected: {error}",
                workdir.display()
            )
        })?;
        if !workdir_metadata.is_dir() || request_timeout.is_zero() {
            return Err("module build job launcher configuration is invalid".to_string());
        }
        Ok(Self {
            job_launcher_path,
            job_launcher_digest,
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
            isolation_attestation_path,
        })
    }
}

#[async_trait]
impl ModuleBuildWorker for OciJobBuildWorker {
    async fn execute_build(
        &self,
        request: ModuleBuildRequest,
    ) -> Result<ModuleBuildResult, ModuleBuildProtocolError> {
        if !self.is_ready() {
            return Err(ModuleBuildProtocolError::Transport(
                "module build worker isolation boundary is not ready".to_string(),
            ));
        }
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
        let artifact_descriptor =
            match ArtifactDescriptorFinalizer::load(source.source_dir(), &request).await {
                Ok(descriptor) => descriptor,
                Err(ArtifactDescriptorError::Invalid) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::SourceManifestInvalid,
                    ));
                }
                Err(ArtifactDescriptorError::ResourceLimit) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ResourceLimitExceeded,
                    ));
                }
                Err(ArtifactDescriptorError::Internal(error)) => {
                    return Err(ModuleBuildProtocolError::Transport(error));
                }
            };
        match validate_source_scenario(
            source.source_dir(),
            &request.scenario,
            artifact_descriptor.capabilities(),
        )
        .await
        {
            Ok(()) => {}
            Err(ScenarioContractError::Invalid) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ScenarioContractInvalid,
                ));
            }
            Err(ScenarioContractError::ResourceLimit) => {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            }
            Err(ScenarioContractError::Internal(error)) => {
                return Err(ModuleBuildProtocolError::Transport(error));
            }
        }
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
        validate_fixed_program(
            &self.job_launcher_path,
            &self.job_launcher_digest,
            "module build OCI job launcher",
        )
        .map_err(ModuleBuildProtocolError::Transport)?;
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
            .env(
                "RUSTOK_MODULE_BUILD_COMPONENT_TARGET",
                &request.toolchain.component_target,
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
            match artifact_descriptor.finalize(&output_dir, &result).await {
                Ok(()) => {}
                Err(ArtifactDescriptorError::Invalid) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ArtifactDescriptorInvalid,
                    ));
                }
                Err(ArtifactDescriptorError::ResourceLimit) => {
                    return Ok(failed_result(
                        &request,
                        ModuleBuildFailureCode::ResourceLimitExceeded,
                    ));
                }
                Err(ArtifactDescriptorError::Internal(error)) => {
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
        validate_fixed_program(
            &self.job_launcher_path,
            &self.job_launcher_digest,
            "module build OCI job launcher",
        )
        .is_ok()
            && matches!(
                self.job_runtime,
                OciJobRuntime::Gvisor | OciJobRuntime::Kata
            )
            && is_sha256_digest(&self.job_image_digest)
            && self.registry_credentials.is_ready()
            && self.signer.is_ready()
            && load_isolation_attestation(
                &self.isolation_attestation_path,
                self.job_runtime,
                &self.job_image_digest,
                &self.job_launcher_digest,
            )
            .is_ok()
    }
}

impl OciJobIsolationAttestation {
    fn matches(&self, runtime: OciJobRuntime, image_digest: &str, launcher_digest: &str) -> bool {
        self.protocol_version == 1
            && self.runtime == runtime.as_str()
            && self.image_digest == image_digest
            && self.launcher_digest == launcher_digest
            && !self.privileged
            && !self.host_mounts
            && !self.container_socket
            && !self.host_pid
            && !self.host_network
            && !self.tenant_database_access
            && !self.general_platform_secret_access
            && self.network_mode == "none"
            && self.resource_limits
            && self.pid_limit > 0
            && self.file_limit > 0
            && self.ephemeral_job
    }
}

fn load_isolation_attestation(
    path: &Path,
    runtime: OciJobRuntime,
    image_digest: &str,
    launcher_digest: &str,
) -> Result<OciJobIsolationAttestation, String> {
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
    let attestation: OciJobIsolationAttestation = serde_json::from_slice(&bytes)
        .map_err(|error| format!("module build isolation attestation is invalid JSON: {error}"))?;
    if !is_sha256_digest(&attestation.image_digest)
        || !is_sha256_digest(&attestation.launcher_digest)
        || !attestation.matches(runtime, image_digest, launcher_digest)
    {
        return Err(
            "module build isolation attestation does not match the configured hardened job"
                .to_string(),
        );
    }
    Ok(attestation)
}

async fn verify_oci_job_receipt(
    output_dir: &Path,
    request: &ModuleBuildRequest,
    runtime: OciJobRuntime,
    image_digest: &str,
    request_digest: &str,
) -> Result<(), ModuleBuildProtocolError> {
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
    let receipt: OciJobReceipt = serde_json::from_slice(&bytes)
        .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
    if !receipt.matches_request(request, runtime, image_digest, request_digest) {
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

enum ScenarioContractError {
    Invalid,
    ResourceLimit,
    Internal(String),
}

async fn validate_source_scenario(
    source_dir: &Path,
    scenario: &ModuleBuildScenario,
    declared_capabilities: &[rustok_sandbox::CapabilityName],
) -> Result<(), ScenarioContractError> {
    let path = source_dir.join(&scenario.source_path);
    let metadata =
        tokio::fs::symlink_metadata(&path)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => ScenarioContractError::Invalid,
                _ => ScenarioContractError::Internal(error.to_string()),
            })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(ScenarioContractError::Invalid);
    }
    if metadata.len() > MAX_SCENARIO_BYTES {
        return Err(ScenarioContractError::ResourceLimit);
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| ScenarioContractError::Internal(error.to_string()))?;
    let parsed_scenario =
        LocalSandboxScenario::parse(&bytes).map_err(|_| ScenarioContractError::Invalid)?;
    let digest = parsed_scenario
        .canonical_digest()
        .map_err(|_| ScenarioContractError::Invalid)?;
    if digest != scenario.digest
        || parsed_scenario.policy.grants.iter().any(|grant| {
            !declared_capabilities
                .iter()
                .any(|declared| declared == &grant.name)
        })
    {
        return Err(ScenarioContractError::Invalid);
    }
    Ok(())
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
            scenario_comparison: None,
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
    use super::{
        OCI_JOB_RECEIPT_PROTOCOL_VERSION, OciJobIsolationAttestation, OciJobReceipt, OciJobRuntime,
        ScenarioContractError, is_sha256_digest, validate_source_scenario,
    };
    use rustok_modules::{
        MODULE_BUILD_PROTOCOL_VERSION, ModuleBuildAuthoring, ModuleBuildDependencyPolicy,
        ModuleBuildLimits, ModuleBuildNetworkPolicy, ModuleBuildRequest, ModuleBuildScenario,
        ModuleBuildSource, ModuleBuildToolchain, ModuleBuildValidationProfile,
        ModuleBuildWitContract, ModuleCommandContext,
    };
    use rustok_sandbox::{CapabilityName, LocalSandboxScenario};
    use std::{env, fs};
    use uuid::Uuid;

    #[test]
    fn sha256_digest_requires_canonical_lowercase_hex() {
        assert!(is_sha256_digest(&format!("sha256:{}", "a".repeat(64))));
        assert!(!is_sha256_digest(&format!("sha256:{}", "A".repeat(64))));
    }

    #[tokio::test]
    async fn source_scenario_must_be_digest_bound_and_subset_of_manifest_capabilities() {
        let root = env::temp_dir().join(format!("rustok-module-build-scenario-{}", Uuid::new_v4()));
        let scenario_path = root.join("tests/sandbox-scenario.json");
        fs::create_dir_all(scenario_path.parent().expect("scenario parent"))
            .expect("create scenario directory");
        let scenario_bytes = String::from_utf8(
            include_bytes!(
                "../../rustok-module-template/assets/tests/sandbox-scenario.json.template"
            )
            .to_vec(),
        )
        .expect("template is UTF-8")
        .replace("{{slug}}", "scenario_test");
        fs::write(&scenario_path, scenario_bytes).expect("write scenario");

        let scenario =
            LocalSandboxScenario::parse(&fs::read(&scenario_path).expect("read scenario"))
                .expect("parse scenario");
        let mut binding = ModuleBuildScenario {
            source_path: "tests/sandbox-scenario.json".to_string(),
            digest: scenario.canonical_digest().expect("scenario digest"),
        };
        let event_capability = CapabilityName::new("platform.events").expect("event capability");
        assert!(
            validate_source_scenario(&root, &binding, std::slice::from_ref(&event_capability))
                .await
                .is_ok()
        );

        binding.digest = format!("sha256:{}", "a".repeat(64));
        assert!(matches!(
            validate_source_scenario(&root, &binding, std::slice::from_ref(&event_capability))
                .await,
            Err(ScenarioContractError::Invalid)
        ));

        binding.digest = scenario.canonical_digest().expect("scenario digest");
        assert!(matches!(
            validate_source_scenario(&root, &binding, &[]).await,
            Err(ScenarioContractError::Invalid)
        ));
        fs::remove_dir_all(root).expect("remove scenario directory");
    }

    #[test]
    fn isolation_attestation_is_strict_and_bound_to_the_current_contract() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let value = serde_json::json!({
            "protocol_version": 1,
            "runtime": "gvisor",
            "image_digest": digest,
            "launcher_digest": format!("sha256:{}", "b".repeat(64)),
            "privileged": false,
            "host_mounts": false,
            "container_socket": false,
            "host_pid": false,
            "host_network": false,
            "tenant_database_access": false,
            "general_platform_secret_access": false,
            "network_mode": "none",
            "resource_limits": true,
            "pid_limit": 64,
            "file_limit": 4096,
            "ephemeral_job": true
        });
        let attestation: OciJobIsolationAttestation =
            serde_json::from_value(value.clone()).expect("valid attestation");
        let launcher_digest = format!("sha256:{}", "b".repeat(64));
        assert!(attestation.matches(OciJobRuntime::Gvisor, &digest, &launcher_digest));
        assert!(!attestation.matches(OciJobRuntime::Kata, &digest, &launcher_digest));
        assert!(!attestation.matches(
            OciJobRuntime::Gvisor,
            &digest,
            &format!("sha256:{}", "c".repeat(64)),
        ));

        let mut with_zero_pid_limit = value.clone();
        with_zero_pid_limit["pid_limit"] = serde_json::Value::from(0);
        let attestation_with_zero_pid_limit: OciJobIsolationAttestation =
            serde_json::from_value(with_zero_pid_limit).expect("syntactically valid attestation");
        assert!(!attestation_with_zero_pid_limit.matches(
            OciJobRuntime::Gvisor,
            &digest,
            &launcher_digest,
        ));

        let mut with_zero_file_limit = value.clone();
        with_zero_file_limit["file_limit"] = serde_json::Value::from(0);
        let attestation_with_zero_file_limit: OciJobIsolationAttestation =
            serde_json::from_value(with_zero_file_limit).expect("syntactically valid attestation");
        assert!(!attestation_with_zero_file_limit.matches(
            OciJobRuntime::Gvisor,
            &digest,
            &launcher_digest,
        ));

        let mut with_tenant_database_access = value.clone();
        with_tenant_database_access["tenant_database_access"] = serde_json::Value::Bool(true);
        let attestation_with_tenant_database_access: OciJobIsolationAttestation =
            serde_json::from_value(with_tenant_database_access)
                .expect("attestation with tenant database access is syntactically valid");
        assert!(!attestation_with_tenant_database_access.matches(
            OciJobRuntime::Gvisor,
            &digest,
            &launcher_digest,
        ));

        let mut with_general_platform_secret_access = value.clone();
        with_general_platform_secret_access["general_platform_secret_access"] =
            serde_json::Value::Bool(true);
        let attestation_with_general_platform_secret_access: OciJobIsolationAttestation =
            serde_json::from_value(with_general_platform_secret_access)
                .expect("attestation with general platform secret access is syntactically valid");
        assert!(!attestation_with_general_platform_secret_access.matches(
            OciJobRuntime::Gvisor,
            &digest,
            &launcher_digest,
        ));

        let mut with_unknown_field = value;
        with_unknown_field["unreviewed_control"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<OciJobIsolationAttestation>(with_unknown_field).is_err());
    }

    #[test]
    fn oci_job_receipt_requires_the_complete_current_schema() {
        let receipt = serde_json::json!({
            "protocol_version": OCI_JOB_RECEIPT_PROTOCOL_VERSION,
            "request_id": "8ba1a4e8-229e-4a94-93d7-2a16c4b69f26",
            "source_digest": format!("sha256:{}", "a".repeat(64)),
            "scenario_digest": format!("sha256:{}", "b".repeat(64)),
            "attempt": 1,
            "dependency_lock_digest": format!("sha256:{}", "b".repeat(64)),
            "toolchain_digest": format!("sha256:{}", "c".repeat(64)),
            "wit_digest": format!("sha256:{}", "d".repeat(64)),
            "component_target": "wasm32-wasip2",
            "request_digest": format!("sha256:{}", "e".repeat(64)),
            "runtime": "gvisor",
            "image_digest": format!("sha256:{}", "f".repeat(64)),
            "job_id": "module-build/8ba1a4e8-229e-4a94-93d7-2a16c4b69f26"
        });
        assert!(serde_json::from_value::<OciJobReceipt>(receipt.clone()).is_ok());

        let mut without_component_target = receipt.clone();
        without_component_target
            .as_object_mut()
            .expect("receipt object")
            .remove("component_target");
        assert!(serde_json::from_value::<OciJobReceipt>(without_component_target).is_err());

        let mut without_scenario_digest = receipt.clone();
        without_scenario_digest
            .as_object_mut()
            .expect("receipt object")
            .remove("scenario_digest");
        assert!(serde_json::from_value::<OciJobReceipt>(without_scenario_digest).is_err());

        let mut with_unknown_field = receipt;
        with_unknown_field["unreviewed_launcher_control"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<OciJobReceipt>(with_unknown_field).is_err());
    }

    #[test]
    fn oci_job_receipt_rejects_a_different_component_target() {
        let request = ModuleBuildRequest {
            protocol_version: MODULE_BUILD_PROTOCOL_VERSION,
            request_id: Uuid::parse_str("8ba1a4e8-229e-4a94-93d7-2a16c4b69f26")
                .expect("request id"),
            context: ModuleCommandContext {
                actor_id: Uuid::parse_str("509c6552-2bc5-4de8-9348-efa13a06d2cf")
                    .expect("actor id"),
                tenant_id: Some(
                    Uuid::parse_str("409c6552-2bc5-4de8-9348-efa13a06d2cf").expect("tenant id"),
                ),
                trace_id: "trace-oci-receipt".to_string(),
                correlation_id: Uuid::parse_str("309c6552-2bc5-4de8-9348-efa13a06d2cf")
                    .expect("correlation id"),
                idempotency_key: Uuid::parse_str("209c6552-2bc5-4de8-9348-efa13a06d2cf")
                    .expect("idempotency key"),
            },
            project_id: "target-binding".to_string(),
            source: ModuleBuildSource {
                reference: format!("cas://sha256:{}", "a".repeat(64)),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            scenario: ModuleBuildScenario {
                source_path: "tests/sandbox-scenario.json".to_string(),
                digest: format!("sha256:{}", "b".repeat(64)),
            },
            expected_module_slug: "target_binding".to_string(),
            expected_version: "1.0.0".to_string(),
            parent_release: None,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            wit: ModuleBuildWitContract {
                world: "rustok:module/module-runtime".to_string(),
                version: "1.0.0".to_string(),
            },
            toolchain: ModuleBuildToolchain {
                rust_toolchain: "1.93.0".to_string(),
                component_target: "wasm32-wasip2".to_string(),
            },
            authoring: ModuleBuildAuthoring {
                sdk_version: "0.1.0".to_string(),
                template_version: "0.1.0".to_string(),
            },
            dependency_policy: ModuleBuildDependencyPolicy {
                lock_digest: format!("sha256:{}", "b".repeat(64)),
                allowed_registries: vec!["https://crates.io".to_string()],
                allow_git_dependencies: false,
                allow_build_scripts: false,
                allow_native_links: false,
            },
            limits: ModuleBuildLimits {
                cpu_cores: 1,
                memory_bytes: 1,
                disk_bytes: 1,
                process_limit: 1,
                output_bytes: 1,
                wall_clock_ms: 1,
            },
            network_policy: ModuleBuildNetworkPolicy::Denied,
            validation_profiles: vec![ModuleBuildValidationProfile::Test],
            attempt: 1,
        };
        let image_digest = format!("sha256:{}", "c".repeat(64));
        let request_digest = format!("sha256:{}", "d".repeat(64));
        let receipt = OciJobReceipt {
            protocol_version: OCI_JOB_RECEIPT_PROTOCOL_VERSION,
            request_id: request.request_id.to_string(),
            source_digest: request.source.digest.clone(),
            scenario_digest: request.scenario.digest.clone(),
            attempt: request.attempt,
            dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
            toolchain_digest: request.toolchain.protocol_digest(),
            wit_digest: request.wit.protocol_digest(),
            component_target: request.toolchain.component_target.clone(),
            request_digest: request_digest.clone(),
            runtime: "gvisor".to_string(),
            image_digest: image_digest.clone(),
            job_id: "module-build/8ba1a4e8-229e-4a94-93d7-2a16c4b69f26".to_string(),
        };
        assert!(receipt.matches_request(
            &request,
            OciJobRuntime::Gvisor,
            &image_digest,
            &request_digest,
        ));

        let receipt_with_substituted_scenario = OciJobReceipt {
            scenario_digest: format!("sha256:{}", "e".repeat(64)),
            ..receipt.clone()
        };
        assert!(!receipt_with_substituted_scenario.matches_request(
            &request,
            OciJobRuntime::Gvisor,
            &image_digest,
            &request_digest,
        ));

        let receipt_with_native_target = OciJobReceipt {
            component_target: "x86_64-pc-windows-msvc".to_string(),
            ..receipt
        };
        assert!(!receipt_with_native_target.matches_request(
            &request,
            OciJobRuntime::Gvisor,
            &image_digest,
            &request_digest,
        ));
    }
}
