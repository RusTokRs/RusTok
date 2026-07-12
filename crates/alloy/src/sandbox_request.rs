//! Versioned request construction for executing an Alloy source revision in
//! the neutral sandbox runtime.

use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use rustok_sandbox::{
    ExecutionPhase as SandboxExecutionPhase, SandboxContext, SandboxExecutorKind, SandboxPayload,
    SandboxPolicy, SandboxRequest, SandboxSubject,
};

use crate::{ExecutionContext, ExecutionPhase, Script};

/// Stable media type for Alloy-authored Rhai source before it becomes a module
/// artifact. Published artifacts use their separately admitted descriptor.
pub const ALLOY_DRAFT_RHAI_MEDIA_TYPE: &str = "application/vnd.rustok.alloy.rhai-source.v1";

/// Builds the sole neutral-runtime request representation for an Alloy source
/// revision. It does not execute the request; that remains the responsibility
/// of `SandboxRuntime` once Alloy's context extension is wired.
#[derive(Clone, Debug)]
pub struct AlloyDraftRequestBuilder {
    policy: SandboxPolicy,
}

impl AlloyDraftRequestBuilder {
    pub fn new(policy: SandboxPolicy) -> Self {
        Self { policy }
    }

    pub fn build(
        &self,
        script: &Script,
        context: &ExecutionContext,
        input: serde_json::Value,
    ) -> Result<SandboxRequest, AlloyDraftRequestError> {
        if script.id.is_nil() {
            return Err(AlloyDraftRequestError::MissingDraftId);
        }
        if script.code.trim().is_empty() {
            return Err(AlloyDraftRequestError::EmptySource);
        }
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
            input,
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
    #[error("Alloy draft id must not be nil")]
    MissingDraftId,
    #[error("Alloy draft source must not be empty")]
    EmptySource,
    #[error("Alloy execution context tenant id is not a UUID")]
    InvalidTenantId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScriptStatus, ScriptTrigger};

    fn script() -> Script {
        let mut script = Script::new("draft", "input.value + 1", ScriptTrigger::manual());
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
            .build(&script, &context, serde_json::json!({ "value": 41 }))
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
                    serde_json::Value::Null,
                )
                .expect_err("invalid tenant id"),
            AlloyDraftRequestError::InvalidTenantId
        );
    }
}
