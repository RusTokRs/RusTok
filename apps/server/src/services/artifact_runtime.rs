//! Production composition for admitted module-artifact execution.
//!
//! This adapter owns only deployment wiring. Artifact identity, CAS reads,
//! installation selection, policy resolution, capability scope, and audit
//! persistence remain owned by `rustok-modules`; the server never supplies a
//! fallback executor or an unscoped capability broker.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::ModuleRegistry;
use rustok_modules::{
    ArtifactBlobStore, ArtifactCapabilityBrokerResolverRouter, ArtifactEffectivePolicyResolveError,
    ArtifactEffectivePolicyResolver, ArtifactRuntime, ArtifactRuntimeLifecycleExecutor,
    ArtifactRuntimeReadinessProbe, ExecutorPoolIdentity, ExecutorPoolRegistration,
    ExecutorReadinessService, InstalledModuleArtifact, MODULE_BUILD_RUNTIME_ABI,
    ModuleControlPlane, ModuleEffectivePolicy, OwnerPlacementPolicy,
    ResolvingArtifactCapabilityBroker, RuntimeFingerprint, SeaOrmArtifactNodeReadiness,
    SharedArtifactBindingExecutor, VerifiedArtifactNodeCache,
};
use rustok_sandbox::{CapabilityName, ExecutorRegistry, SandboxExecutorPlacement, SandboxRuntime};
use rustok_sandbox_transport::GrpcRhaiExecutor;
use rustok_storage::StorageRuntime;
use rustok_worker_transport::MutualTlsClientConfig;
use sea_orm::DatabaseConnection;

use crate::error::{Error, Result};

use super::server_runtime_context::ServerRuntimeContext;

const DEFAULT_ARTIFACT_NODE_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_ARTIFACT_NODE_CACHE_MAX_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct SharedSandboxRhaiExecutor(pub GrpcRhaiExecutor);

pub async fn sandbox_rhai_executor(ctx: &ServerRuntimeContext) -> Result<GrpcRhaiExecutor> {
    if let Some(executor) = ctx.shared_get::<SharedSandboxRhaiExecutor>() {
        return Ok(executor.0.clone());
    }

    let endpoint = std::env::var("RUSTOK_SANDBOX_WORKER_ENDPOINT").map_err(|_| {
        Error::Message("RUSTOK_SANDBOX_WORKER_ENDPOINT must be configured".to_string())
    })?;
    let endpoint = tonic::transport::Endpoint::from_shared(endpoint)
        .map_err(|error| Error::Message(format!("sandbox worker endpoint is invalid: {error}")))?;
    let tls = MutualTlsClientConfig::from_env_prefix("RUSTOK_SANDBOX")
        .map_err(|error| Error::Message(format!("sandbox worker TLS is invalid: {error}")))?;
    let executor = GrpcRhaiExecutor::connect_with_tls(endpoint, tls.tls_config())
        .await
        .map_err(|error| Error::Message(format!("sandbox worker connection failed: {error}")))?;
    executor
        .check_readiness()
        .await
        .map_err(|error| Error::Message(format!("sandbox worker readiness failed: {error}")))?;
    ctx.shared_insert(SharedSandboxRhaiExecutor(executor.clone()));
    Ok(executor)
}

#[derive(Clone)]
struct ServerArtifactEffectivePolicyResolver {
    db: DatabaseConnection,
    registry: ModuleRegistry,
    node_readiness: SeaOrmArtifactNodeReadiness,
}

#[async_trait]
impl ArtifactEffectivePolicyResolver for ServerArtifactEffectivePolicyResolver {
    async fn resolve(
        &self,
        tenant_id: uuid::Uuid,
        artifact: &InstalledModuleArtifact,
    ) -> Result<ModuleEffectivePolicy, ArtifactEffectivePolicyResolveError> {
        let policy =
            crate::services::effective_module_policy::EffectiveModulePolicyService::resolve(
                &self.db,
                &self.registry,
                tenant_id,
            )
            .await
            .map_err(|error| ArtifactEffectivePolicyResolveError::Store(error.to_string()))?;
        self.node_readiness
            .require_active(artifact, policy.policy_revision())
            .await
            .map_err(|error| {
                ArtifactEffectivePolicyResolveError::NodeNotReady(error.to_string())
            })?;
        Ok(policy)
    }
}

pub const DEFAULT_DEV_ARTIFACT_NODE_ID: uuid::Uuid =
    uuid::Uuid::from_u128(0x019dbfb8_66a1_b38b_0559_97cc78906258);

fn configured_artifact_node_id() -> Result<uuid::Uuid> {
    match std::env::var("RUSTOK_ARTIFACT_NODE_ID") {
        Ok(value) => {
            let node_id = uuid::Uuid::parse_str(&value).map_err(|_| {
                Error::Message("RUSTOK_ARTIFACT_NODE_ID must be a UUID".to_string())
            })?;
            if node_id.is_nil() {
                return Err(Error::Message(
                    "RUSTOK_ARTIFACT_NODE_ID must not be nil".to_string(),
                ));
            }
            Ok(node_id)
        }
        Err(std::env::VarError::NotPresent) => {
            if crate::common::settings::is_production_environment() {
                Err(Error::Message(
                    "RUSTOK_ARTIFACT_NODE_ID must be configured in production".to_string(),
                ))
            } else {
                Ok(DEFAULT_DEV_ARTIFACT_NODE_ID)
            }
        }
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::Message(
            "RUSTOK_ARTIFACT_NODE_ID must be valid UTF-8".to_string(),
        )),
    }
}

fn configured_artifact_node_cache_max_bytes() -> Result<usize> {
    match std::env::var("RUSTOK_ARTIFACT_NODE_CACHE_MAX_BYTES") {
        Ok(value) => parse_artifact_node_cache_max_bytes(&value),
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_ARTIFACT_NODE_CACHE_MAX_BYTES),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::Message(
            "RUSTOK_ARTIFACT_NODE_CACHE_MAX_BYTES must be valid UTF-8".to_string(),
        )),
    }
}

fn parse_artifact_node_cache_max_bytes(value: &str) -> Result<usize> {
    let capacity = value.parse::<usize>().map_err(|_| {
        Error::Message("RUSTOK_ARTIFACT_NODE_CACHE_MAX_BYTES must be a byte count".to_string())
    })?;
    if !(1..=MAX_ARTIFACT_NODE_CACHE_MAX_BYTES).contains(&capacity) {
        return Err(Error::Message(format!(
            "RUSTOK_ARTIFACT_NODE_CACHE_MAX_BYTES must be between 1 and {MAX_ARTIFACT_NODE_CACHE_MAX_BYTES}"
        )));
    }
    Ok(capacity)
}

/// Builds the one server-owned executor used for all admitted artifact
/// bindings. Rhai calls reach host capabilities only through the neutral
/// `capability_call` bridge; WASM calls use the equivalent WIT import.
pub async fn compose_artifact_binding_executor(
    ctx: &ServerRuntimeContext,
) -> Result<SharedArtifactBindingExecutor> {
    let storage = ctx.shared_get::<StorageRuntime>().ok_or_else(|| {
        Error::Message("artifact runtime requires initialized durable storage".to_string())
    })?;
    let registry = ctx
        .shared_get::<ModuleRegistry>()
        .ok_or_else(|| Error::Message("module registry is not initialized".to_string()))?;
    let control_plane = ModuleControlPlane::new(ctx.db_clone());
    let node_readiness = control_plane
        .artifact_node_readiness(configured_artifact_node_id()?)
        .map_err(|error| Error::Message(format!("artifact node identity is invalid: {error}")))?;
    let resolver = build_capability_resolver(ctx, &control_plane, &storage, &registry)?;
    let mut executors = ExecutorRegistry::new();
    if std::env::var("RUSTOK_SANDBOX_WORKER_ENDPOINT").is_ok() {
        let rhai = sandbox_rhai_executor(ctx).await?;
        executors
            .register_isolated_worker(rhai)
            .map_err(|error| Error::Message(format!("artifact Rhai executor failed: {error}")))?;
    }
    executors
        .register_in_process(rustok_sandbox::wasm::WasmComponentExecutor::new())
        .map_err(|error| Error::Message(format!("artifact WASM executor failed: {error}")))?;

    let sandbox = SandboxRuntime::new(
        executors,
        Arc::new(ResolvingArtifactCapabilityBroker::new(resolver)),
    )
    .with_observer(Arc::new(control_plane.artifact_execution_audit()));
    let artifact_cache = VerifiedArtifactNodeCache::new(
        control_plane.artifact_blob_store(storage),
        configured_artifact_node_cache_max_bytes()?,
    )
    .map_err(|error| Error::Message(format!("artifact node cache is invalid: {error}")))?;
    let runtime = ArtifactRuntime::new(artifact_cache, sandbox);
    Ok(Arc::new(ArtifactRuntimeLifecycleExecutor::new(
        runtime,
        control_plane.installation(),
        control_plane.artifact_sandbox_policy(),
        ServerArtifactEffectivePolicyResolver {
            db: ctx.db_clone(),
            registry,
            node_readiness,
        },
    )))
}

/// Capability route names this host actually registers with its broker.
/// Readiness probes receive the same set, so a smoke receipt can never be
/// minted for routes the host does not serve.
const ARTIFACT_CAPABILITY_ROUTES: [&str; 6] = [
    "platform.data",
    "platform.data.objects",
    "platform.secrets",
    "platform.mcp",
    "platform.http",
    "platform.events",
];

fn artifact_capability_name(name: &'static str) -> Result<CapabilityName> {
    CapabilityName::new(name)
        .map_err(|error| Error::Message(format!("invalid artifact capability `{name}`: {error}")))
}

fn build_capability_resolver(
    ctx: &ServerRuntimeContext,
    control_plane: &ModuleControlPlane,
    storage: &StorageRuntime,
    registry: &ModuleRegistry,
) -> Result<ArtifactCapabilityBrokerResolverRouter> {
    let data_capability = artifact_capability_name(ARTIFACT_CAPABILITY_ROUTES[0])?;
    let object_data_capability = artifact_capability_name(ARTIFACT_CAPABILITY_ROUTES[1])?;
    let secret_capability = artifact_capability_name(ARTIFACT_CAPABILITY_ROUTES[2])?;
    let mcp_capability = artifact_capability_name(ARTIFACT_CAPABILITY_ROUTES[3])?;
    let http_capability = artifact_capability_name(ARTIFACT_CAPABILITY_ROUTES[4])?;
    let events_capability = artifact_capability_name(ARTIFACT_CAPABILITY_ROUTES[5])?;
    let mcp_audit = ctx
        .shared_get::<Arc<crate::services::mcp_runtime::DbBackedMcpRuntimeBridge>>()
        .ok_or_else(|| {
            Error::Message("artifact runtime requires initialized MCP audit owner".to_string())
        })?;
    let mcp_invoker = crate::services::artifact_mcp::ServerArtifactMcpInvoker::local_registry(
        registry.clone(),
        mcp_audit,
    );
    let secret_policy = control_plane.artifact_secret_handle_policy();
    ArtifactCapabilityBrokerResolverRouter::new()
        .route(
            data_capability,
            Arc::new(control_plane.artifact_data_capability()),
        )
        .and_then(|router| {
            router.route(
                object_data_capability,
                Arc::new(control_plane.artifact_data_object_capability(storage.clone())),
            )
        })
        .and_then(|router| {
            router.route(
                secret_capability,
                Arc::new(control_plane.artifact_secret_capability(secret_policy)),
            )
        })
        .and_then(|router| {
            router.route(
                mcp_capability,
                Arc::new(control_plane.artifact_mcp_capability(mcp_invoker)),
            )
        })
        .and_then(|router| {
            router.route(
                http_capability,
                Arc::new(control_plane.artifact_http_capability()?),
            )
        })
        .and_then(|router| {
            router.route(
                events_capability,
                Arc::new(control_plane.artifact_events_capability()),
            )
        })
        .map_err(|error| Error::Message(format!("artifact capability route failed: {error}")))
}

fn sha256_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// Streams the running server binary into a canonical digest. The WASM
/// engine is compiled into this binary, so one digest is both the
/// executor-binary and the engine-build fact for the in-process pool.
fn current_server_binary_digest() -> Result<String> {
    use sha2::{Digest, Sha256};

    const MAX_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;

    let exe = std::env::current_exe()
        .map_err(|error| Error::Message(format!("server binary path is unavailable: {error}")))?;
    let metadata = std::fs::metadata(&exe)
        .map_err(|error| Error::Message(format!("server binary cannot be inspected: {error}")))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_EXECUTABLE_BYTES {
        return Err(Error::Message(
            "server binary size is outside the supported fingerprint range".to_string(),
        ));
    }
    let mut file = std::fs::File::open(&exe)
        .map_err(|error| Error::Message(format!("server binary cannot be read: {error}")))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)
            .map_err(|error| Error::Message(format!("server binary cannot be read: {error}")))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn required_governed_env(name: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) | Err(std::env::VarError::NotPresent) => Err(Error::Message(format!(
            "{name} must be configured before governed transitions can start"
        ))),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(Error::Message(format!("{name} must be valid UTF-8")))
        }
    }
}

/// Owner placement policy for governed transitions. These are deployment
/// facts chosen by the operator, never by a transport caller: without an
/// explicit configuration the governed composition is unavailable.
fn governed_placement_policy() -> Result<OwnerPlacementPolicy> {
    let policy_revision = required_governed_env("RUSTOK_GOVERNED_PLACEMENT_POLICY_REVISION")?
        .trim()
        .parse::<u64>()
        .map_err(|_| {
            Error::Message(
                "RUSTOK_GOVERNED_PLACEMENT_POLICY_REVISION must be a positive integer".to_string(),
            )
        })?;
    if policy_revision == 0 {
        return Err(Error::Message(
            "RUSTOK_GOVERNED_PLACEMENT_POLICY_REVISION must be positive".to_string(),
        ));
    }
    let required_placement =
        match required_governed_env("RUSTOK_GOVERNED_REQUIRED_PLACEMENT")?.trim() {
            "in_process" => SandboxExecutorPlacement::InProcess,
            "isolated_worker" => SandboxExecutorPlacement::IsolatedWorker,
            _ => {
                return Err(Error::Message(
                    "RUSTOK_GOVERNED_REQUIRED_PLACEMENT must be `in_process` or `isolated_worker`"
                        .to_string(),
                ));
            }
        };
    let allow_in_process_fallback = match std::env::var("RUSTOK_GOVERNED_ALLOW_IN_PROCESS_FALLBACK")
    {
        Ok(value) => match value.trim() {
            "true" => true,
            "false" => false,
            _ => {
                return Err(Error::Message(
                    "RUSTOK_GOVERNED_ALLOW_IN_PROCESS_FALLBACK must be `true` or `false`"
                        .to_string(),
                ));
            }
        },
        Err(std::env::VarError::NotPresent) => false,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(Error::Message(
                "RUSTOK_GOVERNED_ALLOW_IN_PROCESS_FALLBACK must be valid UTF-8".to_string(),
            ));
        }
    };
    Ok(OwnerPlacementPolicy {
        policy_revision,
        required_placement,
        allow_in_process_fallback,
    })
}

/// Host composition for governed transitions: one readiness service per
/// executor pool this node actually runs, the durable identities the pool
/// registry assigned to them, and the operator placement policy.
pub struct GovernedReadinessComposition {
    pub readiness_services: Vec<ExecutorReadinessService>,
    pub required_pools: Vec<ExecutorPoolIdentity>,
    pub placement_policy: OwnerPlacementPolicy,
}

async fn register_pool(
    control_plane: &ModuleControlPlane,
    registration: ExecutorPoolRegistration,
) -> Result<ExecutorPoolIdentity> {
    control_plane
        .executor_pool_registry()
        .register_or_resume(&registration)
        .await
        .map_err(|error| {
            Error::Message(format!(
                "executor pool `{}` registration failed: {error}",
                registration.pool_id
            ))
        })
}

fn build_pool_readiness_service(
    ctx: &ServerRuntimeContext,
    control_plane: &ModuleControlPlane,
    storage: &StorageRuntime,
    registry: &ModuleRegistry,
    cache_capacity: usize,
    executors: ExecutorRegistry,
    identity: ExecutorPoolIdentity,
) -> Result<ExecutorReadinessService> {
    let resolver = build_capability_resolver(ctx, control_plane, storage, registry)?;
    let sandbox = SandboxRuntime::new(
        executors,
        Arc::new(ResolvingArtifactCapabilityBroker::new(resolver)),
    )
    .with_observer(Arc::new(control_plane.artifact_execution_audit()));
    let artifact_cache = VerifiedArtifactNodeCache::new(
        control_plane.artifact_blob_store(storage.clone()),
        cache_capacity,
    )
    .map_err(|error| Error::Message(format!("artifact node cache is invalid: {error}")))?;
    let runtime = ArtifactRuntime::new(artifact_cache, sandbox);
    let probe = ArtifactRuntimeReadinessProbe::new(
        runtime,
        identity,
        ARTIFACT_CAPABILITY_ROUTES
            .iter()
            .map(|route| (*route).to_string()),
    )
    .map_err(|error| Error::Message(format!("readiness probe is invalid: {error}")))?;
    let blobs: Arc<dyn ArtifactBlobStore> = Arc::new(
        VerifiedArtifactNodeCache::new(
            control_plane.artifact_blob_store(storage.clone()),
            cache_capacity,
        )
        .map_err(|error| Error::Message(format!("artifact node cache is invalid: {error}")))?,
    );
    Ok(control_plane.executor_readiness(blobs, Arc::new(probe)))
}

/// Builds governed readiness for this node's pools.
///
/// The in-process WASM pool fingerprint is derived from the running server
/// binary; the isolated Rhai pool fingerprint is built only from the
/// worker's own identity evidence plus the mTLS trust anchor this host
/// pinned for it. When the sandbox worker is configured but cannot prove
/// its identity, the whole composition fails closed: a governed transition
/// must never skip a pool the deployment can still execute on.
pub async fn compose_governed_readiness(
    ctx: &ServerRuntimeContext,
) -> Result<GovernedReadinessComposition> {
    let placement_policy = governed_placement_policy()?;
    let storage = ctx.shared_get::<StorageRuntime>().ok_or_else(|| {
        Error::Message("artifact runtime requires initialized durable storage".to_string())
    })?;
    let registry = ctx
        .shared_get::<ModuleRegistry>()
        .ok_or_else(|| Error::Message("module registry is not initialized".to_string()))?;
    let control_plane = ModuleControlPlane::new(ctx.db_clone());
    let node_id = configured_artifact_node_id()?;
    let cache_capacity = configured_artifact_node_cache_max_bytes()?;

    let mut readiness_services = Vec::new();
    let mut required_pools = Vec::new();

    // In-process WASM pool.
    {
        let binary_digest = current_server_binary_digest()?;
        let identity = register_pool(
            &control_plane,
            ExecutorPoolRegistration {
                pool_id: format!("{node_id}/wasm-in-process"),
                fingerprint: RuntimeFingerprint {
                    engine_config_revision: sha256_digest(
                        format!(
                            "wasm-component-executor:abi-{}\0server-{}",
                            rustok_sandbox::wasm::WASM_COMPONENT_ABI_VERSION,
                            env!("CARGO_PKG_VERSION")
                        )
                        .as_bytes(),
                    ),
                    executor_binary_digest: binary_digest.clone(),
                    engine_build_digest: binary_digest,
                    isolated_worker_image_digest: None,
                    target_cpu_contract: std::env::consts::ARCH.to_string(),
                    runtime_abi: MODULE_BUILD_RUNTIME_ABI.to_string(),
                },
                placement: SandboxExecutorPlacement::InProcess,
                placement_attestation: None,
            },
        )
        .await?;
        let mut executors = ExecutorRegistry::new();
        executors
            .register_in_process(rustok_sandbox::wasm::WasmComponentExecutor::new())
            .map_err(|error| Error::Message(format!("artifact WASM executor failed: {error}")))?;
        readiness_services.push(build_pool_readiness_service(
            ctx,
            &control_plane,
            &storage,
            &registry,
            cache_capacity,
            executors,
            identity.clone(),
        )?);
        required_pools.push(identity);
    }

    // Isolated Rhai pool, present exactly when the worker is configured.
    if let Ok(endpoint) = std::env::var("RUSTOK_SANDBOX_WORKER_ENDPOINT") {
        let rhai = sandbox_rhai_executor(ctx).await?;
        let worker = rhai.worker_identity().await.map_err(|error| {
            Error::Message(format!(
                "sandbox worker identity is unavailable, governed readiness refuses to compose: {error}"
            ))
        })?;
        let tls = MutualTlsClientConfig::from_env_prefix("RUSTOK_SANDBOX")
            .map_err(|error| Error::Message(format!("sandbox worker TLS is invalid: {error}")))?;
        let mut attestation_material = Vec::new();
        for part in [
            "rustok.governed_isolated_pool_attestation",
            endpoint.as_str(),
            tls.trust_anchor_digest().as_str(),
            worker.worker_image_digest.as_str(),
            worker.isolation_runtime.as_str(),
        ] {
            attestation_material.extend_from_slice(part.as_bytes());
            attestation_material.push(0);
        }
        let identity = register_pool(
            &control_plane,
            ExecutorPoolRegistration {
                pool_id: format!("{node_id}/rhai-isolated"),
                fingerprint: RuntimeFingerprint {
                    executor_binary_digest: worker.executor_binary_digest.clone(),
                    engine_build_digest: worker.engine_build_digest.clone(),
                    engine_config_revision: worker.engine_config_digest.clone(),
                    isolated_worker_image_digest: Some(worker.worker_image_digest.clone()),
                    target_cpu_contract: worker.target_cpu_contract.clone(),
                    runtime_abi: worker.runtime_abi.clone(),
                },
                placement: SandboxExecutorPlacement::IsolatedWorker,
                placement_attestation: Some(sha256_digest(&attestation_material)),
            },
        )
        .await?;
        let mut executors = ExecutorRegistry::new();
        executors
            .register_isolated_worker(rhai)
            .map_err(|error| Error::Message(format!("artifact Rhai executor failed: {error}")))?;
        readiness_services.push(build_pool_readiness_service(
            ctx,
            &control_plane,
            &storage,
            &registry,
            cache_capacity,
            executors,
            identity.clone(),
        )?);
        required_pools.push(identity);
    }

    Ok(GovernedReadinessComposition {
        readiness_services,
        required_pools,
        placement_policy,
    })
}

#[derive(Clone)]
struct SharedGovernedReadiness(Arc<GovernedReadinessComposition>);

/// Returns the process-wide governed readiness composition, composing it on
/// first use. Pool fingerprints are processes-lifetime facts, so the binary
/// hashing and worker identity exchange happen once, not per transition.
pub async fn governed_readiness(
    ctx: &ServerRuntimeContext,
) -> Result<Arc<GovernedReadinessComposition>> {
    if let Some(shared) = ctx.shared_get::<SharedGovernedReadiness>() {
        return Ok(shared.0);
    }
    let composition = Arc::new(compose_governed_readiness(ctx).await?);
    ctx.shared_insert(SharedGovernedReadiness(composition.clone()));
    Ok(composition)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_ARTIFACT_NODE_CACHE_MAX_BYTES, MAX_ARTIFACT_NODE_CACHE_MAX_BYTES,
        parse_artifact_node_cache_max_bytes,
    };

    #[test]
    fn artifact_node_cache_capacity_is_bounded() {
        assert_eq!(
            parse_artifact_node_cache_max_bytes(&DEFAULT_ARTIFACT_NODE_CACHE_MAX_BYTES.to_string())
                .expect("default capacity"),
            DEFAULT_ARTIFACT_NODE_CACHE_MAX_BYTES
        );
        assert!(parse_artifact_node_cache_max_bytes("0").is_err());
        assert!(parse_artifact_node_cache_max_bytes("not-a-number").is_err());
        assert!(
            parse_artifact_node_cache_max_bytes(
                &(MAX_ARTIFACT_NODE_CACHE_MAX_BYTES + 1).to_string()
            )
            .is_err()
        );
    }

    #[test]
    fn artifact_node_id_defaults_in_non_production() {
        let node_id = super::configured_artifact_node_id().expect("default node id in dev");
        assert!(!node_id.is_nil());
    }
}
