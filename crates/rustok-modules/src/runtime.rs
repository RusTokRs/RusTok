use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;

use rustok_sandbox::{
    ExecutionPhase, RhaiBindingOutput, SandboxContext, SandboxOutcome, SandboxPolicy,
    SandboxRuntime,
};

use crate::{
    artifact_schema::{ArtifactSchemaValidationError, ArtifactSchemaValidatorCache},
    ArtifactBindingDispatch, ArtifactBindingDispatchEnvelope, ArtifactBindingDispatchEnvelopeError,
    ArtifactBindingExecutor, ArtifactBlobStore, ArtifactReleaseRef, InstalledModuleArtifact,
    ModuleArtifactError, ModuleInstallationError, ModuleRuntimeBinding,
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
    schema_validators: ArtifactSchemaValidatorCache,
}

impl<B> ArtifactRuntime<B>
where
    B: ArtifactBlobStore,
{
    pub fn new(blobs: B, sandbox: SandboxRuntime) -> Self {
        Self {
            blobs,
            sandbox,
            schema_validators: ArtifactSchemaValidatorCache::default(),
        }
    }

    pub async fn execute_binding(
        &self,
        artifact: &InstalledModuleArtifact,
        binding: &ModuleRuntimeBinding,
        context: SandboxContext,
        input: ArtifactBindingDispatchEnvelope,
        policy: SandboxPolicy,
    ) -> Result<SandboxOutcome, ArtifactRuntimeError> {
        artifact
            .descriptor
            .validate()
            .map_err(ArtifactRuntimeError::Descriptor)?;
        if !artifact
            .descriptor
            .bindings
            .iter()
            .any(|candidate| candidate == binding)
        {
            return Err(ArtifactRuntimeError::BindingNotAdmitted(binding.id.clone()));
        }
        input.validate_for(binding, context.phase)?;
        self.validate_binding_value(artifact, binding, BindingSchemaStage::Input, &input.payload)?;
        let payload = self
            .blobs
            .get_verified(&artifact.descriptor.artifact_digest)
            .await?;
        let input = serde_json::to_value(input)
            .map_err(|error| ArtifactRuntimeError::DispatchEnvelopeEncoding(error.to_string()))?;
        let mut request = artifact.sandbox_request(payload, context, input, policy)?;
        request.payload.entrypoint = binding.entrypoint.clone();
        let outcome = self.sandbox.execute(request).await?;
        let outcome = self.decode_rhai_output(artifact, outcome)?;
        self.validate_binding_value(
            artifact,
            binding,
            BindingSchemaStage::Output,
            &outcome.output,
        )?;
        Ok(outcome)
    }

    fn validate_binding_value(
        &self,
        artifact: &InstalledModuleArtifact,
        binding: &ModuleRuntimeBinding,
        stage: BindingSchemaStage,
        value: &Value,
    ) -> Result<(), ArtifactRuntimeError> {
        let schema_digest = match stage {
            BindingSchemaStage::Input => &binding.input_schema_digest,
            BindingSchemaStage::Output => &binding.output_schema_digest,
        };
        let schema = artifact
            .descriptor
            .schema_document(schema_digest)
            .ok_or_else(|| ArtifactRuntimeError::BindingSchemaNotAdmitted {
                binding: binding.id.clone(),
                schema_digest: schema_digest.clone(),
            })?;
        self.schema_validators
            .validate(schema_digest, schema, value)
            .map_err(|error| match error {
                ArtifactSchemaValidationError::Compilation => {
                    ArtifactRuntimeError::SchemaCompilation {
                        schema_digest: schema_digest.clone(),
                    }
                }
                ArtifactSchemaValidationError::Violation => {
                    ArtifactRuntimeError::BindingSchemaViolation {
                        binding: binding.id.clone(),
                        stage: stage.as_str(),
                        schema_digest: schema_digest.clone(),
                    }
                }
                ArtifactSchemaValidationError::CachePoisoned => {
                    ArtifactRuntimeError::SchemaValidatorCachePoisoned
                }
            })
    }

    fn decode_rhai_output(
        &self,
        artifact: &InstalledModuleArtifact,
        mut outcome: SandboxOutcome,
    ) -> Result<SandboxOutcome, ArtifactRuntimeError> {
        if artifact.descriptor.payload_kind == crate::ArtifactPayloadKind::Rhai {
            outcome.output = RhaiBindingOutput::decode(outcome.output)
                .map_err(|error| ArtifactRuntimeError::RhaiBinding(error.to_string()))?
                .output;
        }
        Ok(outcome)
    }
}

#[derive(Clone, Copy)]
enum BindingSchemaStage {
    Input,
    Output,
}

impl BindingSchemaStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
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

    /// Resolves the immutable installation carried by a durable work item.
    /// Implementations may override this with a direct installation lookup.
    /// The default is deliberately fail-closed: it permits execution only when
    /// the current effective selection still names the requested installation.
    async fn resolve_exact(
        &self,
        installation_id: uuid::Uuid,
        release: &ArtifactReleaseRef,
        tenant_id: uuid::Uuid,
    ) -> Result<InstalledModuleArtifact, String> {
        let artifact = self.resolve(release, tenant_id).await?;
        if artifact.installation_id != installation_id {
            return Err(
                "requested artifact installation is no longer the active tenant selection".into(),
            );
        }
        Ok(artifact)
    }
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

/// Production adapter from admitted dispatcher bindings to the shared sandbox.
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

fn effective_binding_policy(
    binding: &ModuleRuntimeBinding,
    phase: ExecutionPhase,
    mut policy: SandboxPolicy,
) -> Result<SandboxPolicy, String> {
    if phase == ExecutionPhase::Http {
        let timeout_ms = binding
            .http
            .as_ref()
            .ok_or_else(|| "HTTP dispatch requires an admitted HTTP binding".to_string())?
            .timeout_ms;
        policy.limits.wall_clock_ms = policy.limits.wall_clock_ms.min(timeout_ms);
    }
    Ok(policy)
}

#[async_trait]
impl<B, I, P> ArtifactBindingExecutor for ArtifactRuntimeLifecycleExecutor<B, I, P>
where
    B: ArtifactBlobStore,
    I: ArtifactInstallationResolver,
    P: ArtifactSandboxPolicyResolver,
{
    async fn dispatch_binding(
        &self,
        dispatch: ArtifactBindingDispatch<'_>,
    ) -> Result<Value, String> {
        if !dispatch.context.is_valid() {
            return Err("artifact binding execution context is invalid".to_string());
        }
        let artifact = match dispatch.target {
            crate::ArtifactInstallationTarget::CurrentRelease => {
                self.installations
                    .resolve(dispatch.release, dispatch.tenant_id)
                    .await?
            }
            crate::ArtifactInstallationTarget::ExactInstallation { installation_id } => {
                self.installations
                    .resolve_exact(installation_id, dispatch.release, dispatch.tenant_id)
                    .await?
            }
        };
        let policy = effective_binding_policy(
            dispatch.binding,
            dispatch.phase,
            self.policies.resolve(&artifact, dispatch.tenant_id).await?,
        )?;
        let mut context = SandboxContext::new(dispatch.phase);
        context.tenant_id = Some(dispatch.tenant_id);
        context.actor_id = dispatch.context.actor_id.clone();
        context.trace_id = dispatch.context.trace_id.clone();
        let input =
            ArtifactBindingDispatchEnvelope::new(dispatch.binding, dispatch.phase, dispatch.input);
        self.runtime
            .execute_binding(&artifact, dispatch.binding, context, input, policy)
            .await
            .map(|outcome| outcome.output)
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Error)]
pub enum ArtifactRuntimeError {
    #[error("artifact binding `{0}` is not admitted for this installation")]
    BindingNotAdmitted(String),
    #[error("artifact Rhai binding is invalid: {0}")]
    RhaiBinding(String),
    #[error("artifact binding dispatch envelope could not be encoded: {0}")]
    DispatchEnvelopeEncoding(String),
    #[error(transparent)]
    DispatchEnvelope(#[from] ArtifactBindingDispatchEnvelopeError),
    #[error("artifact descriptor is invalid: {0}")]
    Descriptor(#[source] ModuleArtifactError),
    #[error(
        "artifact binding `{binding}` references a schema not admitted by its descriptor: `{schema_digest}`"
    )]
    BindingSchemaNotAdmitted {
        binding: String,
        schema_digest: String,
    },
    #[error("admitted artifact schema `{schema_digest}` cannot be compiled")]
    SchemaCompilation { schema_digest: String },
    #[error(
        "artifact binding `{binding}` {stage} does not satisfy admitted schema `{schema_digest}`"
    )]
    BindingSchemaViolation {
        binding: String,
        stage: &'static str,
        schema_digest: String,
    },
    #[error("artifact schema validator cache lock is poisoned")]
    SchemaValidatorCachePoisoned,
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
        ExecutionPhase, ExecutorRegistry, RhaiBindingInput, SandboxContext, SandboxError,
        SandboxExecutor, SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxRequest,
        SandboxResult,
    };

    use super::*;
    use crate::{
        canonical_schema_digest, ArtifactModuleKind, ArtifactPayloadKind,
        ArtifactPermissionDescriptor, ArtifactReleaseRef, ArtifactSchemaDocument,
        InMemoryArtifactBlobStore, ModuleArtifactDescriptor, ModuleArtifactPackage,
        ModuleDependencyLockGraph, ModuleInstallationScope, ModuleRuntimeBinding,
        ModuleRuntimeBindingKind, OciArtifactReference,
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
    struct RecordingExecutor {
        observed: Arc<Mutex<Option<SandboxRequest>>>,
        output: Value,
    }

    impl RecordingExecutor {
        fn new(observed: Arc<Mutex<Option<SandboxRequest>>>, output: Value) -> Self {
            Self { observed, output }
        }
    }

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
            *self.observed.lock().expect("request lock") = Some(request.clone());
            let output = serde_json::to_value(RhaiBindingOutput::new(self.output.clone()))
                .map_err(|error| SandboxError::Internal(error.to_string()))?;
            Ok(SandboxOutcome {
                execution_id: request.context.execution_id,
                output,
                metrics: ExecutionMetrics::default(),
            })
        }
    }

    fn package() -> ModuleArtifactPackage {
        let payload = b"artifact runtime payload".to_vec();
        let payload_digest = format!("sha256:{}", hex::encode(Sha256::digest(&payload)));
        let input_schema = schema_document(json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "integer" } },
            "additionalProperties": false
        }));
        let output_schema = schema_document(json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["executed"],
            "properties": { "executed": { "type": "boolean" } },
            "additionalProperties": false
        }));
        let binding = ModuleRuntimeBinding {
            id: "event_handler".to_string(),
            kind: ModuleRuntimeBindingKind::Event,
            entrypoint: "main".to_string(),
            input_schema_digest: input_schema.digest.clone(),
            output_schema_digest: output_schema.digest.clone(),
            permission: "sample_module.events.handle".to_string(),
            idempotency: crate::ModuleBindingIdempotency::Required,
            limit_profile: "event".to_string(),
            capabilities: Vec::new(),
            event_topics: vec!["sample.event".to_string()],
            schedule: None,
            http: None,
        };
        ModuleArtifactPackage {
            reference: OciArtifactReference {
                registry: "registry.example".to_string(),
                repository: "modules/sample_module".to_string(),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            descriptor: ModuleArtifactDescriptor {
                schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
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
                bindings: vec![binding],
                dependencies: Vec::new(),
                permissions: vec![ArtifactPermissionDescriptor {
                    key: "sample_module.events.handle".to_string(),
                    localizations: vec![rustok_api::ArtifactPermissionLocalization {
                        locale: "en".to_string(),
                        label: "Handle sample event".to_string(),
                        description: "Handle the sample event".to_string(),
                    }],
                }],
                schema_documents: vec![input_schema, output_schema],
                settings_schema_digest: None,
                data_schema_digest: None,
                ui_contributions: Vec::new(),
                persistence_contract: None,
            },
            media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            payload: crate::ArtifactPayloadSource::Bytes(payload),
        }
    }

    fn schema_document(document: Value) -> ArtifactSchemaDocument {
        ArtifactSchemaDocument {
            digest: canonical_schema_digest(&document),
            document,
        }
    }

    fn installed(package: &ModuleArtifactPackage) -> InstalledModuleArtifact {
        InstalledModuleArtifact {
            installation_id: Uuid::new_v4(),
            scope: ModuleInstallationScope::Platform,
            reference: package.reference.clone(),
            release: ArtifactReleaseRef {
                slug: package.descriptor.slug.clone(),
                version: package.descriptor.version.clone(),
                digest: package.descriptor.artifact_digest.clone(),
            },
            descriptor: package.descriptor.clone(),
            payload_media_type: package.media_type.clone(),
            dependency_lock: ModuleDependencyLockGraph::create(0, Vec::new())
                .expect("empty dependency lock"),
            capability_grant_revision: 1,
            installed_at: Utc::now(),
        }
    }

    #[test]
    fn http_binding_clamps_the_effective_sandbox_wall_clock_limit() {
        let binding = ModuleRuntimeBinding {
            id: "http_status".to_string(),
            kind: ModuleRuntimeBindingKind::Http,
            entrypoint: "status".to_string(),
            input_schema_digest: format!("sha256:{}", "a".repeat(64)),
            output_schema_digest: format!("sha256:{}", "b".repeat(64)),
            permission: "sample_module.http.status.read".to_string(),
            idempotency: crate::ModuleBindingIdempotency::Required,
            limit_profile: "http_json".to_string(),
            capabilities: Vec::new(),
            event_topics: Vec::new(),
            schedule: None,
            http: Some(crate::ModuleHttpBinding {
                method: crate::ModuleHttpMethod::Get,
                path: "status".to_string(),
                request_media_type: "application/json".to_string(),
                response_media_type: "application/json".to_string(),
                max_body_bytes: 1_024,
                max_output_bytes: 1_024,
                timeout_ms: 500,
                streaming: crate::ModuleHttpStreamingPolicy::Forbidden,
            }),
        };
        let mut policy = SandboxPolicy::default();
        policy.limits.wall_clock_ms = 2_000;

        let effective = effective_binding_policy(&binding, ExecutionPhase::Http, policy)
            .expect("HTTP binding has a timeout");
        assert_eq!(effective.limits.wall_clock_ms, 500);

        let mut stricter_policy = SandboxPolicy::default();
        stricter_policy.limits.wall_clock_ms = 100;
        let effective = effective_binding_policy(&binding, ExecutionPhase::Http, stricter_policy)
            .expect("HTTP binding has a timeout");
        assert_eq!(effective.limits.wall_clock_ms, 100);
    }

    #[tokio::test]
    async fn installed_artifact_is_resolved_verified_and_executed_by_shared_sandbox() {
        let package = package();
        let installed = installed(&package);
        let binding = installed.descriptor.bindings[0].clone();
        let observed = Arc::new(Mutex::new(None));
        let mut executors = ExecutorRegistry::new();
        executors
            .register(RecordingExecutor::new(
                Arc::clone(&observed),
                json!({ "executed": true }),
            ))
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
            .execute_binding(
                &installed,
                &binding,
                context.clone(),
                ArtifactBindingDispatchEnvelope::new(
                    &binding,
                    ExecutionPhase::Event,
                    json!({ "id": 1 }),
                ),
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
        let envelope = serde_json::from_value::<ArtifactBindingDispatchEnvelope>(
            RhaiBindingInput::decode(request.input)
                .expect("versioned Rhai input")
                .input,
        )
        .expect("versioned artifact dispatch envelope");
        assert_eq!(
            envelope.dispatch_version,
            crate::ARTIFACT_BINDING_DISPATCH_ENVELOPE_VERSION
        );
        assert_eq!(envelope.binding_id, binding.id);
        assert_eq!(envelope.binding_kind, binding.kind);
        assert_eq!(envelope.phase, ExecutionPhase::Event);
        assert_eq!(envelope.payload, json!({ "id": 1 }));
    }

    #[tokio::test]
    async fn execution_fails_closed_when_admitted_blob_is_unavailable() {
        let package = package();
        let installed = installed(&package);
        let binding = installed.descriptor.bindings[0].clone();
        let runtime = ArtifactRuntime::new(
            InMemoryArtifactBlobStore::default(),
            SandboxRuntime::new(ExecutorRegistry::new(), Arc::new(DenyBroker)),
        );

        assert!(matches!(
            runtime
                .execute_binding(
                    &installed,
                    &binding,
                    SandboxContext::new(ExecutionPhase::Event),
                    ArtifactBindingDispatchEnvelope::new(
                        &binding,
                        ExecutionPhase::Event,
                        json!({ "id": 1 }),
                    ),
                    SandboxPolicy::default(),
                )
                .await,
            Err(ArtifactRuntimeError::Installation(
                ModuleInstallationError::BlobNotFound(_)
            ))
        ));
    }

    #[tokio::test]
    async fn dispatch_envelope_rejects_version_or_binding_tampering_before_payload_read() {
        let package = package();
        let installed = installed(&package);
        let binding = installed.descriptor.bindings[0].clone();
        let runtime = ArtifactRuntime::new(
            InMemoryArtifactBlobStore::default(),
            SandboxRuntime::new(ExecutorRegistry::new(), Arc::new(DenyBroker)),
        );
        let mut envelope = ArtifactBindingDispatchEnvelope::new(
            &binding,
            ExecutionPhase::Event,
            json!({ "id": 1 }),
        );
        envelope.dispatch_version += 1;

        assert!(matches!(
            runtime
                .execute_binding(
                    &installed,
                    &binding,
                    SandboxContext::new(ExecutionPhase::Event),
                    envelope,
                    SandboxPolicy::default(),
                )
                .await,
            Err(ArtifactRuntimeError::DispatchEnvelope(
                ArtifactBindingDispatchEnvelopeError::UnsupportedVersion
            ))
        ));
    }

    #[tokio::test]
    async fn binding_input_schema_rejects_before_payload_read_or_sandbox_execution() {
        let package = package();
        let installed = installed(&package);
        let binding = installed.descriptor.bindings[0].clone();
        let observed = Arc::new(Mutex::new(None));
        let mut executors = ExecutorRegistry::new();
        executors
            .register(RecordingExecutor::new(
                Arc::clone(&observed),
                json!({ "executed": true }),
            ))
            .expect("executor registration");
        let runtime = ArtifactRuntime::new(
            InMemoryArtifactBlobStore::default(),
            SandboxRuntime::new(executors, Arc::new(DenyBroker)),
        );

        assert!(matches!(
            runtime
                .execute_binding(
                    &installed,
                    &binding,
                    SandboxContext::new(ExecutionPhase::Event),
                    ArtifactBindingDispatchEnvelope::new(
                        &binding,
                        ExecutionPhase::Event,
                        json!({ "id": "invalid" }),
                    ),
                    SandboxPolicy::default(),
                )
                .await,
            Err(ArtifactRuntimeError::BindingSchemaViolation { stage: "input", .. })
        ));
        assert!(observed.lock().expect("request lock").is_none());
    }

    #[tokio::test]
    async fn binding_output_schema_rejects_after_sandbox_execution() {
        let package = package();
        let installed = installed(&package);
        let binding = installed.descriptor.bindings[0].clone();
        let observed = Arc::new(Mutex::new(None));
        let mut executors = ExecutorRegistry::new();
        executors
            .register(RecordingExecutor::new(
                Arc::clone(&observed),
                json!({ "executed": "invalid" }),
            ))
            .expect("executor registration");
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
        let runtime =
            ArtifactRuntime::new(blobs, SandboxRuntime::new(executors, Arc::new(DenyBroker)));

        assert!(matches!(
            runtime
                .execute_binding(
                    &installed,
                    &binding,
                    SandboxContext::new(ExecutionPhase::Event),
                    ArtifactBindingDispatchEnvelope::new(
                        &binding,
                        ExecutionPhase::Event,
                        json!({ "id": 1 }),
                    ),
                    SandboxPolicy::default(),
                )
                .await,
            Err(ArtifactRuntimeError::BindingSchemaViolation {
                stage: "output",
                ..
            })
        ));
        assert!(observed.lock().expect("request lock").is_some());
    }
}
