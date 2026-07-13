use std::sync::Arc;

use rustok_verification_transport::proto::verification_service_server::VerificationServiceServer;
use rustok_verification_worker::{
    CosignTrustVerifier, ListenerConfig, VerificationGrpcService, VerificationPolicy,
    VerificationWorker,
};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = ListenerConfig::from_env()?;
    let policy: VerificationPolicy =
        serde_json::from_str(&std::env::var("RUSTOK_VERIFICATION_POLICY_JSON")?)?;
    policy.validate()?;
    let worker = Arc::new(VerificationWorker::new(
        CosignTrustVerifier::new(policy.clone()),
        policy,
    ));
    listener
        .server()?
        .concurrency_limit_per_connection(listener.concurrency_limit)
        .timeout(listener.request_timeout)
        .add_service(
            VerificationServiceServer::new(VerificationGrpcService::new(worker))
                .max_decoding_message_size(listener.max_message_size)
                .max_encoding_message_size(listener.max_message_size),
        )
        .serve(listener.address)
        .await?;
    Ok(())
}
