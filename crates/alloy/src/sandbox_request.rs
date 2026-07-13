//! Versioned request construction for executing an Alloy source revision in
//! the neutral sandbox runtime.

use std::collections::HashMap;

use rhai::{Engine, Scope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use rustok_sandbox::{
    ExecutionPhase as SandboxExecutionPhase, SandboxContext, SandboxExecutorKind, SandboxPayload,
    SandboxError, SandboxPolicy, SandboxRequest, SandboxResult, SandboxSubject,
};
use rustok_sandbox::rhai::RhaiHostExtension;

use crate::{
    register_entity_proxy, EntityProxy, ExecutionContext, ExecutionPhase, Script,
    utils::{dynamic_to_json, json_to_dynamic},
};

/// Stable media type for Alloy-authored Rhai source before it becomes a module
/// artifact. Published artifacts use their separately admitted descriptor.
pub const ALLOY_DRAFT_RHAI_MEDIA_TYPE: &str = "application/vnd.rustok.alloy.rhai-source.v1";

/// The only v1 input shape an Alloy sandbox adapter may expose to Rhai. It
/// keeps user-provided parameters and entity snapshots data-only; the later
/// request-scoped extension owns mutable `EntityProxy` reconstruction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlloyDraftInput {
    pub binding_version: u32,
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
            binding_version: 1,
            params: empty_object(),
            entity: None,
            entity_before: None,
        }
    }
}

impl AlloyDraftInput {
    pub fn validate(&self) -> Result<(), AlloyDraftBindingError> {
        if self.binding_version != 1 {
            return Err(AlloyDraftBindingError::UnsupportedVersion(
                self.binding_version,
            ));
        }
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

/// The v1 data-only result produced by the planned Alloy sandbox adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlloyDraftOutput {
    pub binding_version: u32,
    #[serde(default)]
    pub return_value: serde_json::Value,
    #[serde(default = "empty_object")]
    pub entity_changes: serde_json::Value,
}

impl AlloyDraftOutput {
    pub fn validate(&self) -> Result<(), AlloyDraftBindingError> {
        if self.binding_version != 1 {
            return Err(AlloyDraftBindingError::UnsupportedVersion(
                self.binding_version,
            ));
        }
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
    ) {
        if matches!(request.subject, SandboxSubject::AlloyDraft { .. }) {
            register_entity_proxy(engine);
        }
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
        scope.push_constant("params", json_to_dynamic(&input.params));
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
            binding_version: 1,
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
        if script.id.is_nil() {
            return Err(AlloyDraftRequestError::MissingDraftId);
        }
        if script.code.trim().is_empty() {
            return Err(AlloyDraftRequestError::EmptySource);
        }
        input.validate()?;
        let tenant_id = context
            .tenant_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| AlloyDraftRequestError::InvalidTenantId)?;
        let digest = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(script.code.as_bytes()))
        );
        Ok(SandboxRequest {
            subject: SandboxSubject::AlloyDraft {
                draft_id: script.id,
                revision: u64::from(script.version),
            },
            context: SandboxContext {
                execution_id: context.execution_id,
                phase: sandbox_phase(context.phase),
                timestamp: context.timestamp,
                tenant_id,
                actor_id: context.user_id.clone(),
                trace_id: None,
            },
            payload: SandboxPayload {
                executor: SandboxExecutorKind::Rhai,
                media_type: ALLOY_DRAFT_RHAI_MEDIA_TYPE.to_string(),
                digest,
                entrypoint: "main".to_string(),
                bytes: script.code.as_bytes().to_vec(),
            },
            input: serde_json::to_value(input)
                .map_err(|error| AlloyDraftRequestError::Serialize(error.to_string()))?,
            policy: self.policy.clone(),
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AlloyDraftRequestError {
    #[error(transparent)]
    Binding(#[from] AlloyDraftBindingError),
    #[error("Alloy draft id must not be nil")]
    MissingDraftId,
    #[error("Alloy draft source must not be empty")]
    EmptySource,
    #[error("Alloy execution context tenant id is not a UUID")]
    InvalidTenantId,
    #[error("Alloy draft binding serialization failed: {0}")]
    Serialize(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AlloyDraftBindingError {
    #[error("Alloy draft binding version `{0}` is unsupported")]
    UnsupportedVersion(u32),
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
    let input = serde_json::from_value(request.input.clone())
        .map_err(|error| SandboxError::InvalidRequest(format!("invalid Alloy draft input: {error}")))?;
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
        .map(|(key, value)| (key.clone(), json_to_dynamic(value)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScriptStatus, ScriptTrigger};

    fn script() -> Script {
        let mut script = Script::new("draft", "input.params.value + 1", ScriptTrigger::Manual);
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
            format!(
                "sha256:{}",
                hex::encode(Sha256::digest(script.code.as_bytes()))
            )
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
    fn binding_rejects_non_object_entity_changes() {
        assert_eq!(
            AlloyDraftOutput {
                binding_version: 1,
                return_value: serde_json::Value::Null,
                entity_changes: serde_json::json!(["not", "an", "object"]),
            }
            .validate(),
            Err(AlloyDraftBindingError::EntityChangesMustBeObject)
        );
    }
}
