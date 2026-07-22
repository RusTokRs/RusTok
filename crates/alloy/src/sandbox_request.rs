//! Versioned request construction for executing an Alloy source revision in
//! the neutral sandbox runtime.

use std::collections::HashMap;

use rhai::{Engine, Scope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use rustok_sandbox::rhai::RhaiHostExtension;
use rustok_sandbox::{
    ExecutionPhase as SandboxExecutionPhase, RhaiBindingInput, RhaiBindingOutput, SandboxContext,
    SandboxError, SandboxExecutorKind, SandboxPayload, SandboxPolicy, SandboxRequest,
    SandboxResult, SandboxSubject,
};

use crate::{
    AlloyWorkspace, Bridge, EntityProxy, ExecutionContext, ExecutionPhase, Script, ScriptError,
    ScriptResult, register_entity_proxy,
    utils::{dynamic_to_json, json_to_dynamic},
};

/// Stable media type for canonical Alloy workspace bytes before they become a
/// module artifact. Published artifacts use their separately admitted descriptor.
pub const ALLOY_DRAFT_RHAI_MEDIA_TYPE: &str = "application/vnd.rustok.rhai.workspace.v1";

/// Alloy-owned data carried inside the shared Rhai v1 input envelope. It keeps
/// user-provided parameters and entity snapshots data-only; the later
/// request-scoped extension owns mutable `EntityProxy` reconstruction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlloyDraftInput {
    #[serde(default = "empty_object")]
    pub params: serde_json::Value,
    #[serde(default)]
    pub entity: Option<AlloyDraftEntitySnapshot>,
    #[serde(default)]
    pub entity_before: Option<AlloyDraftEntitySnapshot>,
}

impl Default for AlloyDraftInput {
    fn default() -> Self {
        Self {
            params: empty_object(),
            entity: None,
            entity_before: None,
        }
    }
}

impl AlloyDraftInput {
    pub fn validate(&self) -> Result<(), AlloyDraftBindingError> {
        if !self.params.is_object() {
            return Err(AlloyDraftBindingError::ParamsMustBeObject);
        }
        for snapshot in [&self.entity, &self.entity_before].into_iter().flatten() {
            snapshot.validate()?;
        }
        Ok(())
    }
}

/// Serializable source data for an Alloy entity. It intentionally has no Rhai
/// dynamic values or executable fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlloyDraftEntitySnapshot {
    pub id: String,
    pub entity_type: String,
    #[serde(default = "empty_object")]
    pub fields: serde_json::Value,
}

impl AlloyDraftEntitySnapshot {
    fn validate(&self) -> Result<(), AlloyDraftBindingError> {
        if self.id.trim().is_empty() {
            return Err(AlloyDraftBindingError::EmptyEntityId);
        }
        if self.entity_type.trim().is_empty() {
            return Err(AlloyDraftBindingError::EmptyEntityType);
        }
        if !self.fields.is_object() {
            return Err(AlloyDraftBindingError::EntityFieldsMustBeObject);
        }
        Ok(())
    }
}

/// Alloy-owned data carried inside the shared Rhai v1 output envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlloyDraftOutput {
    #[serde(default)]
    pub return_value: serde_json::Value,
    #[serde(default = "empty_object")]
    pub entity_changes: serde_json::Value,
}

impl AlloyDraftOutput {
    pub fn validate(&self) -> Result<(), AlloyDraftBindingError> {
        if !self.entity_changes.is_object() {
            return Err(AlloyDraftBindingError::EntityChangesMustBeObject);
        }
        Ok(())
    }
}

/// Builds the sole neutral-runtime request representation for an Alloy source
/// revision. It does not execute the request; that remains the responsibility
/// of `SandboxRuntime` once Alloy's context extension is wired.
#[derive(Clone, Debug)]
pub struct AlloyDraftRequestBuilder {
    policy: SandboxPolicy,
}

/// Alloy-owned adapter from the legacy orchestration DTOs to one neutral
/// sandbox execution. It is intentionally the only place that translates
/// entity/parameter snapshots and typed output back to Alloy values.
#[derive(Clone)]
pub struct AlloyDraftRuntime {
    sandbox: rustok_sandbox::SandboxRuntime,
    requests: AlloyDraftRequestBuilder,
}

impl AlloyDraftRuntime {
    pub fn new(sandbox: rustok_sandbox::SandboxRuntime, policy: SandboxPolicy) -> Self {
        Self {
            sandbox,
            requests: AlloyDraftRequestBuilder::new(policy),
        }
    }

    pub async fn execute(
        &self,
        script: &Script,
        context: &ExecutionContext,
    ) -> ScriptResult<(rhai::Dynamic, HashMap<String, rhai::Dynamic>)> {
        let request = self
            .requests
            .build(script, context, input_from_context(context))
            .map_err(|error| ScriptError::Runtime(error.to_string()))?;
        self.execute_request(request).await
    }

    /// Executes one immutable `tests/*.rhai` entrypoint from the script's
    /// canonical workspace. Test requests retain the workspace digest and
    /// revision identity but intentionally receive no capability grants.
    pub async fn execute_test(
        &self,
        script: &Script,
        test_path: &str,
        context: &ExecutionContext,
    ) -> ScriptResult<bool> {
        let request = self
            .requests
            .build_test(script, test_path, context, AlloyDraftInput::default())
            .map_err(|error| ScriptError::Runtime(error.to_string()))?;
        let (return_value, changes) = self.execute_request(request).await?;
        if !changes.is_empty() {
            return Err(ScriptError::Runtime(
                "Alloy workspace tests must not mutate an entity".to_string(),
            ));
        }
        dynamic_to_json(return_value).as_bool().ok_or_else(|| {
            ScriptError::Runtime("Alloy workspace tests must return a boolean".to_string())
        })
    }

    /// Executes the fixed publication smoke entrypoint through the same
    /// production sandbox runtime used by Alloy. The request builder removes
    /// every capability grant, pins the exact source digest/revision, and the
    /// returned evidence contains no script input or output.
    pub async fn execute_publication_smoke(
        &self,
        script: &Script,
        context: &ExecutionContext,
    ) -> ScriptResult<crate::AlloyPublicationSmokeEvidence> {
        let request = self
            .requests
            .build_test(
                script,
                rustok_modules::ALLOY_PUBLICATION_SMOKE_TEST_PATH,
                context,
                AlloyDraftInput::default(),
            )
            .map_err(|error| ScriptError::Runtime(error.to_string()))?;
        let policy_bytes = serde_json::to_vec(&request.policy)
            .map_err(|error| ScriptError::Runtime(error.to_string()))?;
        let evidence = crate::AlloyPublicationSmokeEvidence {
            execution_id: request.context.execution_id,
            test_path: request.payload.entrypoint.clone(),
            executor: request.payload.executor.to_string(),
            runtime_abi: request.payload.runtime_abi.clone(),
            policy_digest: format!("sha256:{}", hex::encode(Sha256::digest(policy_bytes))),
            capability_grants: request.policy.grants.len().try_into().unwrap_or(u32::MAX),
        };
        let (return_value, changes) = self.execute_request(request).await?;
        if !changes.is_empty() {
            return Err(ScriptError::Runtime(
                "Alloy publication smoke must not mutate an entity".to_string(),
            ));
        }
        if dynamic_to_json(return_value).as_bool() != Some(true) {
            return Err(ScriptError::Runtime(
                "Alloy publication smoke must return true".to_string(),
            ));
        }
        Ok(evidence)
    }

    async fn execute_request(
        &self,
        request: SandboxRequest,
    ) -> ScriptResult<(rhai::Dynamic, HashMap<String, rhai::Dynamic>)> {
        let output = self
            .sandbox
            .execute(request)
            .await
            .map_err(ScriptError::from)?;
        let binding = RhaiBindingOutput::decode(output.output)
            .map_err(|error| ScriptError::Runtime(error.to_string()))?;
        let output: AlloyDraftOutput = serde_json::from_value(binding.output).map_err(|error| {
            ScriptError::Runtime(format!("invalid Alloy sandbox output: {error}"))
        })?;
        output
            .validate()
            .map_err(|error| ScriptError::Runtime(error.to_string()))?;
        Ok((
            json_to_dynamic(output.return_value),
            changes_from_json(output.entity_changes)?,
        ))
    }
}

/// Reconstructs Alloy's request-scoped script values inside the neutral Rhai
/// executor and wraps successful values in the v1 Alloy output binding.
///
/// No state is retained on this extension: `EntityProxy` lives in the supplied
/// `Scope` for exactly one sandbox request.
#[derive(Debug, Default)]
pub struct AlloyDraftScopeExtension;

impl RhaiHostExtension for AlloyDraftScopeExtension {
    fn register(
        &self,
        engine: &mut Engine,
        request: &SandboxRequest,
        _host: rustok_sandbox::SandboxHost,
    ) -> SandboxResult<()> {
        if request.payload.media_type == ALLOY_DRAFT_RHAI_MEDIA_TYPE {
            let workspace: AlloyWorkspace = serde_json::from_slice(&request.payload.bytes)
                .map_err(|error| {
                    SandboxError::InvalidRequest(format!(
                        "invalid Alloy workspace payload: {error}"
                    ))
                })?;
            if request.payload.entrypoint == rustok_modules::ALLOY_PUBLICATION_SMOKE_TEST_PATH {
                workspace.validate_rhai_workspace().map_err(|error| {
                    SandboxError::InvalidRequest(format!(
                        "invalid Alloy production entrypoint for publication: {error}"
                    ))
                })?;
            }
            workspace
                .configure_rhai_engine_for_entrypoint(engine, &request.payload.entrypoint)
                .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
        }
        if matches!(request.subject, SandboxSubject::AlloyDraft { .. }) {
            Bridge::register_for_phase(engine, alloy_phase(request.context.phase));
            register_entity_proxy(engine);
        }
        Ok(())
    }

    fn source_bytes(&self, request: &SandboxRequest) -> SandboxResult<Option<Vec<u8>>> {
        if request.payload.media_type != ALLOY_DRAFT_RHAI_MEDIA_TYPE {
            return Ok(None);
        }
        let workspace: AlloyWorkspace =
            serde_json::from_slice(&request.payload.bytes).map_err(|error| {
                SandboxError::InvalidRequest(format!("invalid Alloy workspace payload: {error}"))
            })?;
        let source = workspace
            .executable_source(&request.payload.entrypoint)
            .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
        Ok(Some(source.as_bytes().to_vec()))
    }

    fn populate_scope(
        &self,
        scope: &mut Scope<'static>,
        request: &SandboxRequest,
    ) -> SandboxResult<()> {
        if !matches!(request.subject, SandboxSubject::AlloyDraft { .. }) {
            return Ok(());
        }
        let input = alloy_input(request)?;
        scope.push_constant("params", json_to_dynamic(input.params.clone()));
        if let Some(snapshot) = input.entity {
            let entity = entity_proxy(snapshot)?;
            if request.context.phase == SandboxExecutionPhase::BeforeHook {
                scope.push("entity", entity);
            } else {
                scope.push_constant("entity", entity);
            }
        }
        if let Some(snapshot) = input.entity_before {
            scope.push_constant("entity_before", entity_proxy(snapshot)?);
        }
        Ok(())
    }

    fn map_output(
        &self,
        scope: &mut Scope<'static>,
        request: &SandboxRequest,
        output: serde_json::Value,
    ) -> SandboxResult<serde_json::Value> {
        if !matches!(request.subject, SandboxSubject::AlloyDraft { .. }) {
            return Ok(output);
        }
        let entity_changes = scope
            .get_value::<EntityProxy>("entity")
            .map(entity_changes)
            .unwrap_or_else(empty_object);
        let result = AlloyDraftOutput {
            return_value: output,
            entity_changes,
        };
        result
            .validate()
            .map_err(|error| SandboxError::Internal(error.to_string()))?;
        serde_json::to_value(result).map_err(|error| SandboxError::Internal(error.to_string()))
    }
}

impl AlloyDraftRequestBuilder {
    pub fn new(policy: SandboxPolicy) -> Self {
        Self { policy }
    }

    pub fn build(
        &self,
        script: &Script,
        context: &ExecutionContext,
        input: AlloyDraftInput,
    ) -> Result<SandboxRequest, AlloyDraftRequestError> {
        self.build_for_entrypoint(
            script,
            &script.workspace.entrypoint,
            context,
            input,
            sandbox_phase(context.phase),
            self.policy.clone(),
        )
    }

    /// Builds a capability-free request for one declared workspace test.
    pub fn build_test(
        &self,
        script: &Script,
        test_path: &str,
        context: &ExecutionContext,
        input: AlloyDraftInput,
    ) -> Result<SandboxRequest, AlloyDraftRequestError> {
        script
            .workspace
            .validate_rhai_test(test_path)
            .map_err(AlloyDraftRequestError::Workspace)?;
        self.build_for_entrypoint(
            script,
            test_path,
            context,
            input,
            SandboxExecutionPhase::Test,
            SandboxPolicy {
                grants: Vec::new(),
                limits: self.policy.limits.clone(),
            },
        )
    }

    fn build_for_entrypoint(
        &self,
        script: &Script,
        entrypoint: &str,
        context: &ExecutionContext,
        input: AlloyDraftInput,
        phase: SandboxExecutionPhase,
        policy: SandboxPolicy,
    ) -> Result<SandboxRequest, AlloyDraftRequestError> {
        if script.id.is_nil() {
            return Err(AlloyDraftRequestError::MissingDraftId);
        }
        script
            .workspace
            .validate()
            .map_err(AlloyDraftRequestError::Workspace)?;
        input.validate()?;
        let tenant_id = context
            .tenant_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| AlloyDraftRequestError::InvalidTenantId)?;
        let bytes = script
            .workspace
            .canonical_bytes()
            .map_err(AlloyDraftRequestError::Workspace)?;
        let digest = script
            .workspace
            .digest()
            .map_err(AlloyDraftRequestError::Workspace)?;
        Ok(SandboxRequest {
            subject: SandboxSubject::AlloyDraft {
                draft_id: script.id,
                revision: u64::from(script.version),
            },
            context: SandboxContext {
                execution_id: context.execution_id,
                phase,
                timestamp: context.timestamp,
                tenant_id,
                actor_id: context.user_id.clone(),
                trace_id: None,
            },
            payload: SandboxPayload {
                executor: SandboxExecutorKind::Rhai,
                media_type: ALLOY_DRAFT_RHAI_MEDIA_TYPE.to_string(),
                digest,
                runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
                entrypoint: entrypoint.to_string(),
                bytes,
            },
            input: serde_json::to_value(RhaiBindingInput::new(
                serde_json::to_value(input)
                    .map_err(|error| AlloyDraftRequestError::Serialize(error.to_string()))?,
            ))
            .map_err(|error| AlloyDraftRequestError::Serialize(error.to_string()))?,
            policy,
        })
    }
}

impl Default for AlloyDraftRequestBuilder {
    fn default() -> Self {
        Self::new(SandboxPolicy::default())
    }
}

fn sandbox_phase(phase: ExecutionPhase) -> SandboxExecutionPhase {
    match phase {
        ExecutionPhase::Before => SandboxExecutionPhase::BeforeHook,
        ExecutionPhase::After => SandboxExecutionPhase::AfterHook,
        ExecutionPhase::OnCommit => SandboxExecutionPhase::Event,
        ExecutionPhase::Manual => SandboxExecutionPhase::Manual,
        ExecutionPhase::Scheduled => SandboxExecutionPhase::Scheduled,
    }
}

fn alloy_phase(phase: SandboxExecutionPhase) -> ExecutionPhase {
    match phase {
        SandboxExecutionPhase::BeforeHook => ExecutionPhase::Before,
        SandboxExecutionPhase::AfterHook => ExecutionPhase::After,
        SandboxExecutionPhase::Event => ExecutionPhase::OnCommit,
        SandboxExecutionPhase::Scheduled => ExecutionPhase::Scheduled,
        SandboxExecutionPhase::Validate
        | SandboxExecutionPhase::Test
        | SandboxExecutionPhase::Manual
        | SandboxExecutionPhase::Http
        | SandboxExecutionPhase::Lifecycle => ExecutionPhase::Manual,
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AlloyDraftRequestError {
    #[error(transparent)]
    Binding(#[from] AlloyDraftBindingError),
    #[error("Alloy draft id must not be nil")]
    MissingDraftId,
    #[error("Alloy draft workspace is invalid: {0}")]
    Workspace(#[from] crate::WorkspaceError),
    #[error("Alloy execution context tenant id is not a UUID")]
    InvalidTenantId,
    #[error("Alloy draft binding serialization failed: {0}")]
    Serialize(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AlloyDraftBindingError {
    #[error("Alloy draft params must be a JSON object")]
    ParamsMustBeObject,
    #[error("Alloy draft entity id must not be empty")]
    EmptyEntityId,
    #[error("Alloy draft entity type must not be empty")]
    EmptyEntityType,
    #[error("Alloy draft entity fields must be a JSON object")]
    EntityFieldsMustBeObject,
    #[error("Alloy draft entity changes must be a JSON object")]
    EntityChangesMustBeObject,
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn alloy_input(request: &SandboxRequest) -> SandboxResult<AlloyDraftInput> {
    let binding = RhaiBindingInput::decode(request.input.clone())
        .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
    let input: AlloyDraftInput = serde_json::from_value(binding.input).map_err(|error| {
        SandboxError::InvalidRequest(format!("invalid Alloy draft input: {error}"))
    })?;
    input
        .validate()
        .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
    Ok(input)
}

fn entity_proxy(snapshot: AlloyDraftEntitySnapshot) -> SandboxResult<EntityProxy> {
    snapshot
        .validate()
        .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
    let fields = snapshot
        .fields
        .as_object()
        .expect("validated Alloy draft entity fields are an object")
        .iter()
        .map(|(key, value)| (key.clone(), json_to_dynamic(value.clone())))
        .collect::<HashMap<_, _>>();
    Ok(EntityProxy::new(snapshot.id, snapshot.entity_type, fields))
}

fn entity_changes(entity: EntityProxy) -> serde_json::Value {
    serde_json::Value::Object(
        entity
            .changes()
            .into_iter()
            .map(|(key, value)| (key, dynamic_to_json(value)))
            .collect(),
    )
}

fn input_from_context(context: &ExecutionContext) -> AlloyDraftInput {
    AlloyDraftInput {
        params: serde_json::Value::Object(
            context
                .params
                .iter()
                .map(|(key, value)| (key.to_string(), dynamic_to_json(value.clone())))
                .collect(),
        ),
        entity: context.entity_proxy.as_ref().map(entity_snapshot),
        entity_before: context.entity_before_proxy.as_ref().map(entity_snapshot),
    }
}

fn entity_snapshot(entity: &EntityProxy) -> AlloyDraftEntitySnapshot {
    AlloyDraftEntitySnapshot {
        id: entity.id().to_string(),
        entity_type: entity.entity_type().to_string(),
        fields: serde_json::Value::Object(
            entity
                .snapshot()
                .into_iter()
                .map(|(key, value)| (key, dynamic_to_json(value)))
                .collect(),
        ),
    }
}

fn changes_from_json(changes: serde_json::Value) -> ScriptResult<HashMap<String, rhai::Dynamic>> {
    let changes = changes.as_object().ok_or_else(|| {
        ScriptError::Runtime("Alloy sandbox output entity changes must be an object".into())
    })?;
    Ok(changes
        .iter()
        .map(|(key, value)| (key.clone(), json_to_dynamic(value.clone())))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{ScriptStatus, ScriptTrigger};
    use async_trait::async_trait;
    use rustok_sandbox::{
        CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse, ExecutorRegistry,
        SandboxError, SandboxResult, SandboxRuntime,
    };

    struct NoCapabilities;

    #[async_trait]
    impl CapabilityBroker for NoCapabilities {
        async fn invoke(
            &self,
            _call: &CapabilityCall,
            _grant: &CapabilityGrant,
        ) -> SandboxResult<CapabilityResponse> {
            Err(SandboxError::Internal("unexpected capability call".into()))
        }
    }

    fn script() -> Script {
        let mut script = Script::new(
            "draft",
            AlloyWorkspace::single_source("input.params.value + 1"),
            ScriptTrigger::Manual,
        );
        script.id = Uuid::new_v4();
        script.version = 7;
        script.status = ScriptStatus::Draft;
        script
    }

    #[test]
    fn builder_pins_source_revision_and_execution_evidence() {
        let script = script();
        let context = ExecutionContext::new(ExecutionPhase::Manual)
            .with_tenant(Uuid::new_v4().to_string())
            .with_user("user:42");
        let request = AlloyDraftRequestBuilder::default()
            .build(
                &script,
                &context,
                AlloyDraftInput {
                    params: serde_json::json!({ "value": 41 }),
                    ..Default::default()
                },
            )
            .expect("request");

        assert_eq!(
            request.subject,
            SandboxSubject::AlloyDraft {
                draft_id: script.id,
                revision: 7,
            }
        );
        assert_eq!(request.context.execution_id, context.execution_id);
        assert_eq!(request.context.phase, SandboxExecutionPhase::Manual);
        assert_eq!(request.payload.executor, SandboxExecutorKind::Rhai);
        assert_eq!(request.payload.media_type, ALLOY_DRAFT_RHAI_MEDIA_TYPE);
        assert_eq!(
            request.payload.digest,
            script.workspace.digest().expect("workspace digest")
        );
        assert_eq!(
            RhaiBindingInput::decode(request.input)
                .expect("versioned Rhai input")
                .input,
            serde_json::json!({
                "params": { "value": 41 },
                "entity": null,
                "entity_before": null,
            })
        );
    }

    #[test]
    fn builder_rejects_invalid_tenant_context() {
        assert_eq!(
            AlloyDraftRequestBuilder::default()
                .build(
                    &script(),
                    &ExecutionContext::new(ExecutionPhase::Manual).with_tenant("not-a-uuid"),
                    AlloyDraftInput::default(),
                )
                .expect_err("invalid tenant id"),
            AlloyDraftRequestError::InvalidTenantId
        );
    }

    #[test]
    fn test_builder_pins_a_declared_test_entrypoint_and_removes_capabilities() {
        let mut script = script();
        script.workspace.files.push(crate::WorkspaceFile {
            path: "tests/smoke.rhai".into(),
            kind: crate::WorkspaceFileKind::Test,
            contents: "true".into(),
        });
        let request = AlloyDraftRequestBuilder::new(SandboxPolicy {
            grants: vec![CapabilityGrant {
                name: rustok_sandbox::CapabilityName::new("platform.http")
                    .expect("capability name"),
                constraints: serde_json::json!({}),
            }],
            ..Default::default()
        })
        .build_test(
            &script,
            "tests/smoke.rhai",
            &ExecutionContext::new(ExecutionPhase::Manual).with_tenant(Uuid::new_v4().to_string()),
            AlloyDraftInput::default(),
        )
        .expect("test request");

        assert_eq!(request.context.phase, SandboxExecutionPhase::Test);
        assert_eq!(request.payload.entrypoint, "tests/smoke.rhai");
        assert_eq!(
            request.payload.digest,
            script.workspace.digest().expect("workspace digest")
        );
        assert!(request.policy.grants.is_empty());
    }

    #[tokio::test]
    async fn publication_smoke_returns_redacted_exact_sandbox_evidence() {
        let mut script = script();
        script.workspace.files.push(crate::WorkspaceFile {
            path: rustok_modules::ALLOY_PUBLICATION_SMOKE_TEST_PATH.into(),
            kind: crate::WorkspaceFileKind::Test,
            contents: "true".into(),
        });
        let execution_id = Uuid::new_v4();
        let mut context = ExecutionContext::new(ExecutionPhase::Manual)
            .with_tenant(Uuid::new_v4().to_string())
            .with_user("release-operator");
        context.execution_id = execution_id;

        let evidence = crate::create_default_alloy_draft_runtime()
            .execute_publication_smoke(&script, &context)
            .await
            .expect("publication smoke");

        assert_eq!(evidence.execution_id, execution_id);
        assert_eq!(
            evidence.test_path,
            rustok_modules::ALLOY_PUBLICATION_SMOKE_TEST_PATH
        );
        assert_eq!(evidence.executor, "rhai");
        assert_eq!(
            evidence.runtime_abi,
            rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI
        );
        assert!(evidence.policy_digest.starts_with("sha256:"));
        assert_eq!(evidence.capability_grants, 0);
    }

    #[test]
    fn binding_rejects_non_object_entity_changes() {
        assert_eq!(
            AlloyDraftOutput {
                return_value: serde_json::Value::Null,
                entity_changes: serde_json::json!(["not", "an", "object"]),
            }
            .validate(),
            Err(AlloyDraftBindingError::EntityChangesMustBeObject)
        );
    }

    #[tokio::test]
    async fn scope_extension_returns_pre_hook_entity_changes() {
        let mut script = script();
        script.workspace =
            AlloyWorkspace::single_source("entity[\"status\"] = \"approved\"; params[\"amount\"]");
        let context =
            ExecutionContext::new(ExecutionPhase::Before).with_tenant(Uuid::new_v4().to_string());
        let request = AlloyDraftRequestBuilder::default()
            .build(
                &script,
                &context,
                AlloyDraftInput {
                    params: serde_json::json!({ "amount": 42 }),
                    entity: Some(AlloyDraftEntitySnapshot {
                        id: "deal-1".into(),
                        entity_type: "deal".into(),
                        fields: serde_json::json!({ "status": "pending" }),
                    }),
                    ..Default::default()
                },
            )
            .expect("request");
        let mut executors = ExecutorRegistry::new();
        executors
            .register(
                rustok_sandbox::rhai::RhaiExecutor::new()
                    .with_extension(Arc::new(AlloyDraftScopeExtension)),
            )
            .expect("register executor");
        let output = SandboxRuntime::new(executors, Arc::new(NoCapabilities))
            .execute(request)
            .await
            .expect("execute draft");
        let output: AlloyDraftOutput = serde_json::from_value(
            RhaiBindingOutput::decode(output.output)
                .expect("versioned Rhai output")
                .output,
        )
        .expect("typed draft output");
        assert_eq!(output.return_value, serde_json::json!(42));
        assert_eq!(
            output.entity_changes,
            serde_json::json!({ "status": "approved" })
        );
    }
}
