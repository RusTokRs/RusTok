//! Versioned request construction for executing an Alloy source revision in
//! the neutral sandbox runtime.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use rustok_sandbox::{
    ExecutionPhase as SandboxExecutionPhase, RHAI_WORKSPACE_MEDIA_TYPE, RhaiBindingInput,
    RhaiBindingOutput, RhaiRecordInput, RhaiScopeInput, SandboxContext, SandboxExecutorKind,
    SandboxPayload, SandboxPolicy, SandboxRequest, SandboxSubject,
};

use crate::{
    EntityProxy, ExecutionContext, ExecutionPhase, Script, ScriptError, ScriptResult,
    utils::{dynamic_to_json, json_to_dynamic},
};

/// Host-owned policy lookup for a draft imported from an installed marketplace
/// release. Alloy persists only immutable parent lineage; it does not persist,
/// synthesize, or broaden a host capability grant.
#[async_trait]
pub trait AlloyImportedDraftPolicyProvider: Send + Sync {
    async fn resolve_policy(
        &self,
        tenant_id: Uuid,
        parent_release: &rustok_modules::ArtifactReleaseRef,
    ) -> Result<SandboxPolicy, AlloyImportedDraftPolicyError>;
}

/// Host-composed policy provider retained by the Alloy draft runtime.
#[derive(Clone)]
pub struct AlloyImportedDraftPolicyProviderHandle(pub Arc<dyn AlloyImportedDraftPolicyProvider>);

/// Redacted failure contract for resolving an imported draft's installed-parent
/// policy. The detailed owner failure remains outside Alloy's transport output.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AlloyImportedDraftPolicyError {
    #[error("the imported draft's installed parent policy is unavailable")]
    Unavailable,
}

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
    imported_draft_policy: Option<AlloyImportedDraftPolicyProviderHandle>,
}

/// Redacted immutable identity persisted with every production draft
/// execution. It contains no source, input, output, or capability result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlloyExecutionEvidence {
    pub source_revision: u32,
    pub source_digest: String,
    pub policy_digest: String,
    pub executor: String,
    pub runtime_abi: String,
}

impl AlloyDraftRuntime {
    pub fn new(sandbox: rustok_sandbox::SandboxRuntime, policy: SandboxPolicy) -> Self {
        Self {
            sandbox,
            requests: AlloyDraftRequestBuilder::new(policy),
            imported_draft_policy: None,
        }
    }

    /// Enables exact installed-parent policy resolution for imported drafts.
    /// A parent-bearing draft fails closed when this host-owned provider is not
    /// present or cannot resolve its currently eligible installation policy.
    pub fn with_imported_draft_policy_provider(
        mut self,
        provider: AlloyImportedDraftPolicyProviderHandle,
    ) -> Self {
        self.imported_draft_policy = Some(provider);
        self
    }

    pub async fn execute(
        &self,
        script: &Script,
        context: &ExecutionContext,
    ) -> ScriptResult<(rhai::Dynamic, HashMap<String, rhai::Dynamic>)> {
        let policy = self.policy_for(script).await?;
        let request = self
            .requests
            .build_with_policy(script, context, input_from_context(context), policy)
            .map_err(|error| ScriptError::Runtime(error.to_string()))?;
        self.execute_request(request).await
    }

    pub async fn execution_evidence(
        &self,
        script: &Script,
    ) -> ScriptResult<AlloyExecutionEvidence> {
        script.workspace.validate().map_err(ScriptError::from)?;
        let source_digest = script.workspace.digest().map_err(ScriptError::from)?;
        let policy = self.policy_for(script).await?;
        let policy_bytes =
            serde_json::to_vec(&policy).map_err(|error| ScriptError::Runtime(error.to_string()))?;
        Ok(AlloyExecutionEvidence {
            source_revision: script.version,
            source_digest,
            policy_digest: format!("sha256:{}", hex::encode(Sha256::digest(policy_bytes))),
            executor: SandboxExecutorKind::Rhai.to_string(),
            runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
        })
    }

    /// Executes one immutable `tests/*.rhai` entrypoint from the script's
    /// canonical workspace. Test requests retain the workspace digest,
    /// revision identity, and the exact installed-parent policy when the
    /// draft was imported from a marketplace artifact.
    pub async fn execute_test(
        &self,
        script: &Script,
        test_path: &str,
        context: &ExecutionContext,
    ) -> ScriptResult<bool> {
        let policy = self.policy_for(script).await?;
        let request = self
            .requests
            .build_test_with_policy(
                script,
                test_path,
                context,
                AlloyDraftInput::default(),
                policy,
            )
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
        let parent_policy = self.policy_for(script).await?;
        let request = self
            .requests
            .build_test_with_policy(
                script,
                rustok_modules::ALLOY_PUBLICATION_SMOKE_TEST_PATH,
                context,
                AlloyDraftInput::default(),
                SandboxPolicy {
                    grants: Vec::new(),
                    limits: parent_policy.limits,
                },
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

    async fn policy_for(&self, script: &Script) -> ScriptResult<SandboxPolicy> {
        let Some(parent_release) = &script.parent_release else {
            return Ok(self.requests.policy.clone());
        };
        if script.tenant_id.is_nil() {
            return Err(ScriptError::Runtime(
                "imported Alloy draft has no tenant identity".to_string(),
            ));
        }
        let provider = self.imported_draft_policy.as_ref().ok_or_else(|| {
            ScriptError::Runtime(
                "imported Alloy draft parent policy provider is unavailable".to_string(),
            )
        })?;
        provider
            .0
            .resolve_policy(script.tenant_id, parent_release)
            .await
            .map_err(|_| {
                ScriptError::Runtime(
                    "imported Alloy draft parent policy is unavailable".to_string(),
                )
            })
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
        let entity_changes = output
            .rhai_scope
            .and_then(|scope| scope.record_changes.get("entity").cloned())
            .unwrap_or_else(empty_object);
        Ok((
            json_to_dynamic(binding.output),
            changes_from_json(entity_changes)?,
        ))
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
        self.build_with_policy(script, context, input, self.policy.clone())
    }

    /// Builds a request with a host-resolved policy. Runtime callers use this
    /// for imported drafts so capability grants cannot fall back to defaults.
    pub fn build_with_policy(
        &self,
        script: &Script,
        context: &ExecutionContext,
        input: AlloyDraftInput,
        policy: SandboxPolicy,
    ) -> Result<SandboxRequest, AlloyDraftRequestError> {
        self.build_for_entrypoint(
            script,
            &script.workspace.entrypoint,
            context,
            input,
            sandbox_phase(context.phase),
            policy,
        )
    }

    /// Builds a request for one declared workspace test using the supplied
    /// host policy. The test phase remains part of the sandbox context so the
    /// broker can enforce phase-specific capability constraints.
    pub fn build_test(
        &self,
        script: &Script,
        test_path: &str,
        context: &ExecutionContext,
        input: AlloyDraftInput,
    ) -> Result<SandboxRequest, AlloyDraftRequestError> {
        self.build_test_with_policy(script, test_path, context, input, self.policy.clone())
    }

    pub fn build_test_with_policy(
        &self,
        script: &Script,
        test_path: &str,
        context: &ExecutionContext,
        input: AlloyDraftInput,
        policy: SandboxPolicy,
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
            policy,
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
        let rhai_scope = rhai_scope_from_input(&input, phase);
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
                audit_label: None,
            },
            payload: SandboxPayload {
                executor: SandboxExecutorKind::Rhai,
                media_type: RHAI_WORKSPACE_MEDIA_TYPE.to_string(),
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
            rhai_scope: Some(rhai_scope),
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AlloyDraftRequestError {
    #[error(transparent)]
    Binding(#[from] AlloyDraftBindingError),
    #[error("Alloy draft id must not be nil")]
    MissingDraftId,
    #[error("Alloy draft workspace is invalid: {0}")]
    Workspace(#[from] crate::RhaiWorkspaceError),
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
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn rhai_scope_from_input(input: &AlloyDraftInput, phase: SandboxExecutionPhase) -> RhaiScopeInput {
    let mut constants = std::collections::BTreeMap::new();
    constants.insert("params".to_string(), input.params.clone());
    let mut records = std::collections::BTreeMap::new();
    if let Some(snapshot) = &input.entity {
        records.insert(
            "entity".to_string(),
            RhaiRecordInput {
                id: snapshot.id.clone(),
                record_type: snapshot.entity_type.clone(),
                fields: snapshot.fields.clone(),
                mutable: phase == SandboxExecutionPhase::BeforeHook,
            },
        );
    }
    if let Some(snapshot) = &input.entity_before {
        records.insert(
            "entity_before".to_string(),
            RhaiRecordInput {
                id: snapshot.id.clone(),
                record_type: snapshot.entity_type.clone(),
                fields: snapshot.fields.clone(),
                mutable: false,
            },
        );
    }
    RhaiScopeInput { constants, records }
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
        RhaiWorkspace, SandboxError, SandboxResult, SandboxRuntime,
    };

    struct NoCapabilities;

    #[derive(Clone)]
    struct FixedImportedDraftPolicyProvider {
        policy: SandboxPolicy,
    }

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

    #[async_trait]
    impl AlloyImportedDraftPolicyProvider for FixedImportedDraftPolicyProvider {
        async fn resolve_policy(
            &self,
            _tenant_id: Uuid,
            _parent_release: &rustok_modules::ArtifactReleaseRef,
        ) -> Result<SandboxPolicy, AlloyImportedDraftPolicyError> {
            Ok(self.policy.clone())
        }
    }

    fn parent_release() -> rustok_modules::ArtifactReleaseRef {
        rustok_modules::ArtifactReleaseRef {
            slug: "tax_rule".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        }
    }

    fn script() -> Script {
        let mut script = Script::new(
            "draft",
            RhaiWorkspace::single_source("input.params.value + 1"),
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
        assert_eq!(request.payload.media_type, RHAI_WORKSPACE_MEDIA_TYPE);
        assert_eq!(
            request.payload.digest,
            script.workspace.digest().expect("workspace digest")
        );
        assert_eq!(
            request
                .rhai_scope
                .as_ref()
                .and_then(|scope| scope.constants.get("params")),
            Some(&serde_json::json!({ "value": 41 }))
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
    fn test_builder_pins_a_declared_test_entrypoint_and_preserves_host_policy() {
        let mut script = script();
        script.workspace.files.push(crate::RhaiWorkspaceFile {
            path: "tests/smoke.rhai".into(),
            kind: crate::RhaiWorkspaceFileKind::Test,
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
        assert_eq!(request.policy.grants.len(), 1);
    }

    #[tokio::test]
    async fn imported_draft_uses_exact_parent_policy_for_tests_and_execution_evidence() {
        let policy = SandboxPolicy {
            grants: vec![CapabilityGrant {
                name: rustok_sandbox::CapabilityName::new("platform.http")
                    .expect("capability name"),
                constraints: serde_json::json!({ "methods": ["GET"] }),
            }],
            ..Default::default()
        };
        let runtime = crate::create_test_alloy_draft_runtime().with_imported_draft_policy_provider(
            AlloyImportedDraftPolicyProviderHandle(Arc::new(FixedImportedDraftPolicyProvider {
                policy: policy.clone(),
            })),
        );
        let mut script = script();
        script.tenant_id = Uuid::new_v4();
        script.parent_release = Some(parent_release());
        script.workspace.files.push(crate::RhaiWorkspaceFile {
            path: "tests/smoke.rhai".into(),
            kind: crate::RhaiWorkspaceFileKind::Test,
            contents: "true".into(),
        });
        let context =
            ExecutionContext::new(ExecutionPhase::Manual).with_tenant(script.tenant_id.to_string());

        let resolved = runtime.policy_for(&script).await.expect("parent policy");
        let request = runtime
            .requests
            .build_test_with_policy(
                &script,
                "tests/smoke.rhai",
                &context,
                AlloyDraftInput::default(),
                resolved,
            )
            .expect("test request");
        let evidence = runtime
            .execution_evidence(&script)
            .await
            .expect("execution evidence");

        assert_eq!(request.policy, policy);
        assert_eq!(
            request.payload.runtime_abi,
            rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI
        );
        assert_eq!(
            evidence.policy_digest,
            format!(
                "sha256:{}",
                hex::encode(Sha256::digest(
                    serde_json::to_vec(&policy).expect("policy serialization")
                ))
            )
        );
    }

    #[tokio::test]
    async fn imported_draft_fails_closed_without_a_parent_policy_provider() {
        let mut script = script();
        script.tenant_id = Uuid::new_v4();
        script.parent_release = Some(parent_release());

        let error = crate::create_test_alloy_draft_runtime()
            .policy_for(&script)
            .await
            .expect_err("imported draft must not use the default policy");

        assert!(matches!(
            error,
            ScriptError::Runtime(message)
                if message == "imported Alloy draft parent policy provider is unavailable"
        ));
    }

    #[tokio::test]
    async fn publication_smoke_returns_redacted_exact_sandbox_evidence() {
        let mut script = script();
        script.workspace.files.push(crate::RhaiWorkspaceFile {
            path: rustok_modules::ALLOY_PUBLICATION_SMOKE_TEST_PATH.into(),
            kind: crate::RhaiWorkspaceFileKind::Test,
            contents: "true".into(),
        });
        let execution_id = Uuid::new_v4();
        let mut context = ExecutionContext::new(ExecutionPhase::Manual)
            .with_tenant(Uuid::new_v4().to_string())
            .with_user("release-operator");
        context.execution_id = execution_id;

        let evidence = crate::create_test_alloy_draft_runtime()
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

    #[tokio::test]
    async fn neutral_scope_returns_pre_hook_entity_changes() {
        let mut script = script();
        script.workspace =
            RhaiWorkspace::single_source("entity[\"status\"] = \"approved\"; params[\"amount\"]");
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
            .register_in_process(rustok_sandbox::rhai::RhaiExecutor::new())
            .expect("register executor");
        let output = SandboxRuntime::new(executors, Arc::new(NoCapabilities))
            .execute(request)
            .await
            .expect("execute draft");
        let binding = RhaiBindingOutput::decode(output.output).expect("versioned Rhai output");
        assert_eq!(binding.output, serde_json::json!(42));
        assert_eq!(
            output
                .rhai_scope
                .expect("scope output")
                .record_changes
                .get("entity"),
            Some(&serde_json::json!({ "status": "approved" }))
        );
    }
}
