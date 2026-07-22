//! Production composition for admitted module-artifact execution.
//!
//! This adapter owns only deployment wiring. Artifact identity, CAS reads,
//! installation selection, policy resolution, capability scope, and audit
//! persistence remain owned by `rustok-modules`; the server never supplies a
//! fallback executor or an unscoped capability broker.

use std::sync::Arc;

use rustok_modules::{
    ArtifactCapabilityBrokerResolverRouter, ArtifactRuntime, ArtifactRuntimeLifecycleExecutor,
    ModuleControlPlane, ResolvingArtifactCapabilityBroker, SharedArtifactBindingExecutor,
};
use rustok_sandbox::{CapabilityName, ExecutorRegistry, RhaiCapabilityBridge, SandboxRuntime};
use rustok_storage::StorageRuntime;

use crate::error::{Error, Result};

use super::server_runtime_context::ServerRuntimeContext;

/// Builds the one server-owned executor used for all admitted artifact
/// bindings. Rhai calls reach host capabilities only through the neutral
/// `capability_call` bridge; WASM calls use the equivalent WIT import.
pub fn compose_artifact_binding_executor(
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
    let control_plane = ModuleControlPlane::new(ctx.db_clone());
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
        .map_err(|error| Error::Message(format!("artifact capability route failed: {error}")))?;
    let mut executors = ExecutorRegistry::new();
    executors
        .register(
            rustok_sandbox::rhai::RhaiExecutor::new()
                .with_extension(Arc::new(RhaiCapabilityBridge)),
        )
        .map_err(|error| Error::Message(format!("artifact Rhai executor failed: {error}")))?;
    executors
        .register(rustok_sandbox::wasm::WasmComponentExecutor::new())
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
    )))
}
