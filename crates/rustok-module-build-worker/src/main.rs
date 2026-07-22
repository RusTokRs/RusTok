use std::sync::Arc;

use rustok_module_build_transport::{
    ModuleBuildGrpcService,
    module_build_proto::module_build_service_server::ModuleBuildServiceServer,
};
use rustok_module_build_worker::OciJobBuildWorker;
use rustok_worker_transport::MutualTlsListenerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = MutualTlsListenerConfig::from_env_prefix("RUSTOK_MODULE_BUILD")?;
    let worker = Arc::new(OciJobBuildWorker::from_env(listener.request_timeout)?);
    listener
        .server()?
        .concurrency_limit_per_connection(listener.concurrency_limit)
        .timeout(listener.request_timeout)
        .add_service(
            ModuleBuildServiceServer::new(ModuleBuildGrpcService::new(worker))
                .max_decoding_message_size(listener.max_message_size)
                .max_encoding_message_size(listener.max_message_size),
        )
        .serve(listener.address)
        .await?;
    Ok(())
}
