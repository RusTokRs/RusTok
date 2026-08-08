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
    ArtifactRuntimeLifecycleExecutor, ModuleControlPlane, ModuleEffectivePolicy,
    ResolvingArtifactCapabilityBroker, SharedArtifactBindingExecutor,
};
use rustok_sandbox::{CapabilityName, ExecutorRegistry, SandboxRuntime};
use rustok_sandbox_transport::GrpcRhaiExecutor;
use rustok_storage::StorageRuntime;
use rustok_worker_transport::MutualTlsClientConfig;
use sea_orm::DatabaseConnection;

use crate::error::{Error, Result};

use super::server_runtime_context::ServerRuntimeContext;

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
}

#[async_trait]
impl ArtifactEffectivePolicyResolver for ServerArtifactEffectivePolicyResolver {
    async fn resolve(
        &self,
        tenant_id: uuid::Uuid,
        _module_slug: &str,
    ) -> Result<ModuleEffectivePolicy, String> {
        crate::services::effective_module_policy::EffectiveModulePolicyService::resolve(
            &self.db,
            &self.registry,
            tenant_id,
        )
        .await
        .map_err(|error| error.to_string())
    }
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
    let runtime = ArtifactRuntime::new(control_plane.artifact_blob_store(storage), sandbox);
    Ok(Arc::new(ArtifactRuntimeLifecycleExecutor::new(
        runtime,
        control_plane.installation(),
        control_plane.artifact_sandbox_policy(),
        ServerArtifactEffectivePolicyResolver {
            db: ctx.db_clone(),
            registry,
        },
    )))
}
