use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    ExecutionObserver, ExecutionPhase, ExecutionRecord, ExecutionStatus, ExecutorRegistry,
    SandboxContext, SandboxError, SandboxExecutor, SandboxExecutorKind, SandboxHost, SandboxOutcome,
    SandboxPayload, SandboxPolicy, SandboxRequest, SandboxResult, SandboxRuntime, SandboxSubject,
};
use serde_json::json;
use uuid::Uuid;

struct TestBroker;

#[async_trait]
impl CapabilityBroker for TestBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        Ok(CapabilityResponse {
            output: json!({ "operation": call.operation }),
        })
    }
}

struct RhaiFixtureExecutor;

#[async_trait]
impl SandboxExecutor for RhaiFixtureExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        let call = CapabilityCall {
            execution_id: request.context.execution_id,
            subject: request.subject.clone(),
            capability: CapabilityName::new("platform.events")?,
            operation: "publish".to_string(),
            input: request.input.clone(),
        };
        let response = host.invoke(&call).await?;
        Ok(SandboxOutcome {
            execution_id: Uuid::nil(),
            output: response.output,
            metrics: Default::default(),
        })
    }
}

#[derive(Default)]
struct Records(Mutex<Vec<ExecutionRecord>>);

#[async_trait]
impl ExecutionObserver for Records {
    async fn observe(&self, record: &ExecutionRecord) {
        self.0.lock().expect("records lock").push(record.clone());
    }
}

fn request(granted: bool) -> SandboxRequest {
    let capability = CapabilityName::new("platform.events").expect("valid capability");
    SandboxRequest {
        subject: SandboxSubject::AlloyDraft {
            draft_id: Uuid::new_v4(),
            revision: 3,
        },
        context: SandboxContext::new(ExecutionPhase::Test),
        payload: SandboxPayload {
            executor: SandboxExecutorKind::Rhai,
            media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            digest: "sha256:fixture".to_string(),
            entrypoint: "main".to_string(),
            bytes: b"42".to_vec(),
        },
        input: json!({ "value": 42 }),
        policy: SandboxPolicy {
            grants: granted
                .then_some(CapabilityGrant {
                    name: capability,
                    constraints: json!({}),
                })
                .into_iter()
                .collect(),
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn runtime_uses_default_deny_capability_policy() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(RhaiFixtureExecutor)
        .expect("register fixture executor");
    let runtime = SandboxRuntime::new(registry, Arc::new(TestBroker));

    let error = runtime.execute(request(false)).await.expect_err("denied");

    assert!(matches!(error, SandboxError::CapabilityDenied(_)));
    assert_eq!(error.code(), "CAPABILITY_DENIED");
}

#[tokio::test]
async fn alloy_draft_and_module_artifact_share_execution_contract() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(RhaiFixtureExecutor)
        .expect("register fixture executor");
    let records = Arc::new(Records::default());
    let observer: Arc<dyn ExecutionObserver> = records.clone();
    let runtime = SandboxRuntime::new(registry, Arc::new(TestBroker)).with_observer(observer);

    let draft = request(true);
    let draft_execution_id = draft.context.execution_id;
    runtime.execute(draft).await.expect("draft execution");

    let mut installed = request(true);
    installed.subject = SandboxSubject::ModuleArtifact {
        slug: "example".to_string(),
        version: "1.0.0".to_string(),
        digest: "sha256:release".to_string(),
    };
    let installed_execution_id = installed.context.execution_id;
    runtime.execute(installed).await.expect("installed execution");

    let records = records.0.lock().expect("records lock");
    assert_eq!(records.len(), 4);
    assert_eq!(records[0].execution_id, draft_execution_id);
    assert_eq!(records[1].status, ExecutionStatus::Succeeded);
    assert_eq!(records[2].execution_id, installed_execution_id);
    assert_eq!(records[3].status, ExecutionStatus::Succeeded);
}

#[test]
fn executor_registration_is_unique_by_kind() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(RhaiFixtureExecutor)
        .expect("first executor");
    let error = registry
        .register(RhaiFixtureExecutor)
        .expect_err("duplicate executor");

    assert_eq!(
        error,
        SandboxError::ExecutorAlreadyRegistered(SandboxExecutorKind::Rhai)
    );
}

#[test]
fn capability_names_cannot_bypass_validation_through_deserialization() {
    let parsed = serde_json::from_str::<CapabilityName>("\"Platform events\"");

    assert!(parsed.is_err());
}
