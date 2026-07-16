use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;

use rustok_sandbox::{
    ExecutionPhase, SandboxContext, SandboxOutcome, SandboxPolicy, SandboxRuntime,
};

use crate::{
    ArtifactBlobStore, ArtifactLifecycleDispatch, ArtifactLifecycleExecutor, ArtifactReleaseRef,
    InstalledModuleArtifact, ModuleInstallationError, ModuleRuntimeBinding,
};

/// Bounded node-local cache for already-admitted CAS blobs. It is not a source
/// of truth: every hit is rehashed and any corrupt entry is discarded before a
/// sandbox request can be constructed.
pub struct VerifiedArtifactNodeCache<B> {
    durable: Arc<B>,
    max_bytes: usize,
    state: Mutex<NodeCacheState>,
}

struct NodeCacheState {
    bytes: usize,
    entries: HashMap<String, Vec<u8>>,
    lru: VecDeque<String>,
}

impl<B> VerifiedArtifactNodeCache<B>
where
    B: ArtifactBlobStore,
{
    pub fn new(durable: B, max_bytes: usize) -> Result<Self, ModuleInstallationError> {
        if max_bytes == 0 {
            return Err(ModuleInstallationError::Blob(
                "artifact node-cache capacity must be positive".into(),
            ));
        }
        Ok(Self {
            durable: Arc::new(durable),
            max_bytes,
            state: Mutex::new(NodeCacheState {
                bytes: 0,
                entries: HashMap::new(),
                lru: VecDeque::new(),
            }),
        })
    }

    pub async fn get_verified(&self, digest: &str) -> Result<Vec<u8>, ModuleInstallationError> {
        let mut state = self.state.lock().await;
        if let Some(bytes) = state.entries.get(digest).cloned() {
            if crate::installation::sha256_digest(&bytes) == digest {
                state.lru.retain(|key| key != digest);
                state.lru.push_back(digest.to_string());
                return Ok(bytes);
            }
            state.bytes = state.bytes.saturating_sub(bytes.len());
            state.entries.remove(digest);
            state.lru.retain(|key| key != digest);
        }
        drop(state);
        let bytes = self.durable.get_verified(digest).await?;
        if bytes.len() > self.max_bytes {
            return Ok(bytes);
        }
        let mut state = self.state.lock().await;
        while state.bytes + bytes.len() > self.max_bytes {
            let Some(oldest) = state.lru.pop_front() else {
                break;
            };
            if let Some(evicted) = state.entries.remove(&oldest) {
                state.bytes = state.bytes.saturating_sub(evicted.len());
            }
        }
        if !state.entries.contains_key(digest) {
            state.bytes += bytes.len();
            state.entries.insert(digest.to_string(), bytes.clone());
            state.lru.push_back(digest.to_string());
        }
        Ok(bytes)
    }
}

/// Executes an installed immutable artifact without involving the server's
/// source tree, Cargo dependency graph, or an external registry. The payload
/// is read and verified by digest from platform CAS before it crosses the
/// sandbox boundary.
pub struct ArtifactRuntime<B> {
    blobs: B,
    sandbox: SandboxRuntime,
}

impl<B> ArtifactRuntime<B>
where
    B: ArtifactBlobStore,
{
    pub fn new(blobs: B, sandbox: SandboxRuntime) -> Self {
        Self { blobs, sandbox }
    }

    pub async fn execute(
        &self,
        artifact: &InstalledModuleArtifact,
        context: SandboxContext,
        input: Value,
        policy: SandboxPolicy,
    ) -> Result<SandboxOutcome, ArtifactRuntimeError> {
        let payload = self
            .blobs
            .get_verified(&artifact.descriptor.artifact_digest)
            .await?;
        let request = artifact.sandbox_request(payload, context, input, policy)?;
        Ok(self.sandbox.execute(request).await?)
    }

    pub async fn execute_binding(
        &self,
        artifact: &InstalledModuleArtifact,
        binding: &ModuleRuntimeBinding,
        context: SandboxContext,
        input: Value,
        policy: SandboxPolicy,
    ) -> Result<SandboxOutcome, ArtifactRuntimeError> {
        if !artifact
            .descriptor
            .bindings
            .iter()
            .any(|candidate| candidate == binding)
        {
            return Err(ArtifactRuntimeError::BindingNotAdmitted(binding.id.clone()));
        }
        let payload = self
            .blobs
            .get_verified(&artifact.descriptor.artifact_digest)
            .await?;
        let mut request = artifact.sandbox_request(payload, context, input, policy)?;
        request.payload.entrypoint = binding.entrypoint.clone();
        Ok(self.sandbox.execute(request).await?)
    }
}

/// Resolves the immutable installation selected for an artifact lifecycle
/// dispatch. Implementations must apply platform/tenant scope and RLS.
#[async_trait]
pub trait ArtifactInstallationResolver: Send + Sync {
    async fn resolve(
        &self,
        release: &ArtifactReleaseRef,
        tenant_id: uuid::Uuid,
    ) -> Result<InstalledModuleArtifact, String>;
}

/// Supplies the effective capability grants and limits for the selected
/// installation. Descriptor declarations alone never become sandbox grants.
#[async_trait]
pub trait ArtifactSandboxPolicyResolver: Send + Sync {
    async fn resolve(
        &self,
        artifact: &InstalledModuleArtifact,
        tenant_id: uuid::Uuid,
    ) -> Result<SandboxPolicy, String>;
}

/// Production adapter from dispatcher lifecycle bindings to the shared sandbox.
pub struct ArtifactRuntimeLifecycleExecutor<R, I, P> {
    runtime: ArtifactRuntime<R>,
    installations: I,
    policies: P,
}

impl<B, I, P> ArtifactRuntimeLifecycleExecutor<B, I, P>
where
    B: ArtifactBlobStore,
{
    pub fn new(runtime: ArtifactRuntime<B>, installations: I, policies: P) -> Self {
        Self {
            runtime,
            installations,
            policies,
        }
    }
}

#[async_trait]
impl<B, I, P> ArtifactLifecycleExecutor for ArtifactRuntimeLifecycleExecutor<B, I, P>
where
    B: ArtifactBlobStore,
    I: ArtifactInstallationResolver,
    P: ArtifactSandboxPolicyResolver,
{
    async fn dispatch_lifecycle(
        &self,
        dispatch: ArtifactLifecycleDispatch<'_>,
    ) -> Result<(), String> {
        let artifact = self
            .installations
            .resolve(dispatch.release, dispatch.tenant_id)
            .await?;
        let policy = self.policies.resolve(&artifact, dispatch.tenant_id).await?;
        let mut context = SandboxContext::new(ExecutionPhase::Lifecycle);
        context.tenant_id = Some(dispatch.tenant_id);
        self.runtime
            .execute_binding(
                &artifact,
                dispatch.binding,
                context,
                serde_json::json!({
                    "binding_id": dispatch.binding.id,
                    "phase": dispatch.phase,
                    "config": dispatch.config,
                }),
                policy,
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Error)]
pub enum ArtifactRuntimeError {
    #[error("artifact binding `{0}` is not admitted for this installation")]
    BindingNotAdmitted(String),
    #[error(transparent)]
    Installation(#[from] ModuleInstallationError),
    #[error(transparent)]
    Sandbox(#[from] rustok_sandbox::SandboxError),
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use chrono::Utc;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    use rustok_sandbox::{
        CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse, ExecutionMetrics,
        ExecutionPhase, ExecutorRegistry, SandboxContext, SandboxExecutor, SandboxExecutorKind,
        SandboxHost, SandboxOutcome, SandboxRequest, SandboxResult,
    };

    use super::*;
    use crate::{
        ArtifactModuleKind, ArtifactPayloadKind, ArtifactReleaseRef, InMemoryArtifactBlobStore,
        ModuleArtifactDescriptor, ModuleArtifactPackage, ModuleDependencyLockGraph,
        ModuleInstallationScope, OciArtifactReference,
    };

    struct DenyBroker;

    #[async_trait]
    impl CapabilityBroker for DenyBroker {
        async fn invoke(
            &self,
            _call: &CapabilityCall,
            _grant: &CapabilityGrant,
        ) -> SandboxResult<CapabilityResponse> {
            unreachable!("the fixture does not invoke a capability")
        }
    }

    #[derive(Clone)]
    struct RecordingExecutor(Arc<Mutex<Option<SandboxRequest>>>);

    #[async_trait]
    impl SandboxExecutor for RecordingExecutor {
        fn kind(&self) -> SandboxExecutorKind {
            SandboxExecutorKind::Rhai
        }

        async fn execute(
            &self,
            request: &SandboxRequest,
            _host: SandboxHost,
        ) -> SandboxResult<SandboxOutcome> {
            *self.0.lock().expect("request lock") = Some(request.clone());
            Ok(SandboxOutcome {
                execution_id: request.context.execution_id,
                output: json!({ "executed": true }),
                metrics: ExecutionMetrics::default(),
            })
        }
    }

    fn package() -> ModuleArtifactPackage {
        let payload = b"artifact runtime payload".to_vec();
        let payload_digest = format!("sha256:{}", hex::encode(Sha256::digest(&payload)));
        ModuleArtifactPackage {
            reference: OciArtifactReference {
                registry: "registry.example".to_string(),
                repository: "modules/sample_module".to_string(),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            descriptor: ModuleArtifactDescriptor {
                schema_version: 1,
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                payload_kind: ArtifactPayloadKind::Rhai,
                module_kind: ArtifactModuleKind::Optional,
                runtime_abi: "rustok:module/runtime@1".to_string(),
                platform_compatibility: "^0.1".to_string(),
                required_features: Vec::new(),
                artifact_digest: payload_digest,
                entrypoint: "main".to_string(),
                capabilities: Vec::new(),
                bindings: Vec::new(),
                dependencies: Vec::new(),
                permissions: Vec::new(),
                settings_schema: None,
                data_schema: None,
                ui_contributions: Vec::new(),
                persistence_contract: None,
            },
            media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            payload: crate::ArtifactPayloadSource::Bytes(payload),
        }
    }

    #[tokio::test]
    async fn installed_artifact_is_resolved_verified_and_executed_by_shared_sandbox() {
        let package = package();
        let installed = InstalledModuleArtifact {
            installation_id: Uuid::new_v4(),
            scope: ModuleInstallationScope::Platform,
            reference: package.reference.clone(),
            release: ArtifactReleaseRef {
                slug: package.descriptor.slug.clone(),
                version: package.descriptor.version.clone(),
                digest: package.descriptor.artifact_digest.clone(),
            },
            descriptor: package.descriptor.clone(),
            dependency_lock: ModuleDependencyLockGraph::create(0, Vec::new())
                .expect("empty dependency lock"),
            capability_grant_revision: 1,
            installed_at: Utc::now(),
        };
        let observed = Arc::new(Mutex::new(None));
        let mut executors = ExecutorRegistry::new();
        executors
            .register(RecordingExecutor(Arc::clone(&observed)))
            .expect("executor registration");
        let sandbox = SandboxRuntime::new(executors, Arc::new(DenyBroker));
        let blobs = InMemoryArtifactBlobStore::default();
        blobs
            .put_verified(
                &installed.descriptor.artifact_digest,
                match &package.payload {
                    crate::ArtifactPayloadSource::Bytes(payload) => payload,
                    crate::ArtifactPayloadSource::TemporaryFile(_) => {
                        panic!("fixture uses an in-memory payload")
                    }
                },
            )
            .await
            .expect("admit payload");
        let runtime = ArtifactRuntime::new(blobs, sandbox);
        let context = SandboxContext::new(ExecutionPhase::Event);

        let outcome = runtime
            .execute(
                &installed,
                context.clone(),
                json!({ "id": 1 }),
                SandboxPolicy::default(),
            )
            .await
            .expect("artifact execution");

        assert_eq!(outcome.execution_id, context.execution_id);
        assert_eq!(outcome.output, json!({ "executed": true }));
        let request = observed
            .lock()
            .expect("request lock")
            .clone()
            .expect("sandbox request");
        assert_eq!(request.context, context);
        assert!(matches!(
            request.subject,
            rustok_sandbox::SandboxSubject::ModuleArtifact { .. }
        ));
    }

    #[tokio::test]
    async fn execution_fails_closed_when_admitted_blob_is_unavailable() {
        let package = package();
        let installed = InstalledModuleArtifact {
            installation_id: Uuid::new_v4(),
            scope: ModuleInstallationScope::Platform,
            reference: package.reference.clone(),
            release: package.release_ref(),
            descriptor: ModuleArtifactDescriptor {
                entrypoint: "other".to_string(),
                ..package.descriptor.clone()
            },
            dependency_lock: ModuleDependencyLockGraph::create(0, Vec::new())
                .expect("empty dependency lock"),
            capability_grant_revision: 1,
            installed_at: Utc::now(),
        };
        let runtime = ArtifactRuntime::new(
            InMemoryArtifactBlobStore::default(),
            SandboxRuntime::new(ExecutorRegistry::new(), Arc::new(DenyBroker)),
        );

        assert!(matches!(
            runtime
                .execute(
                    &installed,
                    SandboxContext::new(ExecutionPhase::Event),
                    Value::Null,
                    SandboxPolicy::default(),
                )
                .await,
            Err(ArtifactRuntimeError::Installation(
                ModuleInstallationError::BlobNotFound(_)
            ))
        ));
    }
}
