//! Definition-aware dispatch for module runtime bindings.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rustok_core::{ModuleContext, ModuleRegistry};
use rustok_sandbox::ExecutionPhase;

use crate::{
    artifact::{event_topic_matches, valid_event_topic},
    ArtifactReleaseRef, ModuleDefinitionCatalog, ModuleDefinitionSource, ModuleHttpMethod,
    ModuleRuntimeBinding, ModuleRuntimeBindingKind,
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
                let binding = definition
                    .bindings
                    .iter()
                    .find(|binding| binding.kind == kind)
                    .ok_or_else(|| {
                        ModuleDispatchError::ArtifactBindingUnavailable(module_slug.to_string())
                    })?;
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
        let executor = self.artifact_executor.ok_or_else(|| {
            ModuleDispatchError::ArtifactExecutorUnavailable(module_slug.to_string())
        })?;
        executor
            .dispatch_binding(ArtifactBindingDispatch {
                release,
                binding,
                target: ArtifactInstallationTarget::CurrentRelease,
                tenant_id,
                input,
                phase,
                context,
            })
            .await
            .map_err(ModuleDispatchError::ArtifactHook)
    }

    /// Dispatches every admitted Event binding whose exact or terminal-wildcard
    /// subscription matches the supplied platform event type.
    pub async fn dispatch_artifact_event(
        &self,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>, ModuleDispatchError> {
        if !valid_delivered_event_type(event_type) {
            return Err(ModuleDispatchError::InvalidArtifactEventType(
                event_type.to_string(),
            ));
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
        let mut outputs = Vec::with_capacity(binding_ids.len());
        for binding_id in binding_ids {
            outputs.push(
                self.dispatch_artifact_binding(
                    tenant_id,
                    module_slug,
                    &binding_id,
                    serde_json::json!({
                        "binding_id": binding_id.clone(),
                        "event_type": event_type,
                        "payload": payload.clone(),
                    }),
                    ExecutionPhase::Event,
                    ArtifactBindingExecutionContext::default(),
                )
                .await?,
            );
        }
        Ok(outputs)
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
            release,
            &definition.bindings,
            ArtifactInstallationTarget::CurrentRelease,
            tenant_id,
            method,
            path,
            body,
            context,
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

/// Dispatches one admitted command binding through an explicit immutable
/// installation target. The host selects the binding and actor context; an
/// artifact cannot create a command route or select another installation.
pub async fn dispatch_artifact_command_binding<E>(
    executor: &E,
    release: &ArtifactReleaseRef,
    bindings: &[ModuleRuntimeBinding],
    target: ArtifactInstallationTarget,
    tenant_id: uuid::Uuid,
    binding_id: &str,
    input: serde_json::Value,
    context: ArtifactBindingExecutionContext,
) -> Result<serde_json::Value, ModuleDispatchError>
where
    E: ArtifactBindingExecutor + ?Sized,
{
    if !context.is_valid() {
        return Err(ModuleDispatchError::InvalidArtifactExecutionContext);
    }
    let binding = find_artifact_command_binding(bindings, binding_id).ok_or_else(|| {
        ModuleDispatchError::ArtifactCommandUnavailable {
            module_slug: release.slug.clone(),
            binding_id: binding_id.to_string(),
        }
    })?;
    executor
        .dispatch_binding(ArtifactBindingDispatch {
            release,
            binding,
            target,
            tenant_id,
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
    release: &ArtifactReleaseRef,
    bindings: &[ModuleRuntimeBinding],
    target: ArtifactInstallationTarget,
    tenant_id: uuid::Uuid,
    method: ModuleHttpMethod,
    path: &str,
    body: serde_json::Value,
    context: ArtifactBindingExecutionContext,
) -> Result<serde_json::Value, ModuleDispatchError>
where
    E: ArtifactBindingExecutor + ?Sized,
{
    if !context.is_valid() {
        return Err(ModuleDispatchError::InvalidArtifactExecutionContext);
    }
    let binding = find_artifact_http_binding(bindings, method, path).ok_or_else(|| {
        ModuleDispatchError::ArtifactHttpRouteUnavailable {
            module_slug: release.slug.clone(),
            path: path.to_string(),
        }
    })?;
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
    ) -> Result<serde_json::Value, String>;
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
    ) -> Result<(), String> {
        self.dispatch_binding(ArtifactBindingDispatch {
            release,
            binding,
            target: ArtifactInstallationTarget::CurrentRelease,
            tenant_id,
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

pub struct ArtifactBindingDispatch<'a> {
    pub release: &'a ArtifactReleaseRef,
    pub binding: &'a ModuleRuntimeBinding,
    /// Current interactive dispatch resolves the effective release. Durable
    /// delivery workers must pin one immutable installation identity so a later
    /// composition change cannot execute a different artifact.
    pub target: ArtifactInstallationTarget,
    pub tenant_id: uuid::Uuid,
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
    ArtifactHook(String),
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
    #[error("module lifecycle binding failed: {0}")]
    StaticHook(String),
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use sea_orm::Database;

    use super::*;
    use crate::{
        ArtifactReleaseRef, ModuleBindingIdempotency, ModuleDefinition, ModuleDefinitionKind,
        ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    };

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
        ) -> Result<serde_json::Value, String> {
            let phase: ModuleLifecycleHookPhase = dispatch
                .input
                .get("phase")
                .cloned()
                .ok_or_else(|| "lifecycle phase is missing".to_string())
                .and_then(|value| {
                    serde_json::from_value(value).map_err(|error| error.to_string())
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
        ) -> Result<serde_json::Value, String> {
            Ok(dispatch.input)
        }
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
        let body = serde_json::json!({ "include": "summary" });

        let output = dispatcher
            .dispatch_artifact_http(
                uuid::Uuid::new_v4(),
                "artifact_module",
                ModuleHttpMethod::Post,
                "status/query",
                body.clone(),
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
                    uuid::Uuid::new_v4(),
                    "artifact_module",
                    ModuleHttpMethod::Get,
                    "status/query",
                    serde_json::json!({}),
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

        assert!(matches!(
            dispatcher
                .dispatch_artifact_http(
                    uuid::Uuid::new_v4(),
                    "artifact_module",
                    ModuleHttpMethod::Post,
                    "status/query",
                    serde_json::json!({}),
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
