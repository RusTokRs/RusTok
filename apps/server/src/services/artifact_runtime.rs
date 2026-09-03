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
    ArtifactCapabilityBrokerResolverRouter, ArtifactEffectivePolicyResolver, ArtifactRuntime,
    ArtifactRuntimeLifecycleExecutor, InstalledModuleArtifact, ModuleControlPlane,
    ModuleEffectivePolicy, ResolvingArtifactCapabilityBroker, SeaOrmArtifactNodeReadiness,
    SharedArtifactBindingExecutor, VerifiedArtifactNodeCache,
};
use rustok_sandbox::{CapabilityName, ExecutorRegistry, SandboxRuntime};
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
    ) -> Result<ModuleEffectivePolicy, String> {
        let policy =
            crate::services::effective_module_policy::EffectiveModulePolicyService::resolve(
                &self.db,
                &self.registry,
                tenant_id,
            )
            .await
            .map_err(|error| error.to_string())?;
        self.node_readiness
            .require_active(artifact, policy.policy_revision())
            .await
            .map_err(|error| error.to_string())?;
        Ok(policy)
    }
}

fn configured_artifact_node_id() -> Result<uuid::Uuid> {
    let value = std::env::var("RUSTOK_ARTIFACT_NODE_ID")
        .map_err(|_| Error::Message("RUSTOK_ARTIFACT_NODE_ID must be configured".to_string()))?;
    let node_id = uuid::Uuid::parse_str(&value)
        .map_err(|_| Error::Message("RUSTOK_ARTIFACT_NODE_ID must be a UUID".to_string()))?;
    if node_id.is_nil() {
        return Err(Error::Message(
            "RUSTOK_ARTIFACT_NODE_ID must not be nil".to_string(),
        ));
    }
    Ok(node_id)
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
    let data_capability = CapabilityName::new("platform.data")
        .map_err(|error| Error::Message(format!("invalid artifact data capability: {error}")))?;
    let object_data_capability = CapabilityName::new("platform.data.objects").map_err(|error| {
        Error::Message(format!("invalid artifact object-data capability: {error}"))
    })?;
    let secret_capability = CapabilityName::new("platform.secrets")
        .map_err(|error| Error::Message(format!("invalid artifact secret capability: {error}")))?;
    let mcp_capability = CapabilityName::new("platform.mcp")
        .map_err(|error| Error::Message(format!("invalid artifact MCP capability: {error}")))?;
    let http_capability = CapabilityName::new("platform.http")
        .map_err(|error| Error::Message(format!("invalid artifact HTTP capability: {error}")))?;
    let events_capability = CapabilityName::new("platform.events")
        .map_err(|error| Error::Message(format!("invalid artifact events capability: {error}")))?;
    let registry = ctx
        .shared_get::<ModuleRegistry>()
        .ok_or_else(|| Error::Message("module registry is not initialized".to_string()))?;
    let mcp_audit = ctx
        .shared_get::<Arc<crate::services::mcp_runtime::DbBackedMcpRuntimeBridge>>()
        .ok_or_else(|| {
            Error::Message("artifact runtime requires initialized MCP audit owner".to_string())
        })?;
    let mcp_invoker = crate::services::artifact_mcp::ServerArtifactMcpInvoker::local_registry(
        registry.clone(),
        mcp_audit,
    );
    let control_plane = ModuleControlPlane::new(ctx.db_clone());
    let node_readiness = control_plane
        .artifact_node_readiness(configured_artifact_node_id()?)
        .map_err(|error| Error::Message(format!("artifact node identity is invalid: {error}")))?;
    let secret_policy = control_plane.artifact_secret_handle_policy();
    let resolver = ArtifactCapabilityBrokerResolverRouter::new()
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
                Arc::new(control_plane.artifact_http_capability()),
            )
        })
        .and_then(|router| {
            router.route(
                events_capability,
                Arc::new(control_plane.artifact_events_capability()),
            )
        })
        .map_err(|error| Error::Message(format!("artifact capability route failed: {error}")))?;
    let mut executors = ExecutorRegistry::new();
    let rhai = sandbox_rhai_executor(ctx).await?;
    executors
        .register_isolated_worker(rhai)
        .map_err(|error| Error::Message(format!("artifact Rhai executor failed: {error}")))?;
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
}
