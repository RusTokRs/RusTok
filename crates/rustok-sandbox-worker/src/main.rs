use std::sync::Arc;

use rustok_sandbox::{RhaiCapabilityBridge, RhaiStandardLibrary, rhai::RhaiExecutor};
use rustok_sandbox_transport::proto::sandbox_worker_service_server::SandboxWorkerServiceServer;
use rustok_sandbox_transport::{SANDBOX_WORKER_MAX_MESSAGE_SIZE, SandboxWorkerGrpcService};
use rustok_sandbox_worker::{
    IsolationPolicy, ObservedRhaiExecutor, ObservedWorkerReadiness, WorkerMemoryObserver,
};
use rustok_worker_transport::MutualTlsListenerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = MutualTlsListenerConfig::from_env_prefix(
        "RUSTOK_SANDBOX",
        SANDBOX_WORKER_MAX_MESSAGE_SIZE,
    )?;
    let memory = WorkerMemoryObserver::cgroup_v2()?;
    let isolation = Arc::new(ObservedWorkerReadiness::new(
        IsolationPolicy::from_env()?,
        memory.clone(),
    ));
    let executor = Arc::new(ObservedRhaiExecutor::new(
        RhaiExecutor::new()
            .with_extension(Arc::new(RhaiStandardLibrary))
            .with_extension(Arc::new(RhaiCapabilityBridge)),
        memory,
    ));
    let service = SandboxWorkerGrpcService::new(executor, isolation)?;

    listener
        .server()?
        .concurrency_limit_per_connection(listener.concurrency_limit)
        .timeout(listener.request_timeout)
        .add_service(
            SandboxWorkerServiceServer::new(service)
                .max_decoding_message_size(listener.max_message_size)
                .max_encoding_message_size(listener.max_message_size),
        )
        .serve(listener.address)
        .await?;
    Ok(())
}
