use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use rustok_sandbox::{
    ExecutionPhase, SandboxContext, SandboxOutcome, SandboxPolicy, SandboxRuntime,
};

use crate::{
    ArtifactBlobStore, ArtifactLifecycleDispatch, ArtifactLifecycleExecutor, ArtifactReleaseRef,
    InstalledModuleArtifact, ModuleArtifactPackage, ModuleInstallationError, ModuleRuntimeBinding,
};

/// Executes an installed immutable artifact without involving the server's
/// source tree or Cargo dependency graph. The registry is resolved on each
/// invocation by the digest-pinned installation reference, then identity is
/// verified again before the payload crosses the sandbox boundary.
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
    pub fn new(runtime: ArtifactRuntime<R>, installations: I, policies: P) -> Self {
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

fn verify_runtime_package(
    artifact: &InstalledModuleArtifact,
    package: &ModuleArtifactPackage,
) -> Result<(), ArtifactRuntimeError> {
    if package.reference != artifact.reference {
        return Err(ArtifactRuntimeError::RegistryIdentityMismatch {
            installed: artifact.reference.canonical(),
            received: package.reference.canonical(),
        });
    }
    package.verify()?;
    if package.descriptor != artifact.descriptor {
        return Err(ArtifactRuntimeError::DescriptorMismatch {
            installation_id: artifact.installation_id,
        });
    }
    if package.release_ref() != artifact.release {
        return Err(ArtifactRuntimeError::ReleaseMismatch {
            installation_id: artifact.installation_id,
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum ArtifactRuntimeError {
    #[error("artifact binding `{0}` is not admitted for this installation")]
    BindingNotAdmitted(String),
    #[error(transparent)]
    Installation(#[from] ModuleInstallationError),
    #[error(transparent)]
    Sandbox(#[from] rustok_sandbox::SandboxError),
    #[error(
        "registry returned `{received}` for installed artifact reference `{installed}` during execution"
    )]
    RegistryIdentityMismatch { installed: String, received: String },
    #[error("registry descriptor does not match installation `{installation_id}`")]
    DescriptorMismatch { installation_id: uuid::Uuid },
    #[error("registry release does not match installation `{installation_id}`")]
    ReleaseMismatch { installation_id: uuid::Uuid },
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
        ArtifactPayloadKind, ArtifactReleaseRef, InMemoryArtifactBlobStore,
        ModuleArtifactDescriptor, ModuleInstallationScope, OciArtifactReference,
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
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                payload_kind: ArtifactPayloadKind::Rhai,
                runtime_abi: "rustok:module/runtime@1".to_string(),
                artifact_digest: payload_digest,
                entrypoint: "main".to_string(),
                capabilities: Vec::new(),
                bindings: Vec::new(),
            },
            media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            payload,
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
            .put_verified(&installed.descriptor.artifact_digest, &package.payload)
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
