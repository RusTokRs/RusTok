use std::{
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use oci_distribution::{secrets::RegistryAuth, Client};
use rustok_modules::{
    ArtifactAdmissionLimits, ModuleBuildEvidence, ModuleBuildFailureCode, ModuleBuildMetrics,
    ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildProtocolError,
    ModuleBuildPublicationReceipt, ModuleBuildRequest, ModuleBuildResult, ModuleBuildWorker,
    OciArtifactPublicationError, OciArtifactPublicationTarget, OciArtifactPublisher,
    OciDistributionArtifactPublisher,
};
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

/// Fixed deployment-owned job-runner adapter. The configured executable is
/// mounted into the hardened worker image and receives one immutable request
/// on standard input, returning exactly one JSON `ModuleBuildResult` on
/// standard output. It is never selected by request data.
pub struct CommandBuildWorker {
    runner_path: PathBuf,
    cargo_metadata: CargoMetadataInspector,
    source_materializer: SourceMaterializer,
    dependency_materializer: Option<OciScopedDependencyMaterializer>,
    wit_contract: WitContractInspector,
    publication_target: OciArtifactPublicationTarget,
    publisher: OciDistributionArtifactPublisher,
    request_timeout: Duration,
}

impl CommandBuildWorker {
    pub fn from_env(request_timeout: Duration) -> Result<Self, String> {
        let runner_path = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_RUNNER")
                .map_err(|_| "RUSTOK_MODULE_BUILD_RUNNER must be configured".to_string())?,
        );
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
        let publisher = OciDistributionArtifactPublisher::new(
            Client::default(),
            publication_registry_auth_from_env()?,
        );
        Self::new(
            runner_path,
            workdir,
            source_root,
            cargo_path,
            cargo_home,
            dependency_materializer,
            wasm_tools_path,
            publication_target,
            publisher,
            request_timeout,
        )
    }

    pub fn new(
        runner_path: PathBuf,
        workdir: PathBuf,
        source_root: PathBuf,
        cargo_path: PathBuf,
        cargo_home: PathBuf,
        dependency_materializer: Option<OciScopedDependencyMaterializer>,
        wasm_tools_path: PathBuf,
        publication_target: OciArtifactPublicationTarget,
        publisher: OciDistributionArtifactPublisher,
        request_timeout: Duration,
    ) -> Result<Self, String> {
        if !runner_path.is_absolute() || !workdir.is_absolute() {
            return Err("module build runner path and workdir must be absolute".to_string());
        }
        let metadata = std::fs::symlink_metadata(&runner_path).map_err(|error| {
            format!(
                "module build runner {} cannot be inspected: {error}",
                runner_path.display()
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
            return Err("module build runner configuration is invalid".to_string());
        }
        Ok(Self {
            runner_path,
            cargo_metadata: CargoMetadataInspector::new(cargo_path, cargo_home)?,
            source_materializer: SourceMaterializer::new(source_root, workdir)?,
            dependency_materializer,
            wit_contract: WitContractInspector::new(wasm_tools_path)?,
            publication_target,
            publisher,
            request_timeout,
        })
    }
}

fn publication_registry_auth_from_env() -> Result<RegistryAuth, String> {
    let username = std::env::var("RUSTOK_MODULE_BUILD_PUBLICATION_USERNAME").ok();
    let password = std::env::var("RUSTOK_MODULE_BUILD_PUBLICATION_PASSWORD").ok();
    match (username, password) {
        (None, None) => Ok(RegistryAuth::Anonymous),
        (Some(username), Some(password))
            if !username.trim().is_empty() && !password.trim().is_empty() =>
        {
            Ok(RegistryAuth::Basic(username, password))
        }
        _ => Err(
            "RUSTOK_MODULE_BUILD_PUBLICATION_USERNAME and RUSTOK_MODULE_BUILD_PUBLICATION_PASSWORD must be configured together"
                .to_string(),
        ),
    }
}

#[async_trait]
impl ModuleBuildWorker for CommandBuildWorker {
    async fn execute_build(
        &self,
        request: ModuleBuildRequest,
    ) -> Result<ModuleBuildResult, ModuleBuildProtocolError> {
        request.validate()?;
        let request_json = serde_json::to_vec(&request)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
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
        let mut child = Command::new(&self.runner_path)
            .current_dir(source.job_dir())
            .env_clear()
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
        let Some(runner_timeout) = remaining_timeout(execution_deadline) else {
            return Ok(failed_result(
                &request,
                ModuleBuildFailureCode::ResourceLimitExceeded,
            ));
        };
        let status = match timeout(runner_timeout, child.wait()).await {
            Ok(status) => {
                status.map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?
            }
            Err(_) => {
                let _ = child.kill().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(ModuleBuildProtocolError::Transport(
                    "module build runner timed out".to_string(),
                ));
            }
        };
        let stdout = collect_runner_output(stdout_task).await?;
        let _stderr = collect_runner_output(stderr_task).await?;
        if !status.success() {
            return Err(ModuleBuildProtocolError::Transport(format!(
                "module build runner exited with {}",
                status
            )));
        }
        let mut result: ModuleBuildResult = serde_json::from_slice(&stdout)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        if result.publication.is_some() {
            return Err(ModuleBuildProtocolError::Transport(
                "module build runner must not supply publication identity".to_string(),
            ));
        }
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
            let Some(publication_timeout) = remaining_timeout(execution_deadline) else {
                return Ok(failed_result(
                    &request,
                    ModuleBuildFailureCode::ResourceLimitExceeded,
                ));
            };
            let publication_limits = ArtifactAdmissionLimits {
                max_descriptor_bytes: ArtifactAdmissionLimits::default().max_descriptor_bytes,
                max_payload_bytes: request
                    .limits
                    .disk_bytes
                    .min(request.limits.memory_bytes / 4)
                    .min(64 * 1024 * 1024),
            };
            let receipt = match timeout(
                publication_timeout,
                self.publisher.publish(
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
            result.publication = Some(ModuleBuildPublicationReceipt {
                artifact: receipt.artifact,
                sbom_referrer: receipt.sbom_referrer,
                provenance_referrer: receipt.provenance_referrer,
            });
            result.validate_against(&request)?;
        }
        Ok(result)
    }
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

async fn collect_runner_output(
    task: tokio::task::JoinHandle<Result<Vec<u8>, ModuleBuildProtocolError>>,
) -> Result<Vec<u8>, ModuleBuildProtocolError> {
    task.await.map_err(|error| {
        ModuleBuildProtocolError::Transport(format!("module build output reader failed: {error}"))
    })?
}
