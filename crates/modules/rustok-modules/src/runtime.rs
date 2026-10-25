use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;

use rustok_sandbox::{
    ExecutionPhase, RhaiBindingOutput, SandboxContext, SandboxExecutorPlacement, SandboxLimits,
    SandboxOutcome, SandboxPolicy, SandboxRuntime,
};

use crate::{
    ArtifactBindingDispatch, ArtifactBindingDispatchEnvelope, ArtifactBindingDispatchEnvelopeError,
    ArtifactBindingDispatchError, ArtifactBindingExecutor, ArtifactBlobStore, ArtifactPayloadKind,
    ArtifactReleaseRef, ExecutorPoolIdentity, ExecutorReadinessError, ExecutorReadinessProbe,
    ExecutorSmokeProbeOutcome, ExecutorSmokeProbeRequest, InstalledModuleArtifact,
    ModuleArtifactError, ModuleEffectivePolicy, ModuleInstallationError, ModuleInstallationScope,
    ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    artifact_schema::{ArtifactSchemaValidationError, ArtifactSchemaValidatorCache},
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

#[async_trait]
impl<B> ArtifactBlobStore for VerifiedArtifactNodeCache<B>
where
    B: ArtifactBlobStore,
{
    async fn put_verified(
        &self,
        digest: &str,
        bytes: &[u8],
    ) -> Result<(), ModuleInstallationError> {
        self.durable.put_verified(digest, bytes).await
    }

    async fn get_verified(&self, digest: &str) -> Result<Vec<u8>, ModuleInstallationError> {
        Self::get_verified(self, digest).await
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
    validated_descriptors: ValidatedDescriptorCache,
}

/// Bounded node-local record of descriptor digests that already passed
/// full structural validation in this process. Descriptors are immutable
/// per installation and validation is a pure function of descriptor
/// content, so re-validating on every dispatch (which recomputes every
/// schema digest) is repeated work; the canonical descriptor digest is
/// the cache key. The cache never gates safety: a miss — or a poisoned
/// cache lock — falls back to full validation, and a digest is recorded
/// only after validation succeeded.
#[derive(Default)]
struct ValidatedDescriptorCache {
    state: std::sync::Mutex<ValidatedDescriptorCacheState>,
}

#[derive(Default)]
struct ValidatedDescriptorCacheState {
    digests: HashSet<String>,
    lru: VecDeque<String>,
}

const MAX_VALIDATED_DESCRIPTORS: usize = 128;

impl ValidatedDescriptorCache {
    fn contains(&self, digest: &str) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state.digests.contains(digest) {
            state.lru.retain(|candidate| candidate != digest);
            state.lru.push_back(digest.to_string());
            true
        } else {
            false
        }
    }

    fn insert(&self, digest: String) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if !state.digests.insert(digest.clone()) {
            state.lru.retain(|candidate| candidate != &digest);
            state.lru.push_back(digest);
            return;
        }
        state.lru.push_back(digest);
        while state.digests.len() > MAX_VALIDATED_DESCRIPTORS {
            let Some(oldest) = state.lru.pop_front() else {
                break;
            };
            state.digests.remove(&oldest);
        }
    }
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
            validated_descriptors: ValidatedDescriptorCache::default(),
        }
    }

    fn validate_descriptor(
        &self,
        descriptor: &crate::ModuleArtifactDescriptor,
    ) -> Result<(), ArtifactRuntimeError> {
        let digest = crate::canonical_artifact_descriptor_digest(descriptor);
        if self.validated_descriptors.contains(&digest) {
            return Ok(());
        }
        descriptor
            .validate()
            .map_err(ArtifactRuntimeError::Descriptor)?;
        self.validated_descriptors.insert(digest);
        Ok(())
    }

    pub fn supports_payload_kind(&self, payload_kind: crate::ArtifactPayloadKind) -> bool {
        payload_kind
            .sandbox_executor()
            .is_some_and(|executor| self.sandbox.supports_executor(executor))
    }

    pub async fn execute_binding(
        &self,
        artifact: &InstalledModuleArtifact,
        binding: &ModuleRuntimeBinding,
        context: SandboxContext,
        input: ArtifactBindingDispatchEnvelope,
        policy: SandboxPolicy,
    ) -> Result<SandboxOutcome, ArtifactRuntimeError> {
        self.validate_descriptor(&artifact.descriptor)?;
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

/// Production readiness probe backed by the shared artifact runtime.
///
/// The host composition supplies the exact pool identity and the capability
/// names it actually registered with its broker. The pool's claimed placement
/// is not trusted on its own: for every payload kind it is cross-checked
/// against the placement under which that executor was registered with the
/// sandbox runtime, so an in-process executor can never mint an
/// isolated-worker readiness receipt. A smoke execution always
/// uses an empty grant set, so probing a not-yet-active candidate cannot
/// mutate tenant/platform state through a capability. The capability budget
/// remains at the sandbox profile's positive minimum because zero-valued
/// limits are invalid; the empty grant set, not the budget, is the deny
/// boundary. The runtime still reads the exact installed descriptor's
/// payload from CAS and validates binding input/output schemas.
pub struct ArtifactRuntimeReadinessProbe<B> {
    runtime: ArtifactRuntime<B>,
    pool: ExecutorPoolIdentity,
    capability_routes: HashSet<String>,
}

impl<B> ArtifactRuntimeReadinessProbe<B>
where
    B: ArtifactBlobStore,
{
    pub fn new(
        runtime: ArtifactRuntime<B>,
        pool: ExecutorPoolIdentity,
        capability_routes: impl IntoIterator<Item = String>,
    ) -> Result<Self, ExecutorReadinessError> {
        pool.validate()?;
        let capability_routes = capability_routes.into_iter().collect::<HashSet<_>>();
        if capability_routes
            .iter()
            .any(|route| route.trim().is_empty() || route.trim() != route)
        {
            return Err(ExecutorReadinessError::InvalidConfiguration(
                "capability route names must be non-empty canonical names".to_string(),
            ));
        }
        Ok(Self {
            runtime,
            pool,
            capability_routes,
        })
    }

    fn actual_executor_placement(
        &self,
        payload_kind: ArtifactPayloadKind,
    ) -> Result<SandboxExecutorPlacement, ExecutorReadinessError> {
        let executor = payload_kind
            .sandbox_executor()
            .ok_or(ExecutorReadinessError::UnsupportedPayloadKind(payload_kind))?;
        self.runtime
            .sandbox
            .executor_placement(executor)
            .map_err(|error| ExecutorReadinessError::ProbeUnavailable(error.to_string()))
    }

    fn ensure_pool_placement_matches(
        &self,
        payload_kind: ArtifactPayloadKind,
    ) -> Result<(), ExecutorReadinessError> {
        let actual_placement = self.actual_executor_placement(payload_kind)?;
        if actual_placement != self.pool.placement {
            return Err(ExecutorReadinessError::InvalidConfiguration(format!(
                "executor pool `{}` claims placement `{}` but the registered \
                 {payload_kind:?} executor placement is `{actual_placement}`",
                self.pool.pool_id, self.pool.placement,
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl<B> ExecutorReadinessProbe for ArtifactRuntimeReadinessProbe<B>
where
    B: ArtifactBlobStore,
{
    fn pool_identity(&self) -> Result<ExecutorPoolIdentity, ExecutorReadinessError> {
        self.pool.validate()?;
        Ok(self.pool.clone())
    }

    fn supports_payload_kind(&self, payload_kind: ArtifactPayloadKind) -> bool {
        self.runtime.supports_payload_kind(payload_kind)
            && self.ensure_pool_placement_matches(payload_kind).is_ok()
    }

    fn has_capability_route(&self, capability: &str) -> bool {
        self.capability_routes.contains(capability)
    }

    async fn execute_smoke(
        &self,
        request: ExecutorSmokeProbeRequest<'_>,
    ) -> Result<ExecutorSmokeProbeOutcome, ExecutorReadinessError> {
        self.ensure_pool_placement_matches(request.artifact.descriptor.payload_kind)?;
        if !matches!(
            &request.binding.kind,
            ModuleRuntimeBindingKind::ActivationSmoke | ModuleRuntimeBindingKind::Readiness
        ) || !request.binding.capabilities.is_empty()
        {
            return Err(ExecutorReadinessError::SmokeExecutionFailed(format!(
                "binding `{}` is not a capability-free smoke/readiness binding",
                request.binding.id
            )));
        }
        let mut context = SandboxContext::new(ExecutionPhase::Lifecycle);
        context.tenant_id = match request.artifact.scope {
            ModuleInstallationScope::Platform => None,
            ModuleInstallationScope::Tenant { tenant_id } => Some(tenant_id),
        };
        context.actor_id = Some(request.context.actor_id.to_string());
        context.trace_id = Some(request.context.trace_id.clone());
        context.audit_label = Some(request.binding.id.clone());
        let input = ArtifactBindingDispatchEnvelope::new(
            request.binding,
            ExecutionPhase::Lifecycle,
            serde_json::json!({}),
        );
        let policy = SandboxPolicy {
            grants: Vec::new(),
            limits: SandboxLimits {
                wall_clock_ms: 5_000,
                max_capability_calls: 1,
                max_capability_calls_per_second: 1,
                ..SandboxLimits::default()
            },
        };
        let outcome = self
            .runtime
            .execute_binding(request.artifact, request.binding, context, input, policy)
            .await
            .map_err(|error| {
                ExecutorReadinessError::SmokeExecutionFailed(error.to_string())
            })?;
        let output_bytes = serde_json::to_vec(&outcome.output).map_err(|error| {
            ExecutorReadinessError::SmokeExecutionFailed(format!(
                "smoke output could not be canonicalized for its receipt digest: {error}"
            ))
        })?;
        Ok(ExecutorSmokeProbeOutcome {
            binding_id: request.binding.id.clone(),
            output_digest: crate::installation::sha256_digest(&output_bytes),
        })
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

/// Failure of an [`ArtifactInstallationResolver`] port. `Unavailable` and
/// `NotActiveSelection` are durable-state denials (no active installation can
/// satisfy the request); callers must be able to distinguish them from the
/// infrastructure failures carried by `Store`.
#[derive(Debug, Error)]
pub enum ArtifactInstallationResolveError {
    #[error("active artifact installation is unavailable for the requested tenant")]
    Unavailable,
    #[error("requested artifact installation is no longer the active tenant selection")]
    NotActiveSelection,
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Invariant(String),
    #[error("artifact installation resolution failed: {0}")]
    Store(String),
}

/// Failure of an [`ArtifactSandboxPolicyResolver`] port.
#[derive(Debug, Error)]
pub enum ArtifactSandboxPolicyResolveError {
    #[error("no eligible durable sandbox policy exists for the artifact installation")]
    Unavailable,
    #[error("{0}")]
    InvalidRequest(String),
    #[error("sandbox policy revision does not match the admitted installation")]
    RevisionMismatch,
    #[error("{0}")]
    Invariant(String),
    #[error("artifact sandbox policy resolution failed: {0}")]
    Store(String),
}

/// Failure of an [`ArtifactEffectivePolicyResolver`] port.
#[derive(Debug, Error)]
pub enum ArtifactEffectivePolicyResolveError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("artifact node is not ready for the admitted installation: {0}")]
    NodeNotReady(String),
    #[error("artifact effective policy resolution failed: {0}")]
    Store(String),
}

/// Resolves the immutable installation selected for an artifact lifecycle
/// dispatch. Implementations must apply platform/tenant scope and RLS.
#[async_trait]
pub trait ArtifactInstallationResolver: Send + Sync {
    async fn resolve(
        &self,
        release: &ArtifactReleaseRef,
        tenant_id: uuid::Uuid,
    ) -> Result<InstalledModuleArtifact, ArtifactInstallationResolveError>;

    /// Resolves the immutable installation carried by a durable work item.
    /// Implementations may override this with a direct installation lookup.
    /// The default is deliberately fail-closed: it permits execution only when
    /// the current effective selection still names the requested installation.
    async fn resolve_exact(
        &self,
        installation_id: uuid::Uuid,
        release: &ArtifactReleaseRef,
        tenant_id: uuid::Uuid,
    ) -> Result<InstalledModuleArtifact, ArtifactInstallationResolveError> {
        let artifact = self.resolve(release, tenant_id).await?;
        if artifact.installation_id != installation_id {
            return Err(ArtifactInstallationResolveError::NotActiveSelection);
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
    ) -> Result<SandboxPolicy, ArtifactSandboxPolicyResolveError>;
}

/// Resolves the canonical effective module policy and node-admission evidence
/// immediately before a non-lifecycle binding crosses the sandbox boundary.
/// The resolved installation is passed intact so an implementation can require
/// the durable observed assignment for its exact immutable identity.
#[async_trait]
pub trait ArtifactEffectivePolicyResolver: Send + Sync {
    async fn resolve(
        &self,
        tenant_id: uuid::Uuid,
        artifact: &InstalledModuleArtifact,
    ) -> Result<ModuleEffectivePolicy, ArtifactEffectivePolicyResolveError>;
}

/// Production adapter from admitted dispatcher bindings to the shared sandbox.
pub struct ArtifactRuntimeLifecycleExecutor<R, I, P, E> {
    runtime: ArtifactRuntime<R>,
    installations: I,
    policies: P,
    effective_policy: E,
}

impl<B, I, P, E> ArtifactRuntimeLifecycleExecutor<B, I, P, E>
where
    B: ArtifactBlobStore,
{
    pub fn new(
        runtime: ArtifactRuntime<B>,
        installations: I,
        policies: P,
        effective_policy: E,
    ) -> Self {
        Self {
            runtime,
            installations,
            policies,
            effective_policy,
        }
    }
}

fn effective_binding_policy(
    binding: &ModuleRuntimeBinding,
    phase: ExecutionPhase,
    mut policy: SandboxPolicy,
) -> Result<SandboxPolicy, ArtifactBindingDispatchError> {
    if phase == ExecutionPhase::Http {
        let timeout_ms = binding
            .http
            .as_ref()
            .ok_or(ArtifactBindingDispatchError::HttpBindingRequired)?
            .timeout_ms;
        policy.limits.wall_clock_ms = policy.limits.wall_clock_ms.min(timeout_ms);
    }
    Ok(policy)
}

#[async_trait]
impl<B, I, P, E> ArtifactBindingExecutor for ArtifactRuntimeLifecycleExecutor<B, I, P, E>
where
    B: ArtifactBlobStore,
    I: ArtifactInstallationResolver,
    P: ArtifactSandboxPolicyResolver,
    E: ArtifactEffectivePolicyResolver,
{
    fn supports_payload_kind(&self, payload_kind: crate::ArtifactPayloadKind) -> bool {
        self.runtime.supports_payload_kind(payload_kind)
    }

    async fn dispatch_binding(
        &self,
        dispatch: ArtifactBindingDispatch<'_>,
    ) -> Result<Value, ArtifactBindingDispatchError> {
        if !dispatch.context.is_valid() {
            return Err(ArtifactBindingDispatchError::InvalidContext);
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
        dispatch.admission.authorize(
            dispatch.release,
            dispatch.binding,
            dispatch.target,
            dispatch.tenant_id,
            dispatch.phase,
            &artifact,
        )?;
        if dispatch.phase != ExecutionPhase::Lifecycle {
            let policy = self
                .effective_policy
                .resolve(dispatch.tenant_id, &artifact)
                .await?;
            if !policy.contains(&artifact.release.slug) {
                return Err(ArtifactBindingDispatchError::DeniedByPolicy {
                    slug: artifact.release.slug.clone(),
                    policy_revision: policy.policy_revision().to_string(),
                });
            }
        }
        let policy = effective_binding_policy(
            dispatch.binding,
            dispatch.phase,
            self.policies.resolve(&artifact, dispatch.tenant_id).await?,
        )?;
        let mut context = SandboxContext::new(dispatch.phase);
        context.tenant_id = Some(dispatch.tenant_id);
        context.actor_id = dispatch.context.actor_id.clone();
        context.trace_id = dispatch.context.trace_id.clone();
        context.audit_label = Some(dispatch.binding.id.clone());
        let input =
            ArtifactBindingDispatchEnvelope::new(dispatch.binding, dispatch.phase, dispatch.input);
        Ok(self
            .runtime
            .execute_binding(&artifact, dispatch.binding, context, input, policy)
            .await?
            .output)
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

impl ArtifactRuntimeError {
    /// Whether retrying the same work item (same admitted descriptor, same
    /// input) can never succeed. Descriptor/binding/schema defects are
    /// properties of the admitted release and the recorded input, so they
    /// are deterministic; sandbox and installation-store failures can be
    /// transient resource/infrastructure conditions and stay retryable
    /// within the queue's attempt budget.
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::BindingNotAdmitted(_)
            | Self::RhaiBinding(_)
            | Self::DispatchEnvelopeEncoding(_)
            | Self::DispatchEnvelope(_)
            | Self::Descriptor(_)
            | Self::BindingSchemaNotAdmitted { .. }
            | Self::SchemaCompilation { .. }
            | Self::BindingSchemaViolation { .. } => true,
            Self::SchemaValidatorCachePoisoned | Self::Installation(_) | Self::Sandbox(_) => {
                false
            }
        }
    }

    /// Stable, redacted failure code safe to persist in durable delivery
    /// records. Free-form error text is never a stored code: it can carry
    /// guest input and makes operator triage queries unreliable.
    pub fn code(&self) -> &'static str {
        match self {
            Self::BindingNotAdmitted(_) => "binding_not_admitted",
            Self::RhaiBinding(_) => "binding_invalid",
            Self::DispatchEnvelopeEncoding(_) => "dispatch_envelope_encoding",
            Self::DispatchEnvelope(_) => "dispatch_envelope_mismatch",
            Self::Descriptor(_) => "descriptor_invalid",
            Self::BindingSchemaNotAdmitted { .. } => "binding_schema_not_admitted",
            Self::SchemaCompilation { .. } => "schema_compilation_failed",
            Self::BindingSchemaViolation { .. } => "binding_schema_violation",
            Self::SchemaValidatorCachePoisoned => "schema_validator_unavailable",
            Self::Installation(_) => "installation_store_failed",
            Self::Sandbox(_) => "sandbox_execution_failed",
        }
    }
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
        SandboxExecutor, SandboxExecutorKind, SandboxExecutorPlacement, SandboxHost,
        SandboxOutcome, SandboxRequest, SandboxResult,
    };

    use super::*;
    use crate::{
        ArtifactModuleKind, ArtifactPayloadKind, ArtifactPermissionDescriptor, ArtifactReleaseRef,
        ArtifactSchemaDocument, InMemoryArtifactBlobStore, ModuleArtifactDescriptor,
        ModuleArtifactPackage, ModuleDependencyLockGraph, ModuleInstallationScope,
        ModuleRuntimeBinding, ModuleRuntimeBindingKind, OciArtifactReference, RuntimeFingerprint,
        canonical_schema_digest,
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
                rhai_scope: None,
                metrics: ExecutionMetrics::default(),
            })
        }
    }

    struct FixedInstallation(InstalledModuleArtifact);

    #[async_trait]
    impl ArtifactInstallationResolver for FixedInstallation {
        async fn resolve(
            &self,
            release: &ArtifactReleaseRef,
            _tenant_id: Uuid,
        ) -> Result<InstalledModuleArtifact, ArtifactInstallationResolveError> {
            if release != &self.0.release {
                return Err(ArtifactInstallationResolveError::Unavailable);
            }
            Ok(self.0.clone())
        }
    }

    struct DeniedSandboxPolicy;

    #[async_trait]
    impl ArtifactSandboxPolicyResolver for DeniedSandboxPolicy {
        async fn resolve(
            &self,
            _artifact: &InstalledModuleArtifact,
            _tenant_id: Uuid,
        ) -> Result<SandboxPolicy, ArtifactSandboxPolicyResolveError> {
            Err(ArtifactSandboxPolicyResolveError::Unavailable)
        }
    }

    struct RecordingEffectivePolicy(Arc<Mutex<Option<Uuid>>>);

    #[async_trait]
    impl ArtifactEffectivePolicyResolver for RecordingEffectivePolicy {
        async fn resolve(
            &self,
            _tenant_id: Uuid,
            artifact: &InstalledModuleArtifact,
        ) -> Result<ModuleEffectivePolicy, ArtifactEffectivePolicyResolveError> {
            *self.0.lock().expect("effective policy lock") = Some(artifact.installation_id);
            Err(ArtifactEffectivePolicyResolveError::NodeNotReady(
                "durable node assignment is unavailable".to_string(),
            ))
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
                localization_catalogs: Vec::new(),
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
            data_owner_id: Uuid::new_v4(),
            namespace_instance_id: package
                .descriptor
                .persistence_contract
                .as_ref()
                .map(|_| Uuid::new_v4()),
            secret_instance_id: package
                .descriptor
                .capabilities
                .iter()
                .any(|capability| capability.as_str() == "platform.secrets")
                .then(Uuid::new_v4),
            settings_instance_id: Uuid::new_v4(),
            scope: ModuleInstallationScope::Platform,
            reference: package.reference.clone(),
            release: ArtifactReleaseRef {
                slug: package.descriptor.slug.clone(),
                version: package.descriptor.version.clone(),
                digest: package.descriptor.artifact_digest.clone(),
            },
            descriptor: package.descriptor.clone(),
            payload_media_type: package.media_type.clone(),
            dependency_lock: ModuleDependencyLockGraph::create(1, Vec::new())
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
    async fn policy_gate_receives_the_exact_resolved_installation_before_execution() {
        let package = package();
        let installed = installed(&package);
        let binding = installed.descriptor.bindings[0].clone();
        let observed = Arc::new(Mutex::new(None));
        let executor = ArtifactRuntimeLifecycleExecutor::new(
            ArtifactRuntime::new(
                InMemoryArtifactBlobStore::default(),
                SandboxRuntime::new(ExecutorRegistry::new(), Arc::new(DenyBroker)),
            ),
            FixedInstallation(installed.clone()),
            DeniedSandboxPolicy,
            RecordingEffectivePolicy(Arc::clone(&observed)),
        );

        let tenant_id = Uuid::new_v4();
        let error = executor
            .dispatch_binding(ArtifactBindingDispatch {
                release: &installed.release,
                binding: &binding,
                target: crate::ArtifactInstallationTarget::CurrentRelease,
                tenant_id,
                admission: crate::ArtifactExecutionAdmission::owner_revalidation(
                    tenant_id,
                    &installed.release,
                    crate::ArtifactInstallationTarget::CurrentRelease,
                ),
                input: json!({ "id": 1 }),
                phase: ExecutionPhase::Event,
                context: crate::ArtifactBindingExecutionContext::default(),
            })
            .await
            .expect_err("durable policy gate must deny before sandbox execution");

        assert!(matches!(
            error,
            ArtifactBindingDispatchError::EffectivePolicy(
                ArtifactEffectivePolicyResolveError::NodeNotReady(message)
            ) if message == "durable node assignment is unavailable"
        ));
        assert_eq!(
            *observed.lock().expect("effective policy lock"),
            Some(installed.installation_id)
        );
    }

    #[tokio::test]
    async fn installed_artifact_is_resolved_verified_and_executed_by_shared_sandbox() {
        let package = package();
        let installed = installed(&package);
        let binding = installed.descriptor.bindings[0].clone();
        let observed = Arc::new(Mutex::new(None));
        let mut executors = ExecutorRegistry::new();
        executors
            .register_in_process(RecordingExecutor::new(
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
            .register_in_process(RecordingExecutor::new(
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
            .register_in_process(RecordingExecutor::new(
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

    #[test]
    fn readiness_probe_rejects_pool_placement_that_differs_from_registered_executor() {
        fn runtime() -> ArtifactRuntime<InMemoryArtifactBlobStore> {
            let mut executors = ExecutorRegistry::new();
            executors
                .register_in_process(RecordingExecutor::new(
                    Arc::new(Mutex::new(None)),
                    json!({ "executed": true }),
                ))
                .expect("executor registration");
            ArtifactRuntime::new(
                InMemoryArtifactBlobStore::default(),
                SandboxRuntime::new(executors, Arc::new(DenyBroker)),
            )
        }
        fn pool(placement: SandboxExecutorPlacement) -> ExecutorPoolIdentity {
            let isolated = placement == SandboxExecutorPlacement::IsolatedWorker;
            ExecutorPoolIdentity {
                pool_id: "placement-test".to_string(),
                pool_generation: 1,
                fingerprint: RuntimeFingerprint {
                    executor_binary_digest: format!("sha256:{}", "a".repeat(64)),
                    engine_build_digest: format!("sha256:{}", "b".repeat(64)),
                    engine_config_revision: "placement-test".to_string(),
                    isolated_worker_image_digest: isolated
                        .then(|| format!("sha256:{}", "c".repeat(64))),
                    target_cpu_contract: "placement-test".to_string(),
                    runtime_abi: "placement-test".to_string(),
                },
                placement,
                placement_attestation: isolated.then(|| "worker-attestation".to_string()),
            }
        }

        let matching = ArtifactRuntimeReadinessProbe::new(
            runtime(),
            pool(SandboxExecutorPlacement::InProcess),
            Vec::new(),
        )
        .expect("matching readiness probe");
        assert!(matching.supports_payload_kind(ArtifactPayloadKind::Rhai));

        let mismatched = ArtifactRuntimeReadinessProbe::new(
            runtime(),
            pool(SandboxExecutorPlacement::IsolatedWorker),
            Vec::new(),
        )
        .expect("mismatched readiness probe");
        assert!(!mismatched.supports_payload_kind(ArtifactPayloadKind::Rhai));
        assert!(matches!(
            mismatched.ensure_pool_placement_matches(ArtifactPayloadKind::Rhai),
            Err(ExecutorReadinessError::InvalidConfiguration(_))
        ));
    }

    #[tokio::test]
    async fn verified_node_cache_rehashes_and_replaces_a_corrupt_cache_hit() {
        let payload = b"admitted node-cache payload";
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(payload)));
        let durable = InMemoryArtifactBlobStore::default();
        durable
            .put_verified(&digest, payload)
            .await
            .expect("durable payload");
        let cache =
            VerifiedArtifactNodeCache::new(durable, payload.len() * 2).expect("cache capacity");

        assert_eq!(
            cache.get_verified(&digest).await.expect("first read"),
            payload
        );
        let mut state = cache.state.lock().await;
        state
            .entries
            .insert(digest.clone(), vec![0_u8; payload.len()]);
        drop(state);

        assert_eq!(
            cache
                .get_verified(&digest)
                .await
                .expect("rehash and reload"),
            payload
        );
    }
}
