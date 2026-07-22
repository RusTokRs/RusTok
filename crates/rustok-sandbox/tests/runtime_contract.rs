use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustok_sandbox::{
    CapabilityAuditOutcome, CapabilityAuditRecord, CapabilityBroker, CapabilityCall,
    CapabilityCallContext, CapabilityGrant, CapabilityName, CapabilityObserver, CapabilityResponse,
    ExecutionObserver, ExecutionPhase, ExecutionRecord, ExecutionStatus, ExecutorRegistry,
    SandboxContext, SandboxError, SandboxExecutor, SandboxExecutorKind, SandboxHost,
    SandboxOutcome, SandboxPayload, SandboxPolicy, SandboxRequest, SandboxResult, SandboxRuntime,
    SandboxSubject,
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
            context: CapabilityCallContext::from(&request.context),
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

struct ContextMismatchExecutor;

#[async_trait]
impl SandboxExecutor for ContextMismatchExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        let mut context = CapabilityCallContext::from(&request.context);
        context.actor_id = Some("other-actor".to_string());
        let call = CapabilityCall {
            execution_id: request.context.execution_id,
            subject: request.subject.clone(),
            context,
            capability: CapabilityName::new("platform.events")?,
            operation: "publish".to_string(),
            input: request.input.clone(),
        };
        host.invoke(&call).await?;
        unreachable!("context mismatch must be rejected before the broker")
    }
}

struct DoubleCapabilityExecutor;

#[async_trait]
impl SandboxExecutor for DoubleCapabilityExecutor {
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
            context: CapabilityCallContext::from(&request.context),
            capability: CapabilityName::new("platform.events")?,
            operation: "publish".to_string(),
            input: request.input.clone(),
        };
        host.invoke(&call).await?;
        host.invoke(&call).await?;
        unreachable!("the second capability call must exceed the test budget")
    }
}

struct CancellationExecutor;

#[async_trait]
impl SandboxExecutor for CancellationExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        host.cancellation().cancel();
        let call = CapabilityCall {
            execution_id: request.context.execution_id,
            subject: request.subject.clone(),
            context: CapabilityCallContext::from(&request.context),
            capability: CapabilityName::new("platform.events")?,
            operation: "publish".to_string(),
            input: request.input.clone(),
        };
        host.invoke(&call).await?;
        unreachable!("cancelled execution must not invoke a capability")
    }
}

struct SecretFailExecutor;

#[async_trait]
impl SandboxExecutor for SecretFailExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        _request: &SandboxRequest,
        _host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        Err(SandboxError::Trap(
            "token=must-not-appear-in-audit".to_string(),
        ))
    }
}

struct PanicBroker;

#[async_trait]
impl CapabilityBroker for PanicBroker {
    async fn invoke(
        &self,
        _call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        panic!("rejected capability call must not reach the broker")
    }
}

#[derive(Default)]
struct CountingBroker(Mutex<u32>);

#[async_trait]
impl CapabilityBroker for CountingBroker {
    async fn invoke(
        &self,
        _call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        *self.0.lock().expect("call count lock") += 1;
        Ok(CapabilityResponse {
            output: serde_json::Value::Null,
        })
    }
}

#[derive(Default)]
struct Records(Mutex<Vec<ExecutionRecord>>);

#[async_trait]
impl ExecutionObserver for Records {
    async fn observe(&self, record: &ExecutionRecord) -> SandboxResult<()> {
        self.0.lock().expect("records lock").push(record.clone());
        Ok(())
    }
}

#[derive(Default)]
struct CapabilityRecords(Mutex<Vec<CapabilityAuditRecord>>);

#[async_trait]
impl CapabilityObserver for CapabilityRecords {
    async fn observe(&self, record: &CapabilityAuditRecord) {
        self.0
            .lock()
            .expect("capability records lock")
            .push(record.clone());
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
            runtime_abi: "rustok:module/runtime@1".to_string(),
            entrypoint: "main".to_string(),
            bytes: b"42".to_vec(),
        },
        input: json!({ "topic": "sandbox.fixture", "payload": { "value": 42 } }),
        policy: SandboxPolicy {
            grants: granted
                .then_some(CapabilityGrant {
                    name: capability,
                    constraints: json!({
                        "topics": ["sandbox.fixture"],
                        "operations": ["publish"]
                    }),
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
async fn runtime_rejects_a_capability_call_with_another_actor_before_the_broker() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(ContextMismatchExecutor)
        .expect("register fixture executor");
    let runtime = SandboxRuntime::new(registry, Arc::new(PanicBroker));

    let error = runtime
        .execute(request(true))
        .await
        .expect_err("context mismatch");

    assert_eq!(
        error,
        SandboxError::CapabilityContextMismatch { field: "context" }
    );
    assert_eq!(error.code(), "CAPABILITY_CONTEXT_MISMATCH");
}

#[tokio::test]
async fn denied_capability_call_emits_redacted_audit_evidence() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(ContextMismatchExecutor)
        .expect("register fixture executor");
    let records = Arc::new(CapabilityRecords::default());
    let observer: Arc<dyn CapabilityObserver> = records.clone();
    let runtime =
        SandboxRuntime::new(registry, Arc::new(PanicBroker)).with_capability_observer(observer);
    let mut request = request(true);
    request.input = json!({ "secret": "must-not-appear-in-audit" });

    runtime
        .execute(request)
        .await
        .expect_err("context mismatch");

    let records = records.0.lock().expect("capability records lock");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, CapabilityAuditOutcome::Denied);
    assert_eq!(
        records[0].error_code.as_deref(),
        Some("CAPABILITY_CONTEXT_MISMATCH")
    );
    assert!(
        !serde_json::to_string(&records[0])
            .expect("serialize audit record")
            .contains("must-not-appear-in-audit")
    );
}

#[tokio::test]
async fn cancellation_stops_capability_dispatch_before_the_broker() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(CancellationExecutor)
        .expect("register fixture executor");
    let runtime = SandboxRuntime::new(registry, Arc::new(PanicBroker));

    let error = runtime
        .execute(request(true))
        .await
        .expect_err("cancelled execution");

    assert_eq!(error, SandboxError::Cancelled);
    assert_eq!(error.code(), "EXECUTION_CANCELLED");
}

#[tokio::test]
async fn execution_audit_excludes_untrusted_error_text() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(SecretFailExecutor)
        .expect("register fixture executor");
    let records = Arc::new(Records::default());
    let observer: Arc<dyn ExecutionObserver> = records.clone();
    let runtime = SandboxRuntime::new(registry, Arc::new(PanicBroker)).with_observer(observer);

    runtime
        .execute(request(true))
        .await
        .expect_err("fixture failure");

    let records = records.0.lock().expect("records lock");
    let failed = records.last().expect("failed record");
    assert_eq!(failed.status, ExecutionStatus::Failed);
    assert_eq!(failed.error_code.as_deref(), Some("EXECUTION_TRAPPED"));
    let metrics = failed.metrics.as_ref().expect("terminal failure metrics");
    assert_eq!(metrics.capability_calls, 0);
    assert!(
        !serde_json::to_string(failed)
            .expect("serialize audit record")
            .contains("must-not-appear-in-audit")
    );
}

#[tokio::test]
async fn runtime_records_queue_execution_and_capability_metrics() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(RhaiFixtureExecutor)
        .expect("register fixture executor");
    let records = Arc::new(Records::default());
    let observer: Arc<dyn ExecutionObserver> = records.clone();
    let runtime = SandboxRuntime::new(registry, Arc::new(TestBroker)).with_observer(observer);

    let outcome = runtime.execute(request(true)).await.expect("execution");

    assert_eq!(outcome.metrics.capability_calls, 1);
    let records = records.0.lock().expect("records lock");
    let succeeded = records.last().expect("success record");
    assert_eq!(succeeded.status, ExecutionStatus::Succeeded);
    assert_eq!(
        succeeded
            .metrics
            .as_ref()
            .expect("success metrics")
            .capability_calls,
        1
    );
}

#[tokio::test]
async fn runtime_enforces_capability_call_count_before_the_broker() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(DoubleCapabilityExecutor)
        .expect("register fixture executor");
    let broker = Arc::new(CountingBroker::default());
    let runtime = SandboxRuntime::new(registry, broker.clone());
    let mut constrained = request(true);
    constrained.policy.limits.max_capability_calls = 1;

    let error = runtime
        .execute(constrained)
        .await
        .expect_err("second capability call exceeds the budget");

    assert_eq!(
        error,
        SandboxError::LimitExceeded {
            resource: "capability_calls".to_string(),
            limit: 1,
        }
    );
    assert_eq!(*broker.0.lock().expect("call count lock"), 1);
}

#[tokio::test]
async fn runtime_enforces_capability_rate_before_the_broker() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(DoubleCapabilityExecutor)
        .expect("register fixture executor");
    let broker = Arc::new(CountingBroker::default());
    let runtime = SandboxRuntime::new(registry, broker.clone());
    let mut constrained = request(true);
    constrained.policy.limits.max_capability_calls_per_second = 1;

    let error = runtime
        .execute(constrained)
        .await
        .expect_err("second capability call exceeds the rate budget");

    assert_eq!(
        error,
        SandboxError::LimitExceeded {
            resource: "capability_calls_per_second".to_string(),
            limit: 1,
        }
    );
    assert_eq!(*broker.0.lock().expect("call count lock"), 1);
}

#[tokio::test]
async fn runtime_rejects_oversized_capability_input_before_the_broker() {
    let mut registry = ExecutorRegistry::new();
    registry
        .register(RhaiFixtureExecutor)
        .expect("register fixture executor");
    let runtime = SandboxRuntime::new(registry, Arc::new(PanicBroker));
    let mut constrained = request(true);
    constrained.policy.limits.max_capability_input_bytes = 1;

    let error = runtime
        .execute(constrained)
        .await
        .expect_err("capability input exceeds the budget");

    assert_eq!(
        error,
        SandboxError::LimitExceeded {
            resource: "capability_input_bytes".to_string(),
            limit: 1,
        }
    );
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
        installation_id: Uuid::new_v4(),
        slug: "example".to_string(),
        version: "1.0.0".to_string(),
        digest: "sha256:release".to_string(),
    };
    let installed_execution_id = installed.context.execution_id;
    runtime
        .execute(installed)
        .await
        .expect("installed execution");

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
