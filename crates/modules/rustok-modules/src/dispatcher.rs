//! Definition-aware dispatch for module runtime bindings.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rustok_api::{ArtifactUiActionConfirmation, ArtifactUiSurface};
use rustok_core::{ModuleContext, ModuleRegistry};
use rustok_sandbox::ExecutionPhase;

use crate::{
    ArtifactReleaseRef, ArtifactUiAuditPolicy, ArtifactUiContribution,
    ArtifactUiContributionContent, InstalledModuleArtifact, ModuleBindingIdempotency,
    ModuleDefinitionCatalog, ModuleDefinitionSource, ModuleHttpMethod, ModuleInstallationScope,
    ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    artifact::{event_topic_matches, valid_event_topic},
};

/// The v1 lifecycle binding set. Other binding classes are added to the same
/// envelope rather than becoming new host-specific call paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleLifecycleHookPhase {
    PreEnable,
    PostEnable,
    PreDisable,
    PostDisable,
}

/// Resolves a definition before reaching a static implementation handle or the
/// admitted artifact sandbox adapter.
pub struct ModuleExecutionDispatcher<'a> {
    catalog: &'a ModuleDefinitionCatalog,
    static_registry: Option<&'a ModuleRegistry>,
    artifact_executor: Option<&'a dyn ArtifactLifecycleExecutor>,
}

/// Host-supplied, redacted execution identity for one artifact binding.
///
/// The descriptor never controls these fields. They let a transport preserve
/// its authenticated actor and correlation trace through sandbox capability
/// calls and durable audit evidence without exposing headers or credentials to
/// an artifact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBindingExecutionContext {
    pub actor_id: Option<String>,
    pub trace_id: Option<String>,
}

impl ArtifactBindingExecutionContext {
    const MAX_IDENTITY_BYTES: usize = 256;

    pub fn is_valid(&self) -> bool {
        self.actor_id
            .as_ref()
            .is_none_or(|value| !value.is_empty() && value.len() <= Self::MAX_IDENTITY_BYTES)
            && self
                .trace_id
                .as_ref()
                .is_none_or(|value| !value.is_empty() && value.len() <= Self::MAX_IDENTITY_BYTES)
    }
}

/// One binding's outcome from an event fan-out dispatch. Every matching
/// binding is attempted; a failure is reported here instead of aborting the
/// remaining fan-out, so executed side effects are never hidden.
#[derive(Debug)]
pub struct ArtifactEventBindingOutcome {
    pub binding_id: String,
    pub result: Result<serde_json::Value, ModuleDispatchError>,
}

pub struct ArtifactCommandBindingRequest<'a> {
    pub release: &'a ArtifactReleaseRef,
    pub bindings: &'a [ModuleRuntimeBinding],
    pub target: ArtifactInstallationTarget,
    pub tenant_id: uuid::Uuid,
    /// Policy-owner admission for this exact release/target/tenant. Hosts
    /// obtain it from `ModuleEffectivePolicy::admit_artifact_execution`;
    /// it cannot be constructed from descriptor or transport data.
    pub admission: ArtifactExecutionAdmission,
    pub binding_id: &'a str,
    pub input: serde_json::Value,
    pub context: ArtifactBindingExecutionContext,
}

pub struct ArtifactHttpBindingRequest<'a> {
    pub release: &'a ArtifactReleaseRef,
    pub bindings: &'a [ModuleRuntimeBinding],
    pub target: ArtifactInstallationTarget,
    pub tenant_id: uuid::Uuid,
    /// Policy-owner admission for this exact release/target/tenant (see
    /// [`ArtifactCommandBindingRequest::admission`]).
    pub admission: ArtifactExecutionAdmission,
    pub method: ModuleHttpMethod,
    pub path: &'a str,
    pub body: serde_json::Value,
    pub context: ArtifactBindingExecutionContext,
}

/// The only artifact runtime dispatch envelope supported by the v1 module
/// control plane. The host owns every field except `payload`; artifacts cannot
/// select a binding, execution phase, or installation through their input.
pub const ARTIFACT_BINDING_DISPATCH_ENVELOPE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactBindingDispatchEnvelope {
    pub dispatch_version: u32,
    pub binding_id: String,
    pub binding_kind: ModuleRuntimeBindingKind,
    pub phase: ExecutionPhase,
    pub payload: serde_json::Value,
}

impl ArtifactBindingDispatchEnvelope {
    pub fn new(
        binding: &ModuleRuntimeBinding,
        phase: ExecutionPhase,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            dispatch_version: ARTIFACT_BINDING_DISPATCH_ENVELOPE_VERSION,
            binding_id: binding.id.clone(),
            binding_kind: binding.kind.clone(),
            phase,
            payload,
        }
    }

    pub fn validate_for(
        &self,
        binding: &ModuleRuntimeBinding,
        phase: ExecutionPhase,
    ) -> Result<(), ArtifactBindingDispatchEnvelopeError> {
        if self.dispatch_version != ARTIFACT_BINDING_DISPATCH_ENVELOPE_VERSION {
            return Err(ArtifactBindingDispatchEnvelopeError::UnsupportedVersion);
        }
        if self.binding_id != binding.id || self.binding_kind != binding.kind {
            return Err(ArtifactBindingDispatchEnvelopeError::BindingMismatch);
        }
        if self.phase != phase {
            return Err(ArtifactBindingDispatchEnvelopeError::PhaseMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactBindingDispatchEnvelopeError {
    #[error("artifact binding dispatch envelope has an unsupported version")]
    UnsupportedVersion,
    #[error("artifact binding dispatch envelope does not match the admitted binding")]
    BindingMismatch,
    #[error("artifact binding dispatch envelope does not match the execution phase")]
    PhaseMismatch,
}

impl<'a> ModuleExecutionDispatcher<'a> {
    pub fn new(catalog: &'a ModuleDefinitionCatalog, static_registry: &'a ModuleRegistry) -> Self {
        Self {
            catalog,
            static_registry: Some(static_registry),
            artifact_executor: None,
        }
    }

    /// Creates a dispatcher for an artifact-only composition. Static
    /// definitions remain unavailable because no compiled registry is present.
    pub fn artifact_only(
        catalog: &'a ModuleDefinitionCatalog,
        artifact_executor: &'a dyn ArtifactLifecycleExecutor,
    ) -> Self {
        Self {
            catalog,
            static_registry: None,
            artifact_executor: Some(artifact_executor),
        }
    }

    pub fn with_artifact_executor(mut self, executor: &'a dyn ArtifactLifecycleExecutor) -> Self {
        self.artifact_executor = Some(executor);
        self
    }

    pub fn catalog(&self) -> &ModuleDefinitionCatalog {
        self.catalog
    }

    pub async fn dispatch_lifecycle(
        &self,
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        config: &serde_json::Value,
        phase: ModuleLifecycleHookPhase,
    ) -> Result<(), ModuleDispatchError> {
        let definition = self
            .catalog
            .get(module_slug)
            .ok_or_else(|| ModuleDispatchError::UnknownDefinition(module_slug.to_string()))?;
        match &definition.source {
            ModuleDefinitionSource::PlatformNative { .. }
            | ModuleDefinitionSource::PromotedNative { .. } => {
                let static_registry = self.static_registry.ok_or_else(|| {
                    ModuleDispatchError::MissingStaticImplementation(module_slug.to_string())
                })?;
                let module = static_registry.get(module_slug).ok_or_else(|| {
                    ModuleDispatchError::MissingStaticImplementation(module_slug.to_string())
                })?;
                let context = ModuleContext {
                    db,
                    tenant_id,
                    config,
                };
                let result = match phase {
                    ModuleLifecycleHookPhase::PreEnable => module.pre_enable(context).await,
                    ModuleLifecycleHookPhase::PostEnable => module.post_enable(context).await,
                    ModuleLifecycleHookPhase::PreDisable => module.pre_disable(context).await,
                    ModuleLifecycleHookPhase::PostDisable => module.post_disable(context).await,
                };
                result.map_err(|error| ModuleDispatchError::StaticHook(error.to_string()))
            }
            ModuleDefinitionSource::Artifact { release } => {
                let kind = lifecycle_binding_kind(phase);
                // Admission validation enforces one binding per lifecycle
                // kind; refuse to guess if a stored descriptor predates
                // that rule instead of silently running only the first.
                let mut matches = definition
                    .bindings
                    .iter()
                    .filter(|binding| binding.kind == kind);
                let binding = matches.next().ok_or_else(|| {
                    ModuleDispatchError::ArtifactBindingUnavailable(module_slug.to_string())
                })?;
                let extra = matches.count();
                if extra > 0 {
                    return Err(ModuleDispatchError::AmbiguousArtifactBinding {
                        module_slug: module_slug.to_string(),
                        kind: format!("{kind:?}"),
                        matches: extra + 1,
                    });
                }
                let executor = self.artifact_executor.ok_or_else(|| {
                    ModuleDispatchError::ArtifactExecutorUnavailable(module_slug.to_string())
                })?;
                executor
                    .dispatch_lifecycle(release, binding, tenant_id, config, phase)
                    .await
                    .map_err(ModuleDispatchError::ArtifactHook)
            }
        }
    }

    /// Dispatches an admitted non-lifecycle artifact binding through the same
    /// sandbox executor used by lifecycle. Static modules deliberately have no
    /// dynamic fallback: their host contracts remain typed and compiled.
    pub async fn dispatch_artifact_binding(
        &self,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        binding_id: &str,
        input: serde_json::Value,
        phase: ExecutionPhase,
        admission: ArtifactExecutionAdmission,
        context: ArtifactBindingExecutionContext,
    ) -> Result<serde_json::Value, ModuleDispatchError> {
        if !context.is_valid() {
            return Err(ModuleDispatchError::InvalidArtifactExecutionContext);
        }
        let definition = self
            .catalog
            .get(module_slug)
            .ok_or_else(|| ModuleDispatchError::UnknownDefinition(module_slug.to_string()))?;
        let ModuleDefinitionSource::Artifact { release } = &definition.source else {
            return Err(ModuleDispatchError::StaticDynamicBindingUnavailable(
                module_slug.to_string(),
            ));
        };
        let binding = definition.binding(binding_id).ok_or_else(|| {
            ModuleDispatchError::ArtifactBindingUnavailable(module_slug.to_string())
        })?;
        if !binding_allows_phase(binding.kind.clone(), phase) {
            return Err(ModuleDispatchError::BindingPhaseMismatch {
                binding_id: binding.id.clone(),
                phase,
            });
        }
        if !admission.matches_dispatch(
            release,
            binding,
            ArtifactInstallationTarget::CurrentRelease,
            tenant_id,
            phase,
        ) {
            return Err(ModuleDispatchError::AdmissionMismatch);
        }
        let executor = self.artifact_executor.ok_or_else(|| {
            ModuleDispatchError::ArtifactExecutorUnavailable(module_slug.to_string())
        })?;
        executor
            .dispatch_binding(ArtifactBindingDispatch {
                release,
                binding,
                target: ArtifactInstallationTarget::CurrentRelease,
                tenant_id,
                admission,
                input,
                phase,
                context,
            })
            .await
            .map_err(ModuleDispatchError::ArtifactHook)
    }

    /// Dispatches every admitted Event binding whose exact or terminal-wildcard
    /// subscription matches the supplied platform event type.
    ///
    /// Fan-out never aborts: each matching binding is attempted in
    /// declaration order and its individual outcome is returned, so the
    /// caller can tell exactly what ran. (Previously the first failure
    /// aborted the remaining bindings after earlier ones had already
    /// executed, and their outputs were discarded.) The supplied `context`
    /// (actor/trace) is forwarded to every binding instead of being
    /// replaced with a default.
    pub async fn dispatch_artifact_event(
        &self,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        event_type: &str,
        payload: serde_json::Value,
        admission: ArtifactExecutionAdmission,
        context: ArtifactBindingExecutionContext,
    ) -> Result<Vec<ArtifactEventBindingOutcome>, ModuleDispatchError> {
        if !valid_delivered_event_type(event_type) {
            return Err(ModuleDispatchError::InvalidArtifactEventType(
                event_type.to_string(),
            ));
        }
        if !context.is_valid() {
            return Err(ModuleDispatchError::InvalidArtifactExecutionContext);
        }
        let definition = self
            .catalog
            .get(module_slug)
            .ok_or_else(|| ModuleDispatchError::UnknownDefinition(module_slug.to_string()))?;
        let ModuleDefinitionSource::Artifact { release: _release } = &definition.source else {
            return Err(ModuleDispatchError::StaticDynamicBindingUnavailable(
                module_slug.to_string(),
            ));
        };
        let binding_ids = definition
            .bindings
            .iter()
            .filter(|binding| {
                binding.kind == ModuleRuntimeBindingKind::Event
                    && binding
                        .event_topics
                        .iter()
                        .any(|topic| event_topic_matches(topic, event_type))
            })
            .map(|binding| binding.id.clone())
            .collect::<Vec<_>>();
        let mut outcomes = Vec::with_capacity(binding_ids.len());
        for binding_id in binding_ids {
            let result = self
                .dispatch_artifact_binding(
                    tenant_id,
                    module_slug,
                    &binding_id,
                    serde_json::json!({
                        "binding_id": binding_id.clone(),
                        "event_type": event_type,
                        "payload": payload.clone(),
                    }),
                    ExecutionPhase::Event,
                    admission.clone(),
                    context.clone(),
                )
                .await;
            outcomes.push(ArtifactEventBindingOutcome { binding_id, result });
        }
        Ok(outcomes)
    }

    /// Dispatches a platform-routed JSON HTTP request after the host has
    /// resolved actor authorization. The descriptor supplies only a relative
    /// route and bounded JSON envelope; it never supplies an Axum router or a
    /// transport listener.
    pub async fn dispatch_artifact_http(
        &self,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        method: ModuleHttpMethod,
        path: &str,
        body: serde_json::Value,
        admission: ArtifactExecutionAdmission,
        context: ArtifactBindingExecutionContext,
    ) -> Result<serde_json::Value, ModuleDispatchError> {
        if !context.is_valid() {
            return Err(ModuleDispatchError::InvalidArtifactExecutionContext);
        }
        let definition = self
            .catalog
            .get(module_slug)
            .ok_or_else(|| ModuleDispatchError::UnknownDefinition(module_slug.to_string()))?;
        let ModuleDefinitionSource::Artifact { release } = &definition.source else {
            return Err(ModuleDispatchError::StaticDynamicBindingUnavailable(
                module_slug.to_string(),
            ));
        };
        dispatch_artifact_http_binding(
            self.artifact_executor.ok_or_else(|| {
                ModuleDispatchError::ArtifactExecutorUnavailable(module_slug.to_string())
            })?,
            ArtifactHttpBindingRequest {
                release,
                bindings: &definition.bindings,
                target: ArtifactInstallationTarget::CurrentRelease,
                tenant_id,
                admission,
                method,
                path,
                body,
                context,
            },
        )
        .await
    }
}

/// Finds the sole admitted HTTP binding for a literal method/path pair.
pub fn find_artifact_http_binding<'a>(
    bindings: &'a [ModuleRuntimeBinding],
    method: ModuleHttpMethod,
    path: &str,
) -> Option<&'a ModuleRuntimeBinding> {
    bindings.iter().find(|binding| {
        binding.kind == ModuleRuntimeBindingKind::Http
            && binding
                .http
                .as_ref()
                .is_some_and(|http| http.method == method && http.path == path)
    })
}

/// Finds the sole admitted command binding for an exact platform binding ID.
pub fn find_artifact_command_binding<'a>(
    bindings: &'a [ModuleRuntimeBinding],
    binding_id: &str,
) -> Option<&'a ModuleRuntimeBinding> {
    bindings.iter().find(|binding| {
        binding.kind == ModuleRuntimeBindingKind::Command && binding.id == binding_id
    })
}

/// Resolves a host-rendered action or form contribution to its one admitted
/// command binding. This is intentionally narrower than a generic command
/// lookup: a UI transport cannot invoke a binding merely by presenting an
/// arbitrary binding ID.
pub fn find_artifact_ui_action_binding<'a>(
    contributions: &[ArtifactUiContribution],
    bindings: &'a [ModuleRuntimeBinding],
    contribution_id: &str,
) -> Option<&'a ModuleRuntimeBinding> {
    let contribution = contributions
        .iter()
        .find(|contribution| contribution.id == contribution_id)?;
    let (binding_id, confirmation, destructive, audit_policy, expected_surface) =
        match &contribution.content {
            ArtifactUiContributionContent::Action {
                binding_id,
                confirmation,
                destructive,
                audit_policy,
                ..
            } => (
                binding_id.as_str(),
                confirmation,
                destructive,
                audit_policy,
                ArtifactUiSurface::AdminActions,
            ),
            ArtifactUiContributionContent::Form {
                binding_id,
                confirmation,
                destructive,
                audit_policy,
                ..
            } => (
                binding_id.as_str(),
                confirmation,
                destructive,
                audit_policy,
                ArtifactUiSurface::AdminForm,
            ),
            _ => return None,
        };
    if contribution.surface != expected_surface
        || *audit_policy != ArtifactUiAuditPolicy::Required
        || (*destructive != (*confirmation == ArtifactUiActionConfirmation::Destructive))
    {
        return None;
    }
    let binding = find_artifact_command_binding(bindings, binding_id)?;
    (binding.permission == contribution.permission
        && binding.idempotency == ModuleBindingIdempotency::Required)
        .then_some(binding)
}

/// Dispatches one admitted command binding through an explicit immutable
/// installation target. The host selects the binding and actor context; an
/// artifact cannot create a command route or select another installation.
pub async fn dispatch_artifact_command_binding<E>(
    executor: &E,
    request: ArtifactCommandBindingRequest<'_>,
) -> Result<serde_json::Value, ModuleDispatchError>
where
    E: ArtifactBindingExecutor + ?Sized,
{
    let ArtifactCommandBindingRequest {
        release,
        bindings,
        target,
        tenant_id,
        admission,
        binding_id,
        input,
        context,
    } = request;
    if !context.is_valid() {
        return Err(ModuleDispatchError::InvalidArtifactExecutionContext);
    }
    let binding = find_artifact_command_binding(bindings, binding_id).ok_or_else(|| {
        ModuleDispatchError::ArtifactCommandUnavailable {
            module_slug: release.slug.clone(),
            binding_id: binding_id.to_string(),
        }
    })?;
    if !admission.matches_dispatch(release, binding, target, tenant_id, ExecutionPhase::Manual) {
        return Err(ModuleDispatchError::AdmissionMismatch);
    }
    executor
        .dispatch_binding(ArtifactBindingDispatch {
            release,
            binding,
            target,
            tenant_id,
            admission,
            input,
            phase: ExecutionPhase::Manual,
            context,
        })
        .await
        .map_err(ModuleDispatchError::ArtifactHook)
}

/// Dispatches one literal admitted HTTP binding through an explicit immutable
/// installation target. Both the generic dispatcher and platform HTTP hosts
/// use this owner helper so JSON limits and envelope shape stay identical.
pub async fn dispatch_artifact_http_binding<E>(
    executor: &E,
    request: ArtifactHttpBindingRequest<'_>,
) -> Result<serde_json::Value, ModuleDispatchError>
where
    E: ArtifactBindingExecutor + ?Sized,
{
    let ArtifactHttpBindingRequest {
        release,
        bindings,
        target,
        tenant_id,
        admission,
        method,
        path,
        body,
        context,
    } = request;
    if !context.is_valid() {
        return Err(ModuleDispatchError::InvalidArtifactExecutionContext);
    }
    let binding = find_artifact_http_binding(bindings, method, path).ok_or_else(|| {
        ModuleDispatchError::ArtifactHttpRouteUnavailable {
            module_slug: release.slug.clone(),
            path: path.to_string(),
        }
    })?;
    if !admission.matches_dispatch(release, binding, target, tenant_id, ExecutionPhase::Http) {
        return Err(ModuleDispatchError::AdmissionMismatch);
    }
    let http = binding
        .http
        .as_ref()
        .expect("HTTP binding was selected only when it has an HTTP contract");
    let body_bytes = serde_json::to_vec(&body)
        .map_err(|error| ModuleDispatchError::ArtifactHttpEnvelope(error.to_string()))?;
    if body_bytes.len() as u64 > http.max_body_bytes {
        return Err(ModuleDispatchError::ArtifactHttpRequestTooLarge {
            limit: http.max_body_bytes,
        });
    }
    let binding_id = binding.id.clone();
    let output = executor
        .dispatch_binding(ArtifactBindingDispatch {
            release,
            binding,
            target,
            tenant_id,
            admission,
            input: serde_json::json!({
                "binding_id": binding_id,
                "method": method,
                "path": path,
                "body": body,
            }),
            phase: ExecutionPhase::Http,
            context,
        })
        .await
        .map_err(ModuleDispatchError::ArtifactHook)?;
    let output_bytes = serde_json::to_vec(&output)
        .map_err(|error| ModuleDispatchError::ArtifactHttpEnvelope(error.to_string()))?;
    if output_bytes.len() as u64 > http.max_output_bytes {
        return Err(ModuleDispatchError::ArtifactHttpResponseTooLarge {
            limit: http.max_output_bytes,
        });
    }
    Ok(output)
}

/// Failure of an admitted artifact binding dispatch through
/// [`ArtifactBindingExecutor`]. Resolution denials are preserved as typed
/// variants so durable work queues can distinguish terminal causes (the
/// installation or policy state can never satisfy the work item) from
/// transient infrastructure failures via [`Self::is_terminal`].
#[derive(Debug, Error)]
pub enum ArtifactBindingDispatchError {
    #[error("artifact binding execution context is invalid")]
    InvalidContext,
    #[error("HTTP dispatch requires an admitted HTTP binding")]
    HttpBindingRequired,
    #[error("module `{slug}` is denied by effective policy revision {policy_revision}")]
    DeniedByPolicy {
        slug: String,
        policy_revision: String,
    },
    #[error("artifact execution admission does not match the dispatch or resolved installation")]
    AdmissionMismatch,
    #[error(transparent)]
    Installation(#[from] crate::ArtifactInstallationResolveError),
    #[error(transparent)]
    SandboxPolicy(#[from] crate::ArtifactSandboxPolicyResolveError),
    #[error(transparent)]
    EffectivePolicy(#[from] crate::ArtifactEffectivePolicyResolveError),
    #[error(transparent)]
    Runtime(#[from] crate::ArtifactRuntimeError),
    #[error("{0}")]
    Execution(String),
}

impl ArtifactBindingDispatchError {
    /// Whether retrying the same durable work item can never succeed without
    /// an operator or state change. Store failures and node-readiness gaps
    /// are transient; sandbox execution failures stay retryable because
    /// executor availability is transient evidence (see the M-5 policy
    /// rationale). A policy denial is deliberately *not* terminal: policy
    /// toggles are routine state, so durable queues park the delivery (see
    /// [`Self::is_policy_denied`]) instead of dead-lettering work that
    /// becomes valid again when the module is re-enabled.
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::InvalidContext | Self::HttpBindingRequired | Self::AdmissionMismatch => true,
            Self::DeniedByPolicy { .. } => false,
            Self::Installation(error) => {
                !matches!(error, crate::ArtifactInstallationResolveError::Store(_))
            }
            Self::SandboxPolicy(error) => {
                !matches!(error, crate::ArtifactSandboxPolicyResolveError::Store(_))
            }
            Self::EffectivePolicy(error) => matches!(
                error,
                crate::ArtifactEffectivePolicyResolveError::InvalidRequest(_)
            ),
            Self::Runtime(error) => error.is_terminal(),
            Self::Execution(_) => false,
        }
    }

    /// Terminality for one admitted binding's declared idempotency. A
    /// binding that declares `ModuleBindingIdempotency::None` must never
    /// be auto-retried after an ambiguous outcome: sandbox/execution
    /// failures can mean the guest already performed side effects, and a
    /// retry would silently execute it twice. Failures raised before
    /// guest execution (resolution, policy, envelope, input-schema)
    /// remain governed by [`Self::is_terminal`].
    pub fn is_terminal_for(&self, idempotency: crate::ModuleBindingIdempotency) -> bool {
        self.is_terminal()
            || (idempotency == crate::ModuleBindingIdempotency::None && self.may_have_executed())
    }

    /// Whether the failure is an effective-policy denial raised before any
    /// guest execution. Durable delivery queues use this to park (hold)
    /// the delivery without consuming its failure-attempt budget: nothing
    /// executed, and the work becomes deliverable again on re-enable.
    pub fn is_policy_denied(&self) -> bool {
        matches!(self, Self::DeniedByPolicy { .. })
    }

    fn may_have_executed(&self) -> bool {
        match self {
            Self::Runtime(error) => {
                matches!(error, crate::ArtifactRuntimeError::Sandbox(_))
            }
            Self::Execution(_) => true,
            _ => false,
        }
    }

    /// Stable, redacted failure code for durable delivery records (see
    /// `ArtifactRuntimeError::code`).
    pub fn code(&self) -> &'static str {
        use crate::{
            ArtifactEffectivePolicyResolveError, ArtifactInstallationResolveError,
            ArtifactSandboxPolicyResolveError,
        };
        match self {
            Self::InvalidContext => "execution_context_invalid",
            Self::HttpBindingRequired => "http_binding_required",
            Self::DeniedByPolicy { .. } => DELIVERY_CODE_DENIED_BY_POLICY,
            Self::AdmissionMismatch => "execution_admission_mismatch",
            Self::Installation(error) => match error {
                ArtifactInstallationResolveError::Unavailable => "installation_unavailable",
                ArtifactInstallationResolveError::NotActiveSelection => {
                    "installation_not_active_selection"
                }
                ArtifactInstallationResolveError::InvalidRequest(_) => {
                    "installation_request_invalid"
                }
                ArtifactInstallationResolveError::Invariant(_) => "installation_state_invalid",
                ArtifactInstallationResolveError::Store(_) => "installation_store_failed",
            },
            Self::SandboxPolicy(error) => match error {
                ArtifactSandboxPolicyResolveError::Unavailable => "sandbox_policy_unavailable",
                ArtifactSandboxPolicyResolveError::RevisionMismatch => {
                    "sandbox_policy_revision_mismatch"
                }
                ArtifactSandboxPolicyResolveError::InvalidRequest(_) => {
                    "sandbox_policy_request_invalid"
                }
                ArtifactSandboxPolicyResolveError::Invariant(_) => "sandbox_policy_state_invalid",
                ArtifactSandboxPolicyResolveError::Store(_) => "sandbox_policy_store_failed",
            },
            Self::EffectivePolicy(error) => match error {
                ArtifactEffectivePolicyResolveError::InvalidRequest(_) => {
                    "effective_policy_request_invalid"
                }
                ArtifactEffectivePolicyResolveError::NodeNotReady(_) => "node_not_ready",
                ArtifactEffectivePolicyResolveError::Store(_) => "effective_policy_store_failed",
            },
            Self::Runtime(error) => error.code(),
            Self::Execution(_) => "execution_failed",
        }
    }
}

/// Accepts a work-outcome message as a durable delivery error code only
/// when it already looks like one of the stable snake_case codes; any
/// other message (foreign executor, free-form text) collapses to the
/// caller's generic fallback so unbounded error text is never persisted
/// as a code.
/// Stable delivery error code for an effective-policy denial (see
/// [`ArtifactBindingDispatchError::code`]). Delivery queues compare the
/// sanitized code against this constant to park the delivery instead of
/// spending its failure-attempt budget.
pub(crate) const DELIVERY_CODE_DENIED_BY_POLICY: &str = "denied_by_policy";

pub(crate) fn delivery_error_code(message: &str, fallback: &'static str) -> String {
    let looks_like_code = !message.is_empty()
        && message.len() <= 64
        && message
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    if looks_like_code {
        message.to_string()
    } else {
        fallback.to_string()
    }
}

/// Narrow adapter owned by the artifact runtime integration. It must resolve an
/// admitted installation and execute it through `SandboxRuntime`; a static
/// callback cannot implement this port.
#[async_trait]
pub trait ArtifactBindingExecutor: Send + Sync {
    /// Reports whether the composed sandbox has the executor required by this
    /// admitted payload kind. Policy uses this as readiness evidence and must
    /// not infer availability from the presence of this port alone.
    fn supports_payload_kind(&self, payload_kind: crate::ArtifactPayloadKind) -> bool;

    async fn dispatch_binding(
        &self,
        dispatch: ArtifactBindingDispatch<'_>,
    ) -> Result<serde_json::Value, ArtifactBindingDispatchError>;
}

/// Lifecycle remains a convenience surface over the generic admitted binding
/// port. It is not a second artifact execution path.
#[async_trait]
pub trait ArtifactLifecycleExecutor: ArtifactBindingExecutor {
    async fn dispatch_lifecycle(
        &self,
        release: &ArtifactReleaseRef,
        binding: &ModuleRuntimeBinding,
        tenant_id: uuid::Uuid,
        config: &serde_json::Value,
        phase: ModuleLifecycleHookPhase,
    ) -> Result<(), ArtifactBindingDispatchError> {
        self.dispatch_binding(ArtifactBindingDispatch {
            release,
            binding,
            target: ArtifactInstallationTarget::CurrentRelease,
            tenant_id,
            admission: ArtifactExecutionAdmission::lifecycle(tenant_id, release),
            input: serde_json::json!({
                "binding_id": binding.id,
                "phase": phase,
                "config": config,
            }),
            phase: ExecutionPhase::Lifecycle,
            context: ArtifactBindingExecutionContext::default(),
        })
        .await
        .map(|_| ())
    }
}

impl<T> ArtifactLifecycleExecutor for T where T: ArtifactBindingExecutor + ?Sized {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactExecutionAdmissionKind {
    /// Minted by `ModuleEffectivePolicy` for one enabled decision.
    EffectivePolicy,
    /// Minted only by the lifecycle port; the lifecycle operation itself
    /// (enable/disable under the operation journal) is the authorization,
    /// so no effective-policy decision is required or consulted.
    Lifecycle,
    /// Minted only by crate-owned durable delivery workers. It proves the
    /// dispatch came from the owner queue, not that policy still admits
    /// it: the production executor re-resolves effective policy and parks
    /// denied work without executing.
    OwnerRevalidation,
    /// Minted only by the owner data-upgrade bridge for its dedicated
    /// binding kind, which the generic dispatcher never exposes.
    DataUpgrade,
}

/// Opaque proof that an artifact binding dispatch was admitted by the
/// policy/lifecycle owner for one exact tenant, release, and installation
/// target.
///
/// Hosts can obtain a token only from
/// `ModuleEffectivePolicy::admit_artifact_execution`; the fields and all
/// other constructors are crate-private, so a transport cannot fabricate
/// admission from descriptor data, headers, or an enabled-slug set. The
/// production executor additionally re-resolves current effective policy
/// for every non-lifecycle dispatch, so a token minted under a policy
/// that has since been revoked still cannot execute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactExecutionAdmission {
    kind: ArtifactExecutionAdmissionKind,
    tenant_id: uuid::Uuid,
    module_slug: String,
    release_digest: String,
    installation_id: Option<uuid::Uuid>,
    policy_revision: Option<String>,
}

impl ArtifactExecutionAdmission {
    fn new(
        kind: ArtifactExecutionAdmissionKind,
        tenant_id: uuid::Uuid,
        release: &ArtifactReleaseRef,
        target: ArtifactInstallationTarget,
        policy_revision: Option<String>,
    ) -> Self {
        Self {
            kind,
            tenant_id,
            module_slug: release.slug.clone(),
            release_digest: release.digest.clone(),
            installation_id: match target {
                ArtifactInstallationTarget::CurrentRelease => None,
                ArtifactInstallationTarget::ExactInstallation { installation_id } => {
                    Some(installation_id)
                }
            },
            policy_revision,
        }
    }

    pub(crate) fn effective_policy(
        tenant_id: uuid::Uuid,
        release: &ArtifactReleaseRef,
        target: ArtifactInstallationTarget,
        policy_revision: String,
    ) -> Self {
        Self::new(
            ArtifactExecutionAdmissionKind::EffectivePolicy,
            tenant_id,
            release,
            target,
            Some(policy_revision),
        )
    }

    pub(crate) fn lifecycle(tenant_id: uuid::Uuid, release: &ArtifactReleaseRef) -> Self {
        Self::new(
            ArtifactExecutionAdmissionKind::Lifecycle,
            tenant_id,
            release,
            ArtifactInstallationTarget::CurrentRelease,
            None,
        )
    }

    pub(crate) fn owner_revalidation(
        tenant_id: uuid::Uuid,
        release: &ArtifactReleaseRef,
        target: ArtifactInstallationTarget,
    ) -> Self {
        Self::new(
            ArtifactExecutionAdmissionKind::OwnerRevalidation,
            tenant_id,
            release,
            target,
            None,
        )
    }

    pub(crate) fn data_upgrade(tenant_id: uuid::Uuid, release: &ArtifactReleaseRef) -> Self {
        Self::new(
            ArtifactExecutionAdmissionKind::DataUpgrade,
            tenant_id,
            release,
            ArtifactInstallationTarget::CurrentRelease,
            None,
        )
    }

    /// Effective-policy revision under which this token was minted, when
    /// it is a policy token. Lifecycle and owner-revalidation tokens do
    /// not carry a revision.
    pub fn policy_revision(&self) -> Option<&str> {
        self.policy_revision.as_deref()
    }

    /// Whether this token was minted for exactly this not-yet-resolved
    /// dispatch shape. Checked by the dispatcher helpers before any
    /// executor is invoked.
    pub(crate) fn matches_dispatch(
        &self,
        release: &ArtifactReleaseRef,
        binding: &ModuleRuntimeBinding,
        target: ArtifactInstallationTarget,
        tenant_id: uuid::Uuid,
        phase: ExecutionPhase,
    ) -> bool {
        if self.tenant_id != tenant_id
            || self.module_slug != release.slug
            || self.release_digest != release.digest
        {
            return false;
        }
        let target_matches = match target {
            ArtifactInstallationTarget::CurrentRelease => self.installation_id.is_none(),
            ArtifactInstallationTarget::ExactInstallation { installation_id } => {
                self.installation_id == Some(installation_id)
            }
        };
        if !target_matches {
            return false;
        }
        match self.kind {
            ArtifactExecutionAdmissionKind::EffectivePolicy
            | ArtifactExecutionAdmissionKind::OwnerRevalidation => {
                phase != ExecutionPhase::Lifecycle
                    && binding.kind != ModuleRuntimeBindingKind::DataUpgrade
                    && binding_allows_phase(binding.kind.clone(), phase)
            }
            ArtifactExecutionAdmissionKind::Lifecycle => {
                phase == ExecutionPhase::Lifecycle
                    && binding_allows_phase(binding.kind.clone(), phase)
            }
            ArtifactExecutionAdmissionKind::DataUpgrade => {
                phase == ExecutionPhase::Manual
                    && binding.kind == ModuleRuntimeBindingKind::DataUpgrade
            }
        }
    }

    /// Full check against the resolved immutable installation, performed
    /// by the production executor immediately before sandbox policy
    /// resolution. A token minted for one tenant/release/installation
    /// can never authorize a different resolved artifact.
    pub(crate) fn authorize(
        &self,
        release: &ArtifactReleaseRef,
        binding: &ModuleRuntimeBinding,
        target: ArtifactInstallationTarget,
        tenant_id: uuid::Uuid,
        phase: ExecutionPhase,
        artifact: &InstalledModuleArtifact,
    ) -> Result<(), ArtifactBindingDispatchError> {
        let scope_matches = match artifact.scope {
            ModuleInstallationScope::Platform => true,
            ModuleInstallationScope::Tenant {
                tenant_id: scoped_tenant_id,
            } => scoped_tenant_id == tenant_id,
        };
        if !self.matches_dispatch(release, binding, target, tenant_id, phase)
            || !scope_matches
            || artifact.release.slug != self.module_slug
            || artifact.descriptor.artifact_digest != self.release_digest
            || self
                .installation_id
                .is_some_and(|installation_id| installation_id != artifact.installation_id)
        {
            return Err(ArtifactBindingDispatchError::AdmissionMismatch);
        }
        Ok(())
    }
}

pub struct ArtifactBindingDispatch<'a> {
    pub release: &'a ArtifactReleaseRef,
    pub binding: &'a ModuleRuntimeBinding,
    /// Current interactive dispatch resolves the effective release. Durable
    /// delivery workers must pin one immutable installation identity so a later
    /// composition change cannot execute a different artifact.
    pub target: ArtifactInstallationTarget,
    pub tenant_id: uuid::Uuid,
    /// Owner-minted execution admission for this dispatch. See
    /// [`ArtifactExecutionAdmission`]; executors must not run a binding
    /// whose admission does not authorize the resolved installation.
    pub admission: ArtifactExecutionAdmission,
    pub input: serde_json::Value,
    pub phase: ExecutionPhase,
    /// Authenticated transport identity supplied by the host, never by the
    /// descriptor or artifact payload.
    pub context: ArtifactBindingExecutionContext,
}

/// Selects whether a binding may use the current effective artifact release or
/// must execute one exact immutable installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtifactInstallationTarget {
    CurrentRelease,
    ExactInstallation { installation_id: uuid::Uuid },
}

fn lifecycle_binding_kind(phase: ModuleLifecycleHookPhase) -> ModuleRuntimeBindingKind {
    match phase {
        ModuleLifecycleHookPhase::PreEnable => ModuleRuntimeBindingKind::PreEnable,
        ModuleLifecycleHookPhase::PostEnable => ModuleRuntimeBindingKind::PostEnable,
        ModuleLifecycleHookPhase::PreDisable => ModuleRuntimeBindingKind::PreDisable,
        ModuleLifecycleHookPhase::PostDisable => ModuleRuntimeBindingKind::PostDisable,
    }
}

fn binding_allows_phase(kind: ModuleRuntimeBindingKind, phase: ExecutionPhase) -> bool {
    // DataUpgrade is intentionally absent: the owner-only data upgrade bridge
    // invokes its admitted binding directly after its storage read is complete.
    matches!(
        (kind, phase),
        (
            ModuleRuntimeBindingKind::PreEnable
                | ModuleRuntimeBindingKind::PostEnable
                | ModuleRuntimeBindingKind::PreDisable
                | ModuleRuntimeBindingKind::PostDisable
                | ModuleRuntimeBindingKind::Health
                | ModuleRuntimeBindingKind::Readiness
                | ModuleRuntimeBindingKind::ActivationSmoke,
            ExecutionPhase::Lifecycle
        ) | (ModuleRuntimeBindingKind::Command, ExecutionPhase::Manual)
            | (ModuleRuntimeBindingKind::Http, ExecutionPhase::Http)
            | (ModuleRuntimeBindingKind::Event, ExecutionPhase::Event)
            | (
                ModuleRuntimeBindingKind::Schedule,
                ExecutionPhase::Scheduled
            )
            | (
                ModuleRuntimeBindingKind::BeforeCommit,
                ExecutionPhase::BeforeHook
            )
            | (
                ModuleRuntimeBindingKind::AfterCommit | ModuleRuntimeBindingKind::OnCommit,
                ExecutionPhase::AfterHook
            )
    )
}

fn valid_delivered_event_type(value: &str) -> bool {
    valid_event_topic(value) && !value.ends_with(".*")
}

#[derive(Debug, Error)]
pub enum ModuleDispatchError {
    #[error("module definition `{0}` is not active")]
    UnknownDefinition(String),
    #[error("static module definition `{0}` has no compiled implementation")]
    MissingStaticImplementation(String),
    #[error("artifact module `{0}` has no admitted lifecycle binding")]
    ArtifactBindingUnavailable(String),
    #[error("artifact module `{0}` has no sandbox lifecycle executor")]
    ArtifactExecutorUnavailable(String),
    #[error("artifact lifecycle binding failed: {0}")]
    ArtifactHook(#[from] ArtifactBindingDispatchError),
    #[error("static module `{0}` has no dynamic artifact binding path")]
    StaticDynamicBindingUnavailable(String),
    #[error("artifact binding phase does not match its declared kind")]
    BindingPhaseMismatch {
        binding_id: String,
        phase: ExecutionPhase,
    },
    #[error("artifact event type `{0}` is not a valid exact platform event type")]
    InvalidArtifactEventType(String),
    #[error("artifact module `{module_slug}` has no admitted HTTP route for `{path}")]
    ArtifactHttpRouteUnavailable { module_slug: String, path: String },
    #[error("artifact module `{module_slug}` has no admitted command binding `{binding_id}")]
    ArtifactCommandUnavailable {
        module_slug: String,
        binding_id: String,
    },
    #[error("artifact HTTP request exceeds the declared {limit}-byte body limit")]
    ArtifactHttpRequestTooLarge { limit: u64 },
    #[error("artifact HTTP response exceeds the declared {limit}-byte output limit")]
    ArtifactHttpResponseTooLarge { limit: u64 },
    #[error("artifact HTTP JSON envelope could not be encoded: {0}")]
    ArtifactHttpEnvelope(String),
    #[error("artifact binding execution context is invalid")]
    InvalidArtifactExecutionContext,
    #[error("artifact execution admission does not match the requested dispatch")]
    AdmissionMismatch,
    #[error(
        "artifact module `{module_slug}` declares {matches} bindings for kind `{kind}`; exactly one is admissible"
    )]
    AmbiguousArtifactBinding {
        module_slug: String,
        kind: String,
        matches: usize,
    },
    #[error("module lifecycle binding failed: {0}")]
    StaticHook(String),
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use rustok_api::{ArtifactUiActionConfirmation, ArtifactUiSurface};
    use sea_orm::Database;

    use super::*;
    use crate::{
        ArtifactReleaseRef, ArtifactUiAuditPolicy, ArtifactUiContribution,
        ArtifactUiContributionContent, ModuleBindingIdempotency, ModuleDefinition,
        ModuleDefinitionKind, ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    };

    #[test]
    fn delivery_error_code_accepts_only_stable_codes() {
        assert_eq!(
            delivery_error_code("denied_by_policy", "execution_failed"),
            "denied_by_policy"
        );
        assert_eq!(
            delivery_error_code("guest input with spaces", "execution_failed"),
            "execution_failed"
        );
        assert_eq!(
            delivery_error_code("", "execution_rejected"),
            "execution_rejected"
        );
        assert_eq!(
            delivery_error_code(&"x".repeat(65), "execution_failed"),
            "execution_failed"
        );

        let terminal = ArtifactBindingDispatchError::Runtime(
            crate::ArtifactRuntimeError::BindingSchemaViolation {
                binding: "apply".to_string(),
                stage: "input",
                schema_digest: format!("sha256:{}", "a".repeat(64)),
            },
        );
        assert!(terminal.is_terminal());
        assert_eq!(terminal.code(), "binding_schema_violation");

        // An ambiguous execution failure is retryable only when the
        // binding declared itself retry-safe.
        let ambiguous = ArtifactBindingDispatchError::Execution("timeout".to_string());
        assert!(!ambiguous.is_terminal());
        assert!(ambiguous.is_terminal_for(ModuleBindingIdempotency::None));
        assert!(!ambiguous.is_terminal_for(ModuleBindingIdempotency::Required));
        assert!(!ambiguous.is_terminal_for(ModuleBindingIdempotency::BestEffort));

        // Pre-execution failures stay retryable even for `None` bindings:
        // the guest provably never ran.
        let pre_execution = ArtifactBindingDispatchError::Installation(
            crate::ArtifactInstallationResolveError::Store("db down".to_string()),
        );
        assert!(!pre_execution.is_terminal_for(ModuleBindingIdempotency::None));

        // A policy denial parks durable work: it is not terminal for any
        // binding (nothing executed) and is identifiable by its stable
        // code so queues can hold without spending failure attempts.
        let denied = ArtifactBindingDispatchError::DeniedByPolicy {
            slug: "blog".to_string(),
            policy_revision: "7".to_string(),
        };
        assert!(denied.is_policy_denied());
        assert!(!denied.is_terminal());
        assert!(!denied.is_terminal_for(ModuleBindingIdempotency::None));
        assert_eq!(denied.code(), DELIVERY_CODE_DENIED_BY_POLICY);
    }

    struct RecordingArtifactExecutor(Mutex<Vec<(String, ModuleLifecycleHookPhase)>>);

    struct EchoArtifactExecutor;

    #[async_trait]
    impl ArtifactBindingExecutor for RecordingArtifactExecutor {
        fn supports_payload_kind(&self, _payload_kind: crate::ArtifactPayloadKind) -> bool {
            true
        }

        async fn dispatch_binding(
            &self,
            dispatch: ArtifactBindingDispatch<'_>,
        ) -> Result<serde_json::Value, ArtifactBindingDispatchError> {
            let phase: ModuleLifecycleHookPhase = dispatch
                .input
                .get("phase")
                .cloned()
                .ok_or_else(|| {
                    ArtifactBindingDispatchError::Execution(
                        "lifecycle phase is missing".to_string(),
                    )
                })
                .and_then(|value| {
                    serde_json::from_value(value)
                        .map_err(|error| ArtifactBindingDispatchError::Execution(error.to_string()))
                })?;
            self.0
                .lock()
                .expect("executor lock")
                .push((dispatch.release.slug.clone(), phase));
            Ok(serde_json::Value::Null)
        }
    }

    #[async_trait]
    impl ArtifactBindingExecutor for EchoArtifactExecutor {
        fn supports_payload_kind(&self, _payload_kind: crate::ArtifactPayloadKind) -> bool {
            true
        }

        async fn dispatch_binding(
            &self,
            dispatch: ArtifactBindingDispatch<'_>,
        ) -> Result<serde_json::Value, ArtifactBindingDispatchError> {
            Ok(dispatch.input)
        }
    }

    fn command_binding() -> ModuleRuntimeBinding {
        ModuleRuntimeBinding {
            id: "apply_settings".to_string(),
            kind: ModuleRuntimeBindingKind::Command,
            entrypoint: "settings.apply".to_string(),
            input_schema_digest: format!("sha256:{}", "a".repeat(64)),
            output_schema_digest: format!("sha256:{}", "b".repeat(64)),
            permission: "artifact_module.settings.manage".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "command".to_string(),
            capabilities: Vec::new(),
            event_topics: Vec::new(),
            schedule: None,
            http: None,
        }
    }

    #[test]
    fn ui_action_binding_requires_the_exact_admitted_action_or_form_contract() {
        let binding = command_binding();
        let action = ArtifactUiContribution {
            id: "apply".to_string(),
            surface: ArtifactUiSurface::AdminActions,
            localization_digest: format!("sha256:{}", "c".repeat(64)),
            permission: binding.permission.clone(),
            content: ArtifactUiContributionContent::Action {
                title_key: "actions.apply".to_string(),
                binding_id: binding.id.clone(),
                confirmation: ArtifactUiActionConfirmation::Destructive,
                destructive: true,
                audit_policy: ArtifactUiAuditPolicy::Required,
            },
        };
        let form = ArtifactUiContribution {
            id: "settings_form".to_string(),
            surface: ArtifactUiSurface::AdminForm,
            localization_digest: format!("sha256:{}", "c".repeat(64)),
            permission: binding.permission.clone(),
            content: ArtifactUiContributionContent::Form {
                title_key: "settings.title".to_string(),
                binding_id: binding.id.clone(),
                confirmation: ArtifactUiActionConfirmation::None,
                destructive: false,
                audit_policy: ArtifactUiAuditPolicy::Required,
            },
        };
        assert_eq!(
            find_artifact_ui_action_binding(
                &[action.clone(), form.clone()],
                std::slice::from_ref(&binding),
                "apply",
            )
            .map(|binding| binding.id.as_str()),
            Some("apply_settings")
        );
        assert_eq!(
            find_artifact_ui_action_binding(
                &[action.clone(), form],
                std::slice::from_ref(&binding),
                "settings_form",
            )
            .map(|binding| binding.id.as_str()),
            Some("apply_settings")
        );

        let mut mismatched_permission = action.clone();
        mismatched_permission.permission = "artifact_module.settings.read".to_string();
        assert!(
            find_artifact_ui_action_binding(
                &[mismatched_permission],
                std::slice::from_ref(&binding),
                "apply",
            )
            .is_none()
        );

        let mut non_idempotent_binding = binding.clone();
        non_idempotent_binding.idempotency = ModuleBindingIdempotency::BestEffort;
        assert!(
            find_artifact_ui_action_binding(
                std::slice::from_ref(&action),
                &[non_idempotent_binding],
                "apply",
            )
            .is_none()
        );

        let mut non_action = action;
        non_action.surface = ArtifactUiSurface::AdminStatus;
        assert!(find_artifact_ui_action_binding(
            &[non_action],
            std::slice::from_ref(&binding),
            "apply",
        )
        .is_none());
    }

    #[test]
    fn event_subscriptions_and_binding_phases_are_narrowly_matched() {
        assert!(event_topic_matches("order.*", "order.completed"));
        assert!(!event_topic_matches("order.*", "orders.completed"));
        assert!(!event_topic_matches("*", "order.completed"));
        assert!(valid_delivered_event_type("order.completed"));
        assert!(!valid_delivered_event_type("order.*"));
        assert!(!valid_delivered_event_type("order..completed"));
        assert!(binding_allows_phase(
            ModuleRuntimeBindingKind::Event,
            ExecutionPhase::Event
        ));
        assert!(!binding_allows_phase(
            ModuleRuntimeBindingKind::Event,
            ExecutionPhase::Scheduled
        ));
        assert!(!binding_allows_phase(
            ModuleRuntimeBindingKind::DataUpgrade,
            ExecutionPhase::Manual
        ));
    }

    #[tokio::test]
    async fn artifact_http_dispatch_matches_only_the_admitted_bounded_route() {
        let release = ArtifactReleaseRef {
            slug: "artifact_module".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };
        let mut catalog = ModuleDefinitionCatalog::default();
        catalog
            .insert(ModuleDefinition {
                slug: release.slug.clone(),
                version: release.version.clone(),
                kind: ModuleDefinitionKind::Optional,
                source: ModuleDefinitionSource::Artifact {
                    release: release.clone(),
                },
                dependencies: Vec::new(),
                permissions: Vec::new(),
                settings_schema_digest: None,
                schema_documents: Vec::new(),
                bindings: vec![ModuleRuntimeBinding {
                    id: "http_status".to_string(),
                    kind: ModuleRuntimeBindingKind::Http,
                    entrypoint: "http.status".to_string(),
                    input_schema_digest: format!("sha256:{}", "b".repeat(64)),
                    output_schema_digest: format!("sha256:{}", "c".repeat(64)),
                    permission: "artifact_module.http.status.read".to_string(),
                    idempotency: ModuleBindingIdempotency::Required,
                    limit_profile: "http_json".to_string(),
                    capabilities: Vec::new(),
                    event_topics: Vec::new(),
                    schedule: None,
                    http: Some(crate::ModuleHttpBinding {
                        method: ModuleHttpMethod::Post,
                        path: "status/query".to_string(),
                        request_media_type: "application/json".to_string(),
                        response_media_type: "application/json".to_string(),
                        max_body_bytes: 64,
                        max_output_bytes: 1_024,
                        timeout_ms: 5_000,
                        streaming: crate::ModuleHttpStreamingPolicy::Forbidden,
                    }),
                }],
                ui: Vec::new(),
                capabilities: Vec::new(),
            })
            .expect("artifact definition");
        let executor = EchoArtifactExecutor;
        let dispatcher = ModuleExecutionDispatcher::artifact_only(&catalog, &executor);
        let tenant_id = uuid::Uuid::new_v4();
        let admission = ArtifactExecutionAdmission::owner_revalidation(
            tenant_id,
            &release,
            ArtifactInstallationTarget::CurrentRelease,
        );
        let body = serde_json::json!({ "include": "summary" });

        let output = dispatcher
            .dispatch_artifact_http(
                tenant_id,
                "artifact_module",
                ModuleHttpMethod::Post,
                "status/query",
                body.clone(),
                admission.clone(),
                ArtifactBindingExecutionContext {
                    actor_id: Some("user-42".to_string()),
                    trace_id: Some("trace-42".to_string()),
                },
            )
            .await
            .expect("admitted HTTP route");
        assert_eq!(output["binding_id"], "http_status");
        assert_eq!(output["body"], body);

        assert!(matches!(
            dispatcher
                .dispatch_artifact_http(
                    tenant_id,
                    "artifact_module",
                    ModuleHttpMethod::Get,
                    "status/query",
                    serde_json::json!({}),
                    admission,
                    ArtifactBindingExecutionContext::default(),
                )
                .await,
            Err(ModuleDispatchError::ArtifactHttpRouteUnavailable { .. })
        ));
    }

    #[tokio::test]
    async fn artifact_http_dispatch_rejects_invalid_execution_identity() {
        let catalog = ModuleDefinitionCatalog::default();
        let executor = EchoArtifactExecutor;
        let dispatcher = ModuleExecutionDispatcher::artifact_only(&catalog, &executor);
        let release = ArtifactReleaseRef {
            slug: "artifact_module".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };
        let tenant_id = uuid::Uuid::new_v4();
        let admission = ArtifactExecutionAdmission::owner_revalidation(
            tenant_id,
            &release,
            ArtifactInstallationTarget::CurrentRelease,
        );

        assert!(matches!(
            dispatcher
                .dispatch_artifact_http(
                    tenant_id,
                    "artifact_module",
                    ModuleHttpMethod::Post,
                    "status/query",
                    serde_json::json!({}),
                    admission,
                    ArtifactBindingExecutionContext {
                        actor_id: Some(String::new()),
                        trace_id: None,
                    },
                )
                .await,
            Err(ModuleDispatchError::InvalidArtifactExecutionContext)
        ));
    }

    #[tokio::test]
    async fn artifact_only_dispatcher_uses_admitted_executor_without_static_registry() {
        let release = ArtifactReleaseRef {
            slug: "artifact_module".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };
        let mut catalog = ModuleDefinitionCatalog::default();
        catalog
            .insert(ModuleDefinition {
                slug: release.slug.clone(),
                version: release.version.clone(),
                kind: ModuleDefinitionKind::Optional,
                source: ModuleDefinitionSource::Artifact {
                    release: release.clone(),
                },
                dependencies: Vec::new(),
                permissions: Vec::new(),
                settings_schema_digest: None,
                schema_documents: Vec::new(),
                bindings: vec![ModuleRuntimeBinding {
                    id: "pre_disable".to_string(),
                    kind: ModuleRuntimeBindingKind::PreDisable,
                    entrypoint: "lifecycle.pre_disable".to_string(),
                    input_schema_digest: format!("sha256:{}", "b".repeat(64)),
                    output_schema_digest: format!("sha256:{}", "c".repeat(64)),
                    permission: "module.lifecycle.disable".to_string(),
                    idempotency: ModuleBindingIdempotency::Required,
                    limit_profile: "lifecycle".to_string(),
                    capabilities: Vec::new(),
                    event_topics: Vec::new(),
                    schedule: None,
                    http: None,
                }],
                ui: Vec::new(),
                capabilities: Vec::new(),
            })
            .expect("artifact definition");
        let executor = RecordingArtifactExecutor(Mutex::new(Vec::new()));
        let dispatcher = ModuleExecutionDispatcher::artifact_only(&catalog, &executor);
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");

        dispatcher
            .dispatch_lifecycle(
                &database,
                uuid::Uuid::new_v4(),
                "artifact_module",
                &serde_json::json!({}),
                ModuleLifecycleHookPhase::PreDisable,
            )
            .await
            .expect("artifact dispatch");

        assert_eq!(
            *executor.0.lock().expect("executor lock"),
            vec![(
                "artifact_module".to_string(),
                ModuleLifecycleHookPhase::PreDisable
            )]
        );
    }
}
