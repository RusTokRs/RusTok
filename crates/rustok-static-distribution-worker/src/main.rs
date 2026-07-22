use std::sync::Arc;

use rustok_module_build_transport::{
    static_distribution_proto::static_distribution_build_service_server::StaticDistributionBuildServiceServer,
    StaticDistributionGrpcService,
};
use rustok_static_distribution_worker::StaticDistributionWorker;
use rustok_worker_transport::MutualTlsListenerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = MutualTlsListenerConfig::from_env_prefix("RUSTOK_STATIC_DISTRIBUTION")?;
    let worker = Arc::new(StaticDistributionWorker::from_env(
        listener.request_timeout,
    )?);
    listener
        .server()?
        .concurrency_limit_per_connection(listener.concurrency_limit)
        .timeout(listener.request_timeout)
        .add_service(
            StaticDistributionBuildServiceServer::new(StaticDistributionGrpcService::new(worker))
                .max_decoding_message_size(listener.max_message_size)
                .max_encoding_message_size(listener.max_message_size),
        )
        .serve(listener.address)
        .await?;
    Ok(())
}
