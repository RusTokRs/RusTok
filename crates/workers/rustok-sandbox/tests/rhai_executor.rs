#![cfg(feature = "rhai")]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustok_sandbox::rhai::RhaiExecutor;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    ExecutionPhase, ExecutorRegistry, RHAI_WORKSPACE_MEDIA_TYPE, RHAI_WORKSPACE_SCHEMA_VERSION,
    RhaiBindingInput, RhaiBindingOutput, RhaiCapabilityBridge, RhaiRecordInput, RhaiScopeInput,
    RhaiWorkspace, RhaiWorkspaceFile, RhaiWorkspaceFileKind, SandboxContext, SandboxError,
    SandboxExecutorKind, SandboxPayload, SandboxPolicy, SandboxRequest, SandboxResult,
    SandboxRuntime, SandboxSubject,
};
use serde_json::json;
use uuid::Uuid;

struct NoCapabilities;

#[derive(Default)]
struct CapturingBroker(Mutex<Vec<CapabilityCall>>);

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

#[async_trait]
impl CapabilityBroker for CapturingBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        self.0.lock().expect("capability calls").push(call.clone());
        Ok(CapabilityResponse {
            output: json!({ "ok": true, "status": 200, "body": { "source": "broker" } }),
        })
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
            runtime_abi: "rustok:module/runtime@1".to_string(),
            entrypoint: "main".to_string(),
            bytes: source.as_bytes().to_vec(),
        },
        input: serde_json::to_value(RhaiBindingInput::new(json!({ "left": 20, "right": 22 })))
            .expect("serialize Rhai binding"),
        rhai_scope: None,
        policy: SandboxPolicy::default(),
    }
}

fn runtime() -> SandboxRuntime {
    let mut executors = ExecutorRegistry::new();
    executors
        .register_in_process(RhaiExecutor::new())
        .expect("register Rhai executor");
    SandboxRuntime::new(executors, Arc::new(NoCapabilities))
}

fn capability_runtime(broker: Arc<CapturingBroker>) -> SandboxRuntime {
    let mut executors = ExecutorRegistry::new();
    executors
        .register_in_process(RhaiExecutor::new().with_extension(Arc::new(RhaiCapabilityBridge)))
        .expect("register Rhai executor");
    SandboxRuntime::new(executors, broker)
}

fn workspace_request(workspace: RhaiWorkspace) -> SandboxRequest {
    let mut request = request("");
    request.payload.media_type = RHAI_WORKSPACE_MEDIA_TYPE.to_string();
    request.payload.digest = workspace.digest().expect("workspace digest");
    request.payload.entrypoint = workspace.entrypoint.clone();
    request.payload.bytes = workspace.canonical_bytes().expect("workspace bytes");
    request
}

#[tokio::test]
async fn executes_alloy_draft_through_neutral_runtime() {
    let outcome = runtime()
        .execute(request("input.left + input.right"))
        .await
        .expect("execute Rhai");

    assert_eq!(
        RhaiBindingOutput::decode(outcome.output)
            .expect("versioned Rhai output")
            .output,
        json!(42)
    );
    assert!(outcome.metrics.output_bytes.is_some());
}

#[tokio::test]
async fn executes_canonical_workspace_with_in_memory_imports() {
    let workspace = RhaiWorkspace {
        schema_version: RHAI_WORKSPACE_SCHEMA_VERSION,
        entrypoint: "src/main.rhai".to_string(),
        files: vec![
            RhaiWorkspaceFile {
                path: "src/main.rhai".to_string(),
                kind: RhaiWorkspaceFileKind::Source,
                contents: "import \"src/math.rhai\" as math;\nmath::add(input.left, input.right)"
                    .to_string(),
            },
            RhaiWorkspaceFile {
                path: "src/math.rhai".to_string(),
                kind: RhaiWorkspaceFileKind::Source,
                contents: "fn add(left, right) { left + right }".to_string(),
            },
        ],
    };

    let outcome = runtime()
        .execute(workspace_request(workspace))
        .await
        .expect("execute workspace");

    assert_eq!(
        RhaiBindingOutput::decode(outcome.output)
            .expect("versioned Rhai output")
            .output,
        json!(42)
    );
}

#[tokio::test]
async fn returns_changes_from_serialized_mutable_records() {
    let mut request = workspace_request(RhaiWorkspace::single_source(
        "entity[\"status\"] = \"approved\"; params.amount",
    ));
    request.context.phase = ExecutionPhase::BeforeHook;
    request.rhai_scope = Some(RhaiScopeInput {
        constants: BTreeMap::from([("params".to_string(), json!({ "amount": 42 }))]),
        records: BTreeMap::from([(
            "entity".to_string(),
            RhaiRecordInput {
                id: "order-1".to_string(),
                record_type: "order".to_string(),
                fields: json!({ "status": "pending" }),
                mutable: true,
            },
        )]),
    });

    let outcome = runtime()
        .execute(request)
        .await
        .expect("execute scoped workspace");

    assert_eq!(
        RhaiBindingOutput::decode(outcome.output)
            .expect("versioned Rhai output")
            .output,
        json!(42)
    );
    assert_eq!(
        outcome
            .rhai_scope
            .expect("scope output")
            .record_changes
            .get("entity"),
        Some(&json!({ "status": "approved" }))
    );
}

#[tokio::test]
async fn rejects_mutation_of_serialized_immutable_records() {
    let mut request = workspace_request(RhaiWorkspace::single_source(
        "entity_before[\"status\"] = \"approved\"",
    ));
    request.rhai_scope = Some(RhaiScopeInput {
        constants: BTreeMap::new(),
        records: BTreeMap::from([(
            "entity_before".to_string(),
            RhaiRecordInput {
                id: "order-1".to_string(),
                record_type: "order".to_string(),
                fields: json!({ "status": "pending" }),
                mutable: false,
            },
        )]),
    });

    let error = runtime()
        .execute(request)
        .await
        .expect_err("immutable record mutation must fail");

    assert!(matches!(error, SandboxError::Trap(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn brokered_http_helper_is_grant_scoped_and_default_deny() {
    let broker = Arc::new(CapturingBroker::default());
    let runtime = capability_runtime(Arc::clone(&broker));
    let mut granted = request("http_get(\"https://service.example/allowed\")");
    granted.policy.grants.push(CapabilityGrant {
        name: CapabilityName::new("platform.http").expect("capability name"),
        constraints: json!({
            "hosts": ["service.example"],
            "methods": ["GET"],
            "path_prefixes": ["/allowed"],
        }),
    });

    let outcome = runtime.execute(granted).await.expect("granted HTTP helper");
    assert_eq!(
        RhaiBindingOutput::decode(outcome.output)
            .expect("versioned output")
            .output["status"],
        200
    );
    assert_eq!(broker.0.lock().expect("capability calls").len(), 1);

    let denied = runtime
        .execute(request("http_get(\"https://service.example/allowed\")"))
        .await
        .expect("helper returns typed denial");
    assert_eq!(
        RhaiBindingOutput::decode(denied.output)
            .expect("versioned output")
            .output["error_code"],
        "CAPABILITY_DENIED"
    );
    assert_eq!(broker.0.lock().expect("capability calls").len(), 1);
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

#[tokio::test]
async fn maps_elapsed_deadline_to_common_timeout_error() {
    let mut request = request("loop { }");
    request.policy.limits.wall_clock_ms = 0;

    let error = runtime().execute(request).await.expect_err("deadline");

    assert_eq!(error, SandboxError::Timeout { limit_ms: 0 });
}
