use std::time::Duration;

use async_trait::async_trait;
use rustok_sandbox::{
    CapabilityCall, SandboxCancellation, SandboxError, SandboxExecutor, SandboxExecutorKind,
    SandboxHost, SandboxOutcome, SandboxRequest, SandboxResult,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

use crate::proto::sandbox_worker_service_client::SandboxWorkerServiceClient;
use crate::proto::{
    CancelExecution, CapabilityRequest, CapabilityResult, HostFrame, ReadinessRequest,
    SandboxRequestPayload, WorkerFrame, capability_result, host_frame, worker_frame,
};
use crate::{SANDBOX_WORKER_MAX_MESSAGE_SIZE, SANDBOX_WORKER_PROTOCOL_REVISION};

/// Host-side Rhai adapter for the separately deployed worker. The only public
/// constructor requires TLS; transport failure never selects an in-process
/// executor.
#[derive(Clone)]
pub struct GrpcRhaiExecutor {
    client: SandboxWorkerServiceClient<Channel>,
}

impl GrpcRhaiExecutor {
    pub(crate) fn from_channel(channel: Channel) -> Self {
        Self {
            client: SandboxWorkerServiceClient::new(channel)
                .max_decoding_message_size(SANDBOX_WORKER_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(SANDBOX_WORKER_MAX_MESSAGE_SIZE),
        }
    }

    pub async fn connect_with_tls(
        endpoint: Endpoint,
        tls_config: ClientTlsConfig,
    ) -> Result<Self, String> {
        let channel = endpoint
            .tls_config(tls_config)
            .map_err(|error| error.to_string())?
            .connect()
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self::from_channel(channel))
    }

    /// Verifies authenticated reachability and the exact executor/protocol
    /// contract before the server publishes runtime readiness.
    pub async fn check_readiness(&self) -> Result<(), String> {
        let response = self
            .client
            .clone()
            .get_readiness(Request::new(ReadinessRequest {}))
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        if !response.ready {
            return Err("sandbox worker reported not ready".to_string());
        }
        if response.executor != SandboxExecutorKind::Rhai.to_string() {
            return Err(format!(
                "sandbox worker executor mismatch: expected rhai, received {}",
                response.executor
            ));
        }
        if response.protocol_revision != SANDBOX_WORKER_PROTOCOL_REVISION {
            return Err(format!(
                "sandbox worker protocol revision mismatch: expected {}, received {}",
                SANDBOX_WORKER_PROTOCOL_REVISION, response.protocol_revision
            ));
        }
        Ok(())
    }

    async fn execute_session(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
        outbound: mpsc::Sender<HostFrame>,
        inbound: mpsc::Receiver<HostFrame>,
    ) -> SandboxResult<SandboxOutcome> {
        let execution_id = request.context.execution_id.to_string();
        let (mut stream, mut cancellation_poll) = self.open_worker_stream(inbound).await?;
        let cancellation = host.cancellation();

        loop {
            let frame = Self::wait_for_worker_frame(
                &mut stream,
                &cancellation,
                &mut cancellation_poll,
                &outbound,
                &execution_id,
            )
            .await?;
            let frame_content = frame.frame.ok_or_else(|| {
                SandboxError::Aborted("sandbox worker sent an empty frame".to_string())
            })?;
            if let Some(outcome) = Self::process_worker_frame(
                frame_content,
                &host,
                &outbound,
                &execution_id,
                request.context.execution_id,
            )
            .await?
            {
                return Ok(outcome);
            }
        }
    }

    async fn open_worker_stream(
        &self,
        inbound: mpsc::Receiver<HostFrame>,
    ) -> SandboxResult<(tonic::Streaming<WorkerFrame>, tokio::time::Interval)> {
        let response = self
            .client
            .clone()
            .execute(Request::new(ReceiverStream::new(inbound)))
            .await
            .map_err(transport_error)?;
        let stream = response.into_inner();
        let mut cancellation_poll = tokio::time::interval(Duration::from_millis(5));
        cancellation_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        Ok((stream, cancellation_poll))
    }

    async fn process_worker_frame(
        frame_content: worker_frame::Frame,
        host: &SandboxHost,
        outbound: &mpsc::Sender<HostFrame>,
        execution_id: &str,
        expected_execution_id: uuid::Uuid,
    ) -> SandboxResult<Option<SandboxOutcome>> {
        match frame_content {
            worker_frame::Frame::CapabilityRequest(capability) => {
                Self::handle_capability_request(host, outbound, execution_id, capability).await?;
                Ok(None)
            }
            worker_frame::Frame::OutcomePayload(payload) => {
                let outcome: SandboxOutcome = decode(&payload, "sandbox outcome")?;
                if outcome.execution_id != expected_execution_id {
                    return Err(SandboxError::Aborted(
                        "sandbox worker outcome execution identity mismatch".to_string(),
                    ));
                }
                Ok(Some(outcome))
            }
            worker_frame::Frame::ErrorPayload(payload) => Err(decode(&payload, "sandbox error")?),
        }
    }

    async fn wait_for_worker_frame(
        stream: &mut tonic::Streaming<WorkerFrame>,
        cancellation: &SandboxCancellation,
        cancellation_poll: &mut tokio::time::Interval,
        outbound: &mpsc::Sender<HostFrame>,
        execution_id: &str,
    ) -> SandboxResult<WorkerFrame> {
        let next = loop {
            tokio::select! {
                _ = cancellation_poll.tick() => {
                    if cancellation.is_cancelled() {
                        let _ = outbound.try_send(cancel_frame(execution_id));
                        return Err(SandboxError::Cancelled);
                    }
                }
                next = stream.message() => break next.map_err(transport_error)?,
            }
        };
        let frame = next.ok_or_else(|| {
            SandboxError::Aborted(
                "sandbox worker closed the stream before a terminal frame".to_string(),
            )
        })?;
        validate_worker_frame(&frame, execution_id)?;
        Ok(frame)
    }

    async fn handle_capability_request(
        host: &SandboxHost,
        outbound: &mpsc::Sender<HostFrame>,
        execution_id: &str,
        capability: CapabilityRequest,
    ) -> SandboxResult<()> {
        let call: CapabilityCall = decode(&capability.call_payload, "capability call")?;
        let result = host.invoke(&call).await;
        let result = match result {
            Ok(response) => capability_result::Result::ResponsePayload(encode(
                &response,
                "capability response",
            )?),
            Err(error) => {
                capability_result::Result::ErrorPayload(encode(&error, "capability error")?)
            }
        };
        outbound
            .send(HostFrame {
                protocol_revision: SANDBOX_WORKER_PROTOCOL_REVISION,
                execution_id: execution_id.to_string(),
                frame: Some(host_frame::Frame::CapabilityResult(CapabilityResult {
                    call_id: capability.call_id,
                    result: Some(result),
                })),
            })
            .await
            .map_err(|_| {
                SandboxError::Aborted(
                    "sandbox worker request stream closed during a capability call".to_string(),
                )
            })
    }
}

#[async_trait]
impl SandboxExecutor for GrpcRhaiExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        let request_payload = validate_and_prepare_request(request)?;
        let execution_id = request.context.execution_id.to_string();
        let cancellation = host.cancellation();
        let (outbound, inbound) = mpsc::channel(8);
        send_initial_request(&outbound, &execution_id, request_payload).await?;

        let deadline_ms = request.policy.limits.wall_clock_ms;
        match tokio::time::timeout(
            Duration::from_millis(deadline_ms),
            self.execute_session(request, host, outbound.clone(), inbound),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                cancellation.cancel();
                let _ = outbound.try_send(cancel_frame(&execution_id));
                Err(SandboxError::Timeout {
                    limit_ms: deadline_ms,
                })
            }
        }
    }
}

fn validate_and_prepare_request(request: &SandboxRequest) -> SandboxResult<SandboxRequestPayload> {
    if request.payload.executor != SandboxExecutorKind::Rhai {
        return Err(SandboxError::InvalidRequest(
            "Rhai worker received a non-Rhai sandbox request".to_string(),
        ));
    }
    let request_payload = encode_request(request)?;
    let request_size = request_payload
        .request_json
        .len()
        .saturating_add(request_payload.artifact_bytes.len());
    if request_size > SANDBOX_WORKER_MAX_MESSAGE_SIZE {
        return Err(SandboxError::LimitExceeded {
            resource: "worker_request_bytes".to_string(),
            limit: SANDBOX_WORKER_MAX_MESSAGE_SIZE as u64,
        });
    }
    Ok(request_payload)
}

async fn send_initial_request(
    outbound: &mpsc::Sender<HostFrame>,
    execution_id: &str,
    request_payload: SandboxRequestPayload,
) -> SandboxResult<()> {
    outbound
        .send(HostFrame {
            protocol_revision: SANDBOX_WORKER_PROTOCOL_REVISION,
            execution_id: execution_id.to_string(),
            frame: Some(host_frame::Frame::RequestPayload(request_payload)),
        })
        .await
        .map_err(|_| {
            SandboxError::Aborted("sandbox worker request stream could not start".to_string())
        })
}

fn cancel_frame(execution_id: &str) -> HostFrame {
    HostFrame {
        protocol_revision: SANDBOX_WORKER_PROTOCOL_REVISION,
        execution_id: execution_id.to_string(),
        frame: Some(host_frame::Frame::CancelExecution(CancelExecution {})),
    }
}

fn validate_worker_frame(frame: &WorkerFrame, execution_id: &str) -> SandboxResult<()> {
    if frame.protocol_revision != SANDBOX_WORKER_PROTOCOL_REVISION {
        return Err(SandboxError::Aborted(
            "sandbox worker protocol revision mismatch".to_string(),
        ));
    }
    if frame.execution_id != execution_id {
        return Err(SandboxError::Aborted(
            "sandbox worker frame execution identity mismatch".to_string(),
        ));
    }
    Ok(())
}

fn encode_request(request: &SandboxRequest) -> SandboxResult<SandboxRequestPayload> {
    let mut metadata = request.clone();
    let artifact_bytes = std::mem::take(&mut metadata.payload.bytes);
    Ok(SandboxRequestPayload {
        request_json: encode(&metadata, "sandbox request")?,
        artifact_bytes,
    })
}

fn encode<T: serde::Serialize>(value: &T, label: &str) -> SandboxResult<Vec<u8>> {
    serde_json::to_vec(value)
        .map_err(|error| SandboxError::Internal(format!("could not encode {label}: {error}")))
}

fn decode<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8], label: &str) -> SandboxResult<T> {
    serde_json::from_slice(bytes)
        .map_err(|error| SandboxError::Aborted(format!("could not decode {label}: {error}")))
}

fn transport_error(error: impl std::fmt::Display) -> SandboxError {
    SandboxError::Aborted(format!("sandbox worker transport failed: {error}"))
}
