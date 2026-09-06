use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::Stream;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse, ExecutorRegistry,
    SandboxCancellation, SandboxError, SandboxExecutor, SandboxExecutorKind, SandboxHost,
    SandboxLimits, SandboxOutcome, SandboxRequest, SandboxResult, SandboxRuntime,
};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::SANDBOX_WORKER_PROTOCOL_REVISION;
use crate::proto::sandbox_worker_service_server::SandboxWorkerService;
use crate::proto::{
    CapabilityRequest, HostFrame, ReadinessRequest, ReadinessResponse, WorkerFrame,
    capability_result, host_frame, worker_frame,
};

#[async_trait]
pub trait SandboxWorkerReadiness: Send + Sync {
    async fn check_readiness(&self) -> Result<(), String>;

    /// Revalidates deployment isolation and proves that the request fits inside
    /// the process/container envelope before any guest code starts.
    async fn admit_limits(&self, limits: &SandboxLimits) -> Result<(), String>;
}

/// Worker-side streaming adapter. The process supplies exactly one neutral
/// executor and a deployment-backed readiness check; capability calls return
/// to the host stream instead of acquiring infrastructure clients here.
pub struct SandboxWorkerGrpcService {
    executor: Arc<dyn SandboxExecutor>,
    readiness: Arc<dyn SandboxWorkerReadiness>,
    execution_permit: Arc<tokio::sync::Semaphore>,
}

impl SandboxWorkerGrpcService {
    pub fn new(
        executor: Arc<dyn SandboxExecutor>,
        readiness: Arc<dyn SandboxWorkerReadiness>,
    ) -> Result<Self, String> {
        if executor.kind() != SandboxExecutorKind::Rhai {
            return Err("sandbox worker must be composed with the Rhai executor".to_string());
        }
        Ok(Self {
            executor,
            readiness,
            execution_permit: Arc::new(tokio::sync::Semaphore::new(1)),
        })
    }
}

#[tonic::async_trait]
impl SandboxWorkerService for SandboxWorkerGrpcService {
    type ExecuteStream = Pin<Box<dyn Stream<Item = Result<WorkerFrame, Status>> + Send + 'static>>;

    async fn get_readiness(
        &self,
        _request: Request<ReadinessRequest>,
    ) -> Result<Response<ReadinessResponse>, Status> {
        Ok(Response::new(ReadinessResponse {
            ready: self.readiness.check_readiness().await.is_ok(),
            executor: SandboxExecutorKind::Rhai.to_string(),
            protocol_revision: SANDBOX_WORKER_PROTOCOL_REVISION,
        }))
    }

    async fn execute(
        &self,
        request: Request<tonic::Streaming<HostFrame>>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        self.readiness.check_readiness().await.map_err(|_| {
            Status::failed_precondition("sandbox worker isolation policy is not ready")
        })?;
        let mut inbound = request.into_inner();
        let permit = Arc::clone(&self.execution_permit)
            .try_acquire_owned()
            .map_err(|_| {
                Status::resource_exhausted("sandbox worker is executing another request")
            })?;
        let first = inbound
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("sandbox request stream is empty"))?;
        validate_host_envelope(&first).map_err(Status::invalid_argument)?;
        let execution_id = first.execution_id.clone();
        let payload = match first.frame {
            Some(host_frame::Frame::RequestPayload(payload)) => payload,
            _ => {
                return Err(Status::invalid_argument(
                    "first sandbox host frame must contain a request",
                ));
            }
        };
        let mut sandbox_request: SandboxRequest = serde_json::from_slice(&payload.request_json)
            .map_err(|error| {
                Status::invalid_argument(format!("invalid sandbox request: {error}"))
            })?;
        if !sandbox_request.payload.bytes.is_empty() {
            return Err(Status::invalid_argument(
                "sandbox request metadata must not contain artifact bytes",
            ));
        }
        sandbox_request.payload.bytes = payload.artifact_bytes;
        sandbox_request
            .validate()
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        self.readiness
            .admit_limits(&sandbox_request.policy.limits)
            .await
            .map_err(|_| {
                Status::failed_precondition("sandbox request exceeds the worker isolation envelope")
            })?;
        if sandbox_request.payload.executor != SandboxExecutorKind::Rhai {
            return Err(Status::invalid_argument(
                "sandbox worker accepts only Rhai requests",
            ));
        }
        if sandbox_request.context.execution_id.to_string() != execution_id {
            return Err(Status::invalid_argument(
                "sandbox request execution identity does not match its frame",
            ));
        }

        let (outbound, output) = mpsc::channel(8);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let cancellation = SandboxCancellation::new();
        let reader = tokio::spawn(read_host_frames(
            inbound,
            execution_id.clone(),
            Arc::clone(&pending),
            cancellation.clone(),
        ));

        let executor = Arc::clone(&self.executor);
        tokio::spawn(async move {
            let _permit = permit;
            let callback = CallbackBroker::new(
                execution_id.clone(),
                outbound.clone(),
                pending,
                Duration::from_millis(sandbox_request.policy.limits.wall_clock_ms),
            );
            let result = execute_request(executor, sandbox_request, callback, cancellation).await;
            let terminal = terminal_frame(&execution_id, result);
            let _ = outbound.send(Ok(terminal)).await;
            reader.abort();
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(output))))
    }
}

type PendingCalls = Arc<Mutex<HashMap<u64, oneshot::Sender<SandboxResult<CapabilityResponse>>>>>;

struct CallbackBroker {
    execution_id: String,
    outbound: mpsc::Sender<Result<WorkerFrame, Status>>,
    pending: PendingCalls,
    next_call_id: AtomicU64,
    timeout: Duration,
}

impl CallbackBroker {
    fn new(
        execution_id: String,
        outbound: mpsc::Sender<Result<WorkerFrame, Status>>,
        pending: PendingCalls,
        timeout: Duration,
    ) -> Self {
        Self {
            execution_id,
            outbound,
            pending,
            next_call_id: AtomicU64::new(1),
            timeout,
        }
    }
}

#[async_trait]
impl CapabilityBroker for CallbackBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        let call_id = self.next_call_id.fetch_add(1, Ordering::Relaxed);
        if call_id == u64::MAX {
            return Err(SandboxError::LimitExceeded {
                resource: "worker_capability_call_id".to_string(),
                limit: u64::MAX - 1,
            });
        }
        let payload = serde_json::to_vec(call).map_err(|error| {
            SandboxError::Internal(format!("could not encode capability call: {error}"))
        })?;
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(call_id, sender);
        let send_result = self
            .outbound
            .send(Ok(WorkerFrame {
                protocol_revision: SANDBOX_WORKER_PROTOCOL_REVISION,
                execution_id: self.execution_id.clone(),
                frame: Some(worker_frame::Frame::CapabilityRequest(CapabilityRequest {
                    call_id,
                    call_payload: payload,
                })),
            }))
            .await;
        if send_result.is_err() {
            self.pending.lock().await.remove(&call_id);
            return Err(SandboxError::Aborted(
                "sandbox host stream closed during a capability call".to_string(),
            ));
        }
        match tokio::time::timeout(self.timeout, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(SandboxError::Aborted(
                "sandbox capability callback was abandoned".to_string(),
            )),
            Err(_) => {
                self.pending.lock().await.remove(&call_id);
                Err(SandboxError::Timeout {
                    limit_ms: self.timeout.as_millis().try_into().unwrap_or(u64::MAX),
                })
            }
        }
    }
}

#[derive(Clone)]
struct SharedExecutor(Arc<dyn SandboxExecutor>);

#[async_trait]
impl SandboxExecutor for SharedExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        self.0.kind()
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        self.0.execute(request, host).await
    }
}

async fn execute_request(
    executor: Arc<dyn SandboxExecutor>,
    request: SandboxRequest,
    callback: CallbackBroker,
    cancellation: SandboxCancellation,
) -> SandboxResult<SandboxOutcome> {
    let mut executors = ExecutorRegistry::new();
    executors.register_in_process(SharedExecutor(executor))?;
    SandboxRuntime::new(executors, Arc::new(callback))
        .execute_with_cancellation(request, cancellation)
        .await
}

async fn read_host_frames(
    mut inbound: tonic::Streaming<HostFrame>,
    execution_id: String,
    pending: PendingCalls,
    cancellation: SandboxCancellation,
) {
    loop {
        let frame = match inbound.message().await {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                fail_pending(
                    &pending,
                    SandboxError::Aborted("sandbox host closed its request stream".to_string()),
                )
                .await;
                cancellation.cancel();
                return;
            }
            Err(error) => {
                fail_pending(
                    &pending,
                    SandboxError::Aborted(format!("sandbox host request stream failed: {error}")),
                )
                .await;
                cancellation.cancel();
                return;
            }
        };
        if let Err(error) = validate_host_envelope_for_execution(&frame, &execution_id) {
            fail_pending(&pending, SandboxError::Aborted(error)).await;
            cancellation.cancel();
            return;
        }
        match frame.frame {
            Some(host_frame::Frame::CapabilityResult(result)) => {
                let outcome = match result.result {
                    Some(capability_result::Result::ResponsePayload(payload)) => {
                        serde_json::from_slice::<CapabilityResponse>(&payload).map_err(|error| {
                            SandboxError::Aborted(format!(
                                "invalid capability response payload: {error}"
                            ))
                        })
                    }
                    Some(capability_result::Result::ErrorPayload(payload)) => {
                        match serde_json::from_slice::<SandboxError>(&payload) {
                            Ok(error) => Err(error),
                            Err(error) => Err(SandboxError::Aborted(format!(
                                "invalid capability error payload: {error}"
                            ))),
                        }
                    }
                    None => Err(SandboxError::Aborted(
                        "capability result is empty".to_string(),
                    )),
                };
                let Some(sender) = pending.lock().await.remove(&result.call_id) else {
                    fail_pending(
                        &pending,
                        SandboxError::Aborted(
                            "sandbox host returned an unknown capability call id".to_string(),
                        ),
                    )
                    .await;
                    cancellation.cancel();
                    return;
                };
                let _ = sender.send(outcome);
            }
            Some(host_frame::Frame::CancelExecution(_)) => {
                fail_pending(&pending, SandboxError::Cancelled).await;
                cancellation.cancel();
                return;
            }
            Some(host_frame::Frame::RequestPayload(_)) | None => {
                fail_pending(
                    &pending,
                    SandboxError::Aborted(
                        "sandbox host sent an invalid post-start frame".to_string(),
                    ),
                )
                .await;
                cancellation.cancel();
                return;
            }
        }
    }
}

async fn fail_pending(pending: &PendingCalls, error: SandboxError) {
    let senders = pending
        .lock()
        .await
        .drain()
        .map(|(_, sender)| sender)
        .collect::<Vec<_>>();
    for sender in senders {
        let _ = sender.send(Err(error.clone()));
    }
}

fn validate_host_envelope(frame: &HostFrame) -> Result<(), String> {
    if frame.protocol_revision != SANDBOX_WORKER_PROTOCOL_REVISION {
        return Err("sandbox host protocol revision mismatch".to_string());
    }
    uuid::Uuid::parse_str(&frame.execution_id)
        .map_err(|_| "sandbox host execution identity must be a UUID".to_string())?;
    Ok(())
}

fn validate_host_envelope_for_execution(
    frame: &HostFrame,
    execution_id: &str,
) -> Result<(), String> {
    validate_host_envelope(frame)?;
    if frame.execution_id != execution_id {
        return Err("sandbox host frame execution identity mismatch".to_string());
    }
    Ok(())
}

fn terminal_frame(execution_id: &str, result: SandboxResult<SandboxOutcome>) -> WorkerFrame {
    let frame = match result {
        Ok(outcome) => match serde_json::to_vec(&outcome) {
            Ok(payload) => worker_frame::Frame::OutcomePayload(payload),
            Err(error) => worker_frame::Frame::ErrorPayload(encode_terminal_error(
                SandboxError::Internal(format!("could not encode sandbox outcome: {error}")),
            )),
        },
        Err(error) => worker_frame::Frame::ErrorPayload(encode_terminal_error(error)),
    };
    WorkerFrame {
        protocol_revision: SANDBOX_WORKER_PROTOCOL_REVISION,
        execution_id: execution_id.to_string(),
        frame: Some(frame),
    }
}

fn encode_terminal_error(error: SandboxError) -> Vec<u8> {
    serde_json::to_vec(&error).unwrap_or_else(|_| {
        // SandboxError is a repository-owned serializable enum. This fallback
        // remains a real terminal failure if serialization itself regresses.
        serde_json::to_vec(&SandboxError::Internal(
            "could not encode sandbox terminal error".to_string(),
        ))
        .expect("static SandboxError must serialize")
    })
}
