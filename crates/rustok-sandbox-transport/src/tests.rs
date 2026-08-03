use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityCallContext, CapabilityGrant, CapabilityName,
    CapabilityResponse, ExecutionMetrics, ExecutionPhase, ExecutorRegistry, RhaiScopeInput,
    RhaiScopeOutput, SandboxCancellation, SandboxContext, SandboxError, SandboxExecutor,
    SandboxExecutorKind, SandboxExecutorPlacement, SandboxHost, SandboxOutcome, SandboxPayload,
    SandboxPolicy, SandboxRequest, SandboxResult, SandboxRuntime, SandboxSubject,
};
use serde_json::json;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::Request;
use tonic::transport::{Channel, Endpoint, Server};

use crate::client::GrpcRhaiExecutor;
use crate::proto::sandbox_worker_service_client::SandboxWorkerServiceClient;
use crate::proto::sandbox_worker_service_server::SandboxWorkerServiceServer;
use crate::proto::{HostFrame, SandboxRequestPayload, host_frame};
use crate::server::{SandboxWorkerGrpcService, SandboxWorkerReadiness};
use crate::{SANDBOX_WORKER_MAX_MESSAGE_SIZE, SANDBOX_WORKER_PROTOCOL_REVISION};

struct EchoExecutor;

#[async_trait]
impl SandboxExecutor for EchoExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        if request
            .rhai_scope
            .as_ref()
            .and_then(|scope| scope.constants.get("params"))
            != Some(&json!({ "transport": true }))
        {
            return Err(SandboxError::InvalidRequest(
                "serialized Rhai scope did not reach the worker".to_string(),
            ));
        }
        let response = host
            .invoke(&CapabilityCall {
                execution_id: request.context.execution_id,
                subject: request.subject.clone(),
                context: CapabilityCallContext::from(&request.context),
                capability: CapabilityName::new("test.echo")?,
                operation: "invoke".to_string(),
                input: request.input.clone(),
            })
            .await?;
        Ok(SandboxOutcome {
            execution_id: request.context.execution_id,
            output: response.output,
            rhai_scope: Some(RhaiScopeOutput {
                record_changes: BTreeMap::from([(
                    "entity".to_string(),
                    json!({ "status": "remote" }),
                )]),
            }),
            metrics: ExecutionMetrics::default(),
        })
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
        _request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        loop {
            if host.cancellation().is_cancelled() {
                return Err(SandboxError::Cancelled);
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
}

struct EchoBroker;

#[async_trait]
impl CapabilityBroker for EchoBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        Ok(CapabilityResponse {
            output: call.input.clone(),
        })
    }
}

struct DenyBroker;

#[async_trait]
impl CapabilityBroker for DenyBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        Err(SandboxError::CapabilityDenied(call.capability.clone()))
    }
}

struct Readiness(AtomicBool);

#[async_trait]
impl SandboxWorkerReadiness for Readiness {
    async fn check_readiness(&self) -> Result<(), String> {
        if self.0.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err("fixture is not ready".to_string())
        }
    }

    async fn admit_limits(&self, limits: &rustok_sandbox::SandboxLimits) -> Result<(), String> {
        self.check_readiness().await?;
        if limits.wall_clock_ms > 5_000
            || limits.max_memory_bytes > 128 * 1024 * 1024
            || limits.max_output_bytes > 8 * 1024 * 1024
            || limits.max_concurrency != 1
        {
            return Err("fixture isolation envelope exceeded".to_string());
        }
        Ok(())
    }
}

async fn start_worker(
    executor: Arc<dyn SandboxExecutor>,
) -> (Channel, tokio::task::JoinHandle<()>) {
    start_worker_with_readiness(executor, true).await
}

async fn start_worker_with_readiness(
    executor: Arc<dyn SandboxExecutor>,
    ready: bool,
) -> (Channel, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind worker listener");
    let address = listener.local_addr().expect("worker address");
    let service =
        SandboxWorkerGrpcService::new(executor, Arc::new(Readiness(AtomicBool::new(ready))))
            .expect("worker service");
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(
                SandboxWorkerServiceServer::new(service)
                    .max_decoding_message_size(SANDBOX_WORKER_MAX_MESSAGE_SIZE)
                    .max_encoding_message_size(SANDBOX_WORKER_MAX_MESSAGE_SIZE),
            )
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .expect("serve worker");
    });
    let channel = Endpoint::from_shared(format!("http://{address}"))
        .expect("worker endpoint")
        .connect()
        .await
        .expect("connect worker");
    (channel, server)
}

fn request() -> SandboxRequest {
    let mut context = SandboxContext::new(ExecutionPhase::Manual);
    context.tenant_id = Some(uuid::Uuid::new_v4());
    let mut policy = SandboxPolicy {
        grants: vec![CapabilityGrant {
            name: CapabilityName::new("test.echo").expect("capability"),
            constraints: json!({}),
        }],
        ..Default::default()
    };
    policy.limits.wall_clock_ms = 2_000;
    SandboxRequest {
        subject: SandboxSubject::ModuleArtifact {
            installation_id: uuid::Uuid::new_v4(),
            slug: "fixture".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        },
        context,
        payload: SandboxPayload {
            executor: SandboxExecutorKind::Rhai,
            media_type: "application/vnd.rustok.rhai".to_string(),
            digest: format!("sha256:{}", "b".repeat(64)),
            runtime_abi: "rustok:module/runtime@1".to_string(),
            entrypoint: "main".to_string(),
            bytes: b"fixture".to_vec(),
        },
        input: json!({ "value": 42 }),
        rhai_scope: Some(RhaiScopeInput {
            constants: BTreeMap::from([("params".to_string(), json!({ "transport": true }))]),
            records: BTreeMap::new(),
        }),
        policy,
    }
}

#[tokio::test]
async fn isolated_executor_round_trips_capabilities_through_the_host_broker() {
    let (channel, server) = start_worker(Arc::new(EchoExecutor)).await;
    let executor = GrpcRhaiExecutor::from_channel(channel);
    executor.check_readiness().await.expect("worker readiness");
    let mut executors = ExecutorRegistry::new();
    executors
        .register_isolated_worker(executor)
        .expect("register worker");
    assert_eq!(
        executors
            .placement(SandboxExecutorKind::Rhai)
            .expect("executor placement"),
        SandboxExecutorPlacement::IsolatedWorker
    );
    let outcome = SandboxRuntime::new(executors, Arc::new(EchoBroker))
        .execute(request())
        .await
        .expect("remote execution");
    assert_eq!(outcome.output, json!({ "value": 42 }));
    assert_eq!(
        outcome
            .rhai_scope
            .expect("scope output")
            .record_changes
            .get("entity"),
        Some(&json!({ "status": "remote" }))
    );
    assert_eq!(outcome.metrics.capability_calls, 1);
    server.abort();
}

#[tokio::test]
async fn typed_capability_errors_round_trip_without_string_fallbacks() {
    let (channel, server) = start_worker(Arc::new(EchoExecutor)).await;
    let mut executors = ExecutorRegistry::new();
    executors
        .register_isolated_worker(GrpcRhaiExecutor::from_channel(channel))
        .expect("register worker");
    let error = SandboxRuntime::new(executors, Arc::new(DenyBroker))
        .execute(request())
        .await
        .expect_err("capability must be denied");
    assert_eq!(
        error,
        SandboxError::CapabilityDenied(CapabilityName::new("test.echo").expect("capability"))
    );
    server.abort();
}

#[tokio::test]
async fn host_cancellation_terminates_a_remote_execution() {
    let (channel, server) = start_worker(Arc::new(CancellationExecutor)).await;
    let mut executors = ExecutorRegistry::new();
    executors
        .register_isolated_worker(GrpcRhaiExecutor::from_channel(channel))
        .expect("register worker");
    let runtime = SandboxRuntime::new(executors, Arc::new(EchoBroker));
    let cancellation = SandboxCancellation::new();
    let cancellation_trigger = cancellation.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        cancellation_trigger.cancel();
    });
    let error = runtime
        .execute_with_cancellation(request(), cancellation)
        .await
        .expect_err("execution must be cancelled");
    assert_eq!(error, SandboxError::Cancelled);
    server.abort();
}

#[tokio::test]
async fn worker_rejects_a_mismatched_protocol_revision_before_execution() {
    let (channel, server) = start_worker(Arc::new(EchoExecutor)).await;
    let execution_id = uuid::Uuid::new_v4().to_string();
    let frames = ReceiverStream::new({
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        sender
            .send(HostFrame {
                protocol_revision: SANDBOX_WORKER_PROTOCOL_REVISION + 1,
                execution_id,
                frame: Some(host_frame::Frame::RequestPayload(SandboxRequestPayload {
                    request_json: Vec::new(),
                    artifact_bytes: Vec::new(),
                })),
            })
            .await
            .expect("send frame");
        receiver
    });
    let error = SandboxWorkerServiceClient::new(channel)
        .execute(Request::new(frames))
        .await
        .expect_err("revision mismatch must fail");
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    server.abort();
}

#[tokio::test]
async fn remote_hang_is_bounded_by_the_sandbox_deadline() {
    let (channel, server) = start_worker(Arc::new(CancellationExecutor)).await;
    let mut executors = ExecutorRegistry::new();
    executors
        .register_isolated_worker(GrpcRhaiExecutor::from_channel(channel))
        .expect("register worker");
    let mut sandbox_request = request();
    sandbox_request.policy.limits.wall_clock_ms = 50;
    let error = SandboxRuntime::new(executors, Arc::new(EchoBroker))
        .execute(sandbox_request)
        .await
        .expect_err("hung execution must time out");
    assert_eq!(error, SandboxError::Timeout { limit_ms: 50 });
    server.abort();
}

#[tokio::test]
async fn worker_disconnect_aborts_without_an_in_process_fallback() {
    let (channel, server) = start_worker(Arc::new(CancellationExecutor)).await;
    let mut executors = ExecutorRegistry::new();
    executors
        .register_isolated_worker(GrpcRhaiExecutor::from_channel(channel))
        .expect("register worker");
    server.abort();
    let _ = server.await;
    let error = SandboxRuntime::new(executors, Arc::new(EchoBroker))
        .execute(request())
        .await
        .expect_err("disconnected worker must abort");
    assert!(matches!(
        error,
        SandboxError::Aborted(_) | SandboxError::Timeout { .. }
    ));
}

#[tokio::test]
async fn worker_refuses_execution_when_isolation_readiness_is_lost() {
    let (channel, server) = start_worker_with_readiness(Arc::new(EchoExecutor), false).await;
    let execution_id = uuid::Uuid::new_v4().to_string();
    let frames = ReceiverStream::new({
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        sender
            .send(HostFrame {
                protocol_revision: SANDBOX_WORKER_PROTOCOL_REVISION,
                execution_id,
                frame: Some(host_frame::Frame::RequestPayload(SandboxRequestPayload {
                    request_json: Vec::new(),
                    artifact_bytes: Vec::new(),
                })),
            })
            .await
            .expect("send frame");
        receiver
    });
    let error = SandboxWorkerServiceClient::new(channel)
        .execute(Request::new(frames))
        .await
        .expect_err("unready isolation must fail");
    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
    server.abort();
}
