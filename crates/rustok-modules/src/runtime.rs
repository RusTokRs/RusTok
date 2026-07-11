use serde_json::Value;
use thiserror::Error;

use rustok_sandbox::{SandboxContext, SandboxOutcome, SandboxPolicy, SandboxRuntime};

use crate::{
    ArtifactRegistry, InstalledModuleArtifact, ModuleArtifactPackage, ModuleInstallationError,
};

/// Executes an installed immutable artifact without involving the server's
/// source tree or Cargo dependency graph. The registry is resolved on each
/// invocation by the digest-pinned installation reference, then identity is
/// verified again before the payload crosses the sandbox boundary.
pub struct ArtifactRuntime<R> {
    registry: R,
    sandbox: SandboxRuntime,
}

impl<R> ArtifactRuntime<R>
where
    R: ArtifactRegistry,
{
    pub fn new(registry: R, sandbox: SandboxRuntime) -> Self {
        Self { registry, sandbox }
    }

    pub async fn execute(
        &self,
        artifact: &InstalledModuleArtifact,
        context: SandboxContext,
        input: Value,
        policy: SandboxPolicy,
    ) -> Result<SandboxOutcome, ArtifactRuntimeError> {
        let package = self.registry.fetch(&artifact.reference).await?;
        verify_runtime_package(artifact, &package)?;
        let request = artifact.sandbox_request(package.payload, context, input, policy)?;
        Ok(self.sandbox.execute(request).await?)
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
        ArtifactPayloadKind, ArtifactReleaseRef, ModuleArtifactDescriptor, ModuleInstallationScope,
        OciArtifactReference,
    };

    #[derive(Clone)]
    struct FixtureRegistry(ModuleArtifactPackage);

    #[async_trait]
    impl ArtifactRegistry for FixtureRegistry {
        async fn fetch(
            &self,
            _reference: &OciArtifactReference,
        ) -> Result<ModuleArtifactPackage, ModuleInstallationError> {
            Ok(self.0.clone())
        }
    }

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
        let runtime = ArtifactRuntime::new(FixtureRegistry(package), sandbox);
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
    async fn execution_rejects_a_registry_descriptor_different_from_the_installation() {
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
            FixtureRegistry(package),
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
            Err(ArtifactRuntimeError::DescriptorMismatch { .. })
        ));
    }
}
