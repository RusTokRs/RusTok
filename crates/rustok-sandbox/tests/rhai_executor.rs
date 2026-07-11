#![cfg(feature = "rhai")]

use std::sync::Arc;

use async_trait::async_trait;
use rustok_sandbox::rhai::RhaiExecutor;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse, ExecutionPhase,
    ExecutorRegistry, SandboxContext, SandboxError, SandboxExecutorKind, SandboxPayload,
    SandboxPolicy, SandboxRequest, SandboxResult, SandboxRuntime, SandboxSubject,
};
use serde_json::json;
use uuid::Uuid;

struct NoCapabilities;

#[async_trait]
impl CapabilityBroker for NoCapabilities {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        Err(SandboxError::CapabilityDenied(call.capability.clone()))
    }
}

fn request(source: &str) -> SandboxRequest {
    SandboxRequest {
        subject: SandboxSubject::AlloyDraft {
            draft_id: Uuid::new_v4(),
            revision: 1,
        },
        context: SandboxContext::new(ExecutionPhase::Test),
        payload: SandboxPayload {
            executor: SandboxExecutorKind::Rhai,
            media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            digest: "sha256:test".to_string(),
            entrypoint: "main".to_string(),
            bytes: source.as_bytes().to_vec(),
        },
        input: json!({ "left": 20, "right": 22 }),
        policy: SandboxPolicy::default(),
    }
}

fn runtime() -> SandboxRuntime {
    let mut executors = ExecutorRegistry::new();
    executors
        .register(RhaiExecutor::new())
        .expect("register Rhai executor");
    SandboxRuntime::new(executors, Arc::new(NoCapabilities))
}

#[tokio::test]
async fn executes_alloy_draft_through_neutral_runtime() {
    let outcome = runtime()
        .execute(request("input.left + input.right"))
        .await
        .expect("execute Rhai");

    assert_eq!(outcome.output, json!(42));
    assert!(outcome.metrics.output_bytes.is_some());
}

#[tokio::test]
async fn maps_operation_pressure_to_common_limit_error() {
    let mut request = request("loop { }");
    request.policy.limits.instruction_budget = 100;

    let error = runtime().execute(request).await.expect_err("limit");

    assert!(matches!(
        error,
        SandboxError::LimitExceeded { ref resource, limit: 100 }
            if resource == "instructions"
    ));
}

